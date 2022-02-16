use opencv::{
    prelude::*,
    core::Scalar,
    core::Size,
    core::Mat,
    core::Vector,
    core::get_cuda_enabled_device_count,
    core::CV_32F,
    highgui::named_window,
    highgui::resize_window,
    highgui::imshow,
    highgui::wait_key,
    videoio::VideoCapture,
    videoio::get_backends,
    imgproc::resize,
    dnn::DNN_BACKEND_CUDA,
    dnn::DNN_TARGET_CUDA,
    dnn::Net,
    dnn::read_net,
    dnn::read_net_from_caffe,
    dnn::blob_from_image,
};

use chrono::{
    DateTime,
    Utc,
    Duration
};

mod lib;
use lib::tracking::{
    KalmanBlobiesTracker,
};
use lib::data_storage::{
    DataStorage
};
use lib::detection::{
    process_yolo_detections,
    process_mobilenet_detections
};

mod settings;
use settings::{
    AppSettings,
};

mod video_capture;
use video_capture::{
    get_video_capture
};

mod publisher;
use publisher::{
    RedisConnection
};

use lib::rest_api;
use std::env;
use std::time::Duration as STDDuration;
use std::process;
use std::time::Instant;
use std::io::Write;
use std::thread;
use std::sync::{Arc, RwLock, mpsc};

use ctrlc;

const VIDEOCAPTURE_POS_MSEC: i32 = 0;
const COCO_FILTERED_CLASSNAMES: &'static [&'static str] = &["car", "motorbike", "bus", "train", "truck"];

pub struct ThreadedFrame {
    frame: Mat,
    last_time: DateTime<Utc>,
    sec_diff: f64,
    capture_millis: f32,
}

fn run(config_file: &str) -> opencv::Result<()> {

    let app_settings = AppSettings::new_settings(config_file);
    println!("Settings are:\n\t{}", app_settings);

    let output_width: i32 = app_settings.output.width;
    let output_height: i32 = app_settings.output.height;
    let conf_threshold: f32 = app_settings.detection.conf_threshold;
    let nms_threshold: f32 = app_settings.detection.nms_threshold;
    let max_points_in_track: usize = app_settings.tracking.max_points_in_track;
    let default_scalar: Scalar = Scalar::default();

    // Define default tracker for detected objects (blobs storage)
    let mut tracker = KalmanBlobiesTracker::default();

    let video_src = &app_settings.input.video_src;
    let weights_src = &app_settings.detection.network_weights;
    let cfg_src = &app_settings.detection.network_cfg;
    let network_type = app_settings.detection.network_type.to_lowercase();
    let window = &app_settings.output.window_name;
    let verbose_dbg = match app_settings.debug {
        Some(x) => { x.enable },
        None => { false }
    };

    let convex_polygons = DataStorage::new_with_id(app_settings.equipment_info.id);
    let convex_polygons_arc = Arc::new(RwLock::new(convex_polygons));

    let convex_polygons_populate = convex_polygons_arc.clone();
    let scale_x = match app_settings.input.scale_x {
        Some(x) => { x },
        None => { 1.0 }
    };
    let scale_y = match app_settings.input.scale_y {
        Some(y) => { y },
        None => { 1.0 }
    };
    for road_lane in app_settings.road_lanes.iter() {
        let mut polygon = road_lane.convert_to_convex_polygon();
        polygon.scale_geom(scale_x, scale_y);    
        polygon.set_target_classes(COCO_FILTERED_CLASSNAMES);
        let guarded = convex_polygons_populate.write().unwrap();
        guarded.insert_polygon(polygon);
        drop(guarded);
    }
    let worker_reset_millis = app_settings.worker.reset_data_milliseconds;
    let convex_polygons_analytics = convex_polygons_arc.clone();
    thread::spawn(move || {
        DataStorage::start_data_worker_thread(convex_polygons_analytics, worker_reset_millis, verbose_dbg);
    });

    if app_settings.redis_publisher.enable {
        let convex_polygons_redis = convex_polygons_arc.clone();
        let redis_host = app_settings.redis_publisher.host;
        let redis_port = app_settings.redis_publisher.port;
        let redis_password = app_settings.redis_publisher.password;
        let redis_db_index = app_settings.redis_publisher.db_index;
        let redis_channel = app_settings.redis_publisher.channel_name;
        thread::spawn(move || {
            let mut redis_conn = match redis_password.chars().count() {
                0 => {
                    RedisConnection::new(redis_host, redis_port, redis_db_index)
                },
                _ => {
                    RedisConnection::new_with_password(redis_host, redis_port, redis_db_index, redis_password)
                }
            };
            if redis_channel.chars().count() != 0 {
                redis_conn.set_channel(redis_channel);
            }
            redis_conn.start_worker(convex_polygons_redis, worker_reset_millis);
        });
    }

    if app_settings.rest_api.enable {
        let server_host = app_settings.rest_api.host;
        let server_port = app_settings.rest_api.back_end_port;
        let convex_polygons_rest = convex_polygons_arc.clone();
        thread::spawn(move || {
            match rest_api::start_rest_api(server_host, server_port, convex_polygons_rest) {
                Ok(_) => {},
                Err(err) => {
                    panic!("Can't start API due the error: {:?}", err)
                }
            }
        });
    }
    
    let convex_polygons_cv = convex_polygons_arc.clone();
    let convex_polygons_cv_read = convex_polygons_cv.read().unwrap();
    let convex_polygons_cloned = convex_polygons_cv_read.clone_polygons_arc();
    drop(convex_polygons_cv_read);

    // Prepare output window
    if app_settings.output.enable {
        match named_window(window, 1) {
            Ok(_) => {},
            Err(err) =>{
                panic!("Can't give a name to output window due the error: {:?}", err)
            }
        };
        match resize_window(window, output_width, output_height) {
            Ok(_) => {},
            Err(err) =>{
                panic!("Can't resize output window due the error: {:?}", err)
            }
        }
    }
    println!("Available <videoio> backends: {:?}", get_backends()?);

    // Check if CUDA is an option at all
    let cuda_count = get_cuda_enabled_device_count()?;
    let cuda_available = cuda_count > 0;
    println!("CUDA is {}", if cuda_available { "available" } else { "not available" });
    
    // Prepare video
    let mut video_capture = get_video_capture(video_src, app_settings.input.typ);
    let opened = VideoCapture::is_opened(&video_capture)?;
    if !opened {
        panic!("Unable to open video '{}'", video_src);
    }

    // Prepare neural network
    let mut neural_net: Net; 
    let blob_scale;
    let net_size = Size::new(app_settings.detection.net_width, app_settings.detection.net_height);
    let blob_mean;
    let blob_name;

    let coco_classnames = app_settings.detection.net_classes;

    match network_type.as_ref() {
        "darknet" => {
            neural_net = match read_net(weights_src, cfg_src, "Darknet"){
                Ok(result) => result,
                Err(err) => {
                    panic!("Can't read network '{}' (with cfg '{}') due the error: {:?}", weights_src, cfg_src, err);
                }
            };
            blob_scale = 1.0/255.0;
            blob_mean = default_scalar;
            blob_name = "";
        },
        "caffe-mobilenet-ssd" => {
            neural_net = match read_net_from_caffe(weights_src, cfg_src){
                Ok(result) => result,
                Err(err) => {
                    panic!("Can't read network '{}' (with cfg '{}') due the error: {:?}", weights_src, cfg_src, err);
                }
            };
            blob_scale = 0.007843;
            blob_mean = Scalar::from(127.5);
            blob_name = "data";
        },
        _ => {
            panic!("Only this network types are supported: Darknet / Caffe-Mobilenet-SSD. You've provided: '{}'", app_settings.detection.network_type);
        }
    };

    let classes_num: usize = coco_classnames.len();

    let out_layers_names = match neural_net.get_unconnected_out_layers_names() {
        Ok(result) => result,
        Err(err) => {
            panic!("Can't get output layers names of neural network due the error: {:?}", err);
        }
    };

    // Initialize CUDA back-end if possible
    if cuda_available {
        match neural_net.set_preferable_backend(DNN_BACKEND_CUDA){
            Ok(_) => {},
            Err(err) => {
                panic!("Can't set DNN_BACKEND_CUDA for neural network due the error {:?}", err);
            }
        }
        match neural_net.set_preferable_target(DNN_TARGET_CUDA){
            Ok(_) => {},
            Err(err) => {
                panic!("Can't set DNN_TARGET_CUDA for neural network due the error {:?}", err);
            }
        }
    }
    
    let mut frame = Mat::default();
    let mut resized_frame = Mat::default();
    let mut detections = Vector::<Mat>::new();

    /* Read first frame to determine image width/height */
    match video_capture.read(&mut frame) {
        Ok(_) => {},
        Err(_) => {
            panic!("Can't read first frame");
        }
    };
    let frame_cols = frame.cols() as f32;
    let frame_rows = frame.rows() as f32;

    let mut last_ms: f64 = 0.0;
	let mut last_time = Utc::now();

    println!("Waiting for Ctrl-C...");
    ctrlc::set_handler(move || {
        println!("\nCtrl+C has been pressed! Exit in 2 seconds");
        thread::sleep(STDDuration::from_secs(2));
        process::exit(1);
    }).expect("\nError setting Ctrl-C handler");

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        loop {
            let capture_now = Instant::now();
            let mut read_frame = Mat::default();
            match video_capture.read(&mut read_frame) {
                Ok(_) => {},
                Err(_) => {
                    println!("Can't read next frame");
                    break;
                }
            };
            /* Evaluate time difference */
            let current_ms = video_capture.get(VIDEOCAPTURE_POS_MSEC).unwrap();
            let ms_diff = current_ms - last_ms;
            let sec_diff = ms_diff / 1000.0;
            let prev_time = last_time;
            last_time = prev_time + Duration::milliseconds(ms_diff as i64);
            last_ms = current_ms;
            let elapsed_capture = capture_now.elapsed().as_millis() as f32;
            /* Send frame and capture info */
            tx.send(ThreadedFrame{
                frame: read_frame,
                sec_diff: sec_diff,
                last_time: last_time,
                capture_millis: elapsed_capture,
            }).unwrap();
        }
    });

    for received in rx {
        let mut frame = received.frame.clone();

        let blobimg = blob_from_image(&frame, blob_scale, net_size, blob_mean, true, false, CV_32F);
        match neural_net.set_input(&blobimg.unwrap(), blob_name, 1.0, default_scalar){
            Ok(_) => {},
            Err(err) => {
                println!("Can't set input of neural network due the error {:?}", err);
            }
        };

        let detection_now = Instant::now();
        match neural_net.forward(&mut detections, &out_layers_names) {
            Ok(_) => {
                let mut tmp_blobs;
                if network_type == "darknet" {
                    /* Tiny YOLO */
                    tmp_blobs = process_yolo_detections(
                        &detections,
                        conf_threshold,
                        nms_threshold,
                        frame_cols,
                        frame_rows,
                        max_points_in_track,
                        &coco_classnames,
                        COCO_FILTERED_CLASSNAMES,
                        classes_num,
                        received.last_time,
                        received.sec_diff,
                    );
                } else {
                    /* Caffe's Mobilenet */
                    tmp_blobs = process_mobilenet_detections(
                        &detections,
                        conf_threshold,
                        frame_cols,
                        frame_rows,
                        max_points_in_track, 
                        &coco_classnames,
                        COCO_FILTERED_CLASSNAMES,
                        received.last_time,
                        received.sec_diff,
                    );
                }

                // Match blobs
                tracker.match_to_existing(&mut tmp_blobs);

                // Run through the blobs and check if some of them either entered or left road lanes polygons
                for (_, b) in tracker.objects.iter_mut() {
                    let n = b.get_track().len();
                    let blob_center = b.get_center();
                    if n > 2 {
                        let blob_id = b.get_id();
                        let mut convex_polygons_write = convex_polygons_cloned.write().expect("RwLock poisoned");
                        for (_, v) in convex_polygons_write.iter_mut() {
                            let mut polygon = v.lock().expect("Mutex poisoned");
                            let contains_blob = polygon.contains_cv_point(&blob_center);
                            if contains_blob {
                                b.estimate_speed_mut(&polygon.spatial_converter);
                                // If blob is not registered in polygon
                                if !polygon.blob_registered(&blob_id) {
                                    // Register it
                                    polygon.register_blob(blob_id);
                                }
                            } else {
                                // Otherwise
                                // If blob is registered in polygon and left it (since contains_blob == false)
                                if polygon.blob_registered(&blob_id) {
                                    polygon.deregister_blob(&blob_id);
                                    polygon.increment_intensity(b.get_class_name());
                                    polygon.consider_speed(b.get_class_name(), b.get_avg_speed());
                                }
                            }
                            drop(polygon);
                        }
                        drop(convex_polygons_write);
                    }
                    if app_settings.output.enable {
                        b.draw_track(&mut frame);
                        b.draw_center(&mut frame);
                        // b.draw_predicted(&mut frame);
                        b.draw_rectangle(&mut frame);
                        b.draw_class_name(&mut frame);
                        b.draw_id(&mut frame);
                    }
                }
            }
            Err(err) => {
                println!("Can't process input of neural network due the error {:?}", err);
            }
        }
        let elapsed_detection = detection_now.elapsed().as_millis();

        if app_settings.output.enable {
            let convex_polygons_read = convex_polygons_cloned.read().expect("RwLock poisoned");
            for (_, v) in convex_polygons_read.iter() {
                let polygon = v.lock().expect("Mutex poisoned");
                polygon.draw_geom(&mut frame);
                polygon.draw_params(&mut frame);
                drop(polygon);
            }
            drop(convex_polygons_read);
            match resize(&mut frame, &mut resized_frame, Size::new(output_width, output_height), 1.0, 1.0, 1) {
                Ok(_) => {},
                Err(err) => {
                    panic!("Can't resize output frame due the error {:?}", err);
                }
            }
            if resized_frame.size()?.width > 0 {
                imshow(window, &mut resized_frame)?;
            }
            let key = wait_key(10)?;
            if key > 0 && key != 255 {
                break;
            }
        }

        if verbose_dbg {
            print!("\rСapturing process millis: {} | Detection process millis: {} | Average FPS of detection process: {}", received.capture_millis, elapsed_detection, 1000.0 / elapsed_detection as f32);
            match std::io::stdout().flush() {
                Ok(_) => {},
                Err(err) => {
                    panic!("There is a problem with stdout().flush(): {}", err);
                }
            };
        }  
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let path_to_config = match args.len() {
        2 => {
            &args[1]
        },
        _ => {
            println!("Args should contain exactly one string: path to TOML configuration file. Setting to default './data/conf.toml'");
            "./data/conf.toml"
        }
    };
    run(path_to_config).unwrap();
}
