use opencv::{
    prelude::*,
    core::Scalar,
    core::Size,
    core::Mat,
    core::Vector,
    core::get_cuda_enabled_device_count,
    core::CV_32F,
    core::Rect,
    highgui::named_window,
    highgui::resize_window,
    highgui::imshow,
    highgui::wait_key,
    videoio::VideoCapture,
    videoio::CAP_ANY,
    videoio::get_backends,
    imgproc::resize,
    dnn::DNN_BACKEND_CUDA,
    dnn::DNN_TARGET_CUDA,
    dnn::Net,
    dnn::read_net,
    dnn::read_net_from_caffe,
    dnn::blob_from_image,
    dnn::nms_boxes
};

use std::time::Instant;
use std::io::Write;
use std::thread;
use chrono::{
    DateTime,
    Utc,
    Duration
};
mod lib;
use lib::tracking::{
    KalmanBlobie,
    KalmanBlobiesTracker,
};
use lib::polygons::{ConvexPolygons};
mod settings;
use settings::{
    AppSettings,
};
mod storage;

use lib::rest_api;
use std::sync::{Arc, Mutex, RwLock};

fn run() -> opencv::Result<()> {
    let app_settings = AppSettings::new_settings("./data/conf.toml");
    println!("Settings are: {:?}", app_settings);

    let output_width: i32 = app_settings.output.width;
    let output_height: i32 = app_settings.output.height;
    let conf_threshold: f32 = app_settings.detection.conf_threshold;
    let nms_threshold: f32 = app_settings.detection.nms_threshold;
    let max_points_in_track: usize = app_settings.tracking.max_points_in_track;

    let default_scalar: Scalar = Scalar::default();

    // Define default tracker for detected objects (blobs storage)
    let mut tracker = KalmanBlobiesTracker::default();

    let video_src = &app_settings.input.video_src;
    const VIDEOCAPTURE_POS_MSEC: i32 = 0;
    let weights_src = &app_settings.detection.network_weights;
    let cfg_src = &app_settings.detection.network_cfg;
    let network_type = app_settings.detection.network_type.to_lowercase();
    let window = &app_settings.output.window_name;

    const COCO_FILTERED_CLASSNAMES: &'static [&'static str] = &["car", "motorbike", "bus", "train", "truck"];
    let mut convex_polygons = ConvexPolygons::new_with_id(app_settings.equipment_info.id);
    let convex_polygons_cloned = convex_polygons.clone_arc();
    for road_lane in app_settings.road_lanes.iter() {
        let mut polygon = road_lane.convert_to_convex_polygon();
        polygon.set_target_classes(COCO_FILTERED_CLASSNAMES);
        convex_polygons.insert_polygon(polygon);
    }
    let convex_polygons_rest = convex_polygons.clone_arc();
    let worker_reset_millis = app_settings.worker.reset_data_milliseconds;
    thread::spawn(move || {
        convex_polygons.start_data_worker(worker_reset_millis);
    });

    let server_host = app_settings.rest_api.host;
    let server_port = app_settings.rest_api.back_end_port;
    thread::spawn(move || {
        rest_api::start_rest_api(server_host, server_port, convex_polygons_rest);
    });

    // Prepare output window
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
    println!("Available <videoio> backends: {:?}", get_backends()?);

    // Check if CUDA is an option at all
    let cuda_count = get_cuda_enabled_device_count()?;
    let cuda_available = cuda_count > 0;
    println!("CUDA is {}", if cuda_available { "available" } else { "not available" });
    
    // Prepare video
    let mut video_capture = match VideoCapture::from_file(video_src, CAP_ANY) {
        Ok(result) => {result},
        Err(err) => {
            panic!("Can't init '{}' due the error: {:?}", video_src, err);
        }
    };
    let opened = VideoCapture::is_opened(&video_capture)?;
    if !opened {
        panic!("Unable to open video '{}'", video_src);
    }

    // Prepare neural network
    let mut neural_net: Net; 
    let blob_scale;
    let net_size;
    let blob_mean;
    let blob_name;

    let coco_classnames: &'static [&'static str];

    match network_type.as_ref() {
        "darknet" => {
            neural_net = match read_net(weights_src, cfg_src, "Darknet"){
                Ok(result) => result,
                Err(err) => {
                    panic!("Can't read network '{}' (with cfg '{}') due the error: {:?}", weights_src, cfg_src, err);
                }
            };
            blob_scale = 1.0/255.0;
            net_size = Size::new(416, 416);
            blob_mean = default_scalar;
            blob_name = "";
            coco_classnames = &["person", "bicycle", "car", "motorbike", "aeroplane", "bus", "train", "truck", "boat", "traffic light", "fire hydrant", "stop sign", "parking meter", "bench", "bird", "cat", "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra", "giraffe", "backpack", "umbrella", "handbag", "tie", "suitcase", "frisbee", "skis", "snowboard", "sports ball", "kite", "baseball bat", "baseball glove", "skateboard", "surfboard", "tennis racket", "bottle", "wine glass", "cup", "fork", "knife", "spoon", "bowl", "banana", "apple", "sandwich", "orange", "broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair", "sofa", "pottedplant", "bed", "diningtable", "toilet", "tvmonitor", "laptop", "mouse", "remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink", "refrigerator", "book", "clock", "vase", "scissors", "teddy bear", "hair drier", "toothbrush"];
        },
        "caffe-mobilenet-ssd" => {
            neural_net = match read_net_from_caffe(weights_src, cfg_src){
                Ok(result) => result,
                Err(err) => {
                    panic!("Can't read network '{}' (with cfg '{}') due the error: {:?}", weights_src, cfg_src, err);
                }
            };
            blob_scale = 0.007843;
            net_size = Size::new(300, 300);
            blob_mean = Scalar::from(127.5);
            blob_name = "data";
            coco_classnames = &["background", "aeroplane", "bicycle", "bird", "boat", "bottle", "bus", "car", "cat", "chair", "cow", "diningtable", "dog", "horse", "motorbike", "person", "pottedplant", "sheep", "sofa", "train", "tvmonitor"];
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

    loop {
        let all_now = Instant::now();

        match video_capture.read(&mut frame) {
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

        let elapsed_capture = all_now.elapsed().as_millis() as f32;

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
                    tmp_blobs = process_yolo_detections(&detections, conf_threshold, nms_threshold, frame_cols, frame_rows, max_points_in_track, coco_classnames, COCO_FILTERED_CLASSNAMES, classes_num, last_time, sec_diff);
                } else {
                    /* Caffe's Mobilenet */
                    tmp_blobs = process_mobilenet_detections(&detections, conf_threshold, frame_cols, frame_rows, max_points_in_track, coco_classnames, COCO_FILTERED_CLASSNAMES, last_time, sec_diff);
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
                    b.draw_track(&mut frame);
                    b.draw_center(&mut frame);
                    // b.draw_predicted(&mut frame);
                    b.draw_rectangle(&mut frame);
                    b.draw_class_name(&mut frame);
                    b.draw_id(&mut frame);
                }
            }
            Err(err) => {
                println!("Can't process input of neural network due the error {:?}", err);
            }
        }
        let elapsed_detection = 1000.0 / detection_now.elapsed().as_millis() as f32;

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

        let elapsed_all = 1000.0 / all_now.elapsed().as_millis() as f32;
        print!("\rСapturing process millis: {} | Average FPS of detection process: {} | Average FPS of whole process: {}", elapsed_capture, elapsed_detection, elapsed_all);
        match std::io::stdout().flush() {
            Ok(_) => {},
            Err(err) => {
                panic!("There is a problem with stdout().flush(): {}", err);
            }
        };
    }
    Ok(())
}

fn process_mobilenet_detections(detections: &Vector::<Mat>, conf_threshold: f32, frame_cols: f32, frame_rows: f32, max_points_in_track: usize, classes: &'static [&'static str], filtered_classes: &'static [&'static str], last_time: DateTime<Utc>, sec_diff: f64) -> Vec<KalmanBlobie> {
    let mut tmp_blobs = vec![];
    let outs = detections.len();
    for o in 0..outs {
        let output = detections.get(o).unwrap();
        let data_ptr = output.data_typed::<f32>().unwrap();
        for (i, _) in data_ptr.iter().enumerate().step_by(7) {
            let confidence = data_ptr[i+2];
            let class_id = data_ptr[i+1] as usize;
            let class_name = classes[class_id];
            if filtered_classes.contains(&class_name) {
                if confidence > conf_threshold {
                    let left = (data_ptr[i+3] * frame_cols) as i32;
                    let top = (data_ptr[i+4] * frame_rows) as i32;
                    let right = (data_ptr[i+5] * frame_cols) as i32;
                    let bottom = (data_ptr[i+6] * frame_rows) as i32;
                    let width = right - left + 1; 
                    let height = bottom - top + 1;
                    if (frame_cols as i32 - width) < 100 {
                        continue
                    }
                    let bbox = Rect::new(left, top, width, height);
                    let mut kb = KalmanBlobie::new_with_time(&bbox, max_points_in_track, last_time, sec_diff);
                    kb.set_class_name(class_name.to_string());
                    tmp_blobs.push(kb);
                }
            }
        }
    }
    return tmp_blobs;
}

fn process_yolo_detections(detections: &Vector::<Mat>, conf_threshold: f32, nms_threshold: f32, frame_cols: f32, frame_rows: f32, max_points_in_track: usize, classes: &'static [&'static str], filtered_classes: &'static [&'static str], classes_num: usize, last_time: DateTime<Utc>, sec_diff: f64) -> Vec<KalmanBlobie> {
    let mut tmp_blobs = vec![];
    let outs = detections.len();
    let mut class_names = vec![];
    let mut confidences = Vector::<f32>::new();
    let mut bboxes = Vector::<Rect>::new();
    for o in 0..outs {
        let output = detections.get(o).unwrap();
        let data_ptr = output.data_typed::<f32>().unwrap();
        for (i, _) in data_ptr.iter().enumerate().step_by(classes_num + 5) {
            let mut class_id = 0 as usize;
            let mut max_probability = 0.0;
            for j in 5..(classes_num + 5) {
                if data_ptr[i+j] > max_probability {
                    max_probability = data_ptr[i+j];
                    class_id = (j-5) % classes_num;
                }
            }
            let class_name = classes[class_id];
            if filtered_classes.contains(&class_name) {
                let confidence = max_probability * data_ptr[i+4];
                if confidence > conf_threshold {
                    let center_x = data_ptr[i] * frame_cols;
                    let center_y = data_ptr[i + 1] * frame_rows;
                    let width = data_ptr[i + 2] * frame_cols;
                    let height = data_ptr[i + 3] * frame_rows;
                    let left = center_x - width / 2.0;
                    let top = center_y - height / 2.0;
                    let bbox = Rect::new(left as i32, top as i32, width as i32, height as i32);
                    class_names.push(class_name);
                    confidences.push(confidence);
                    bboxes.push(bbox);
                }
            }
        }
    }
    let mut indices = Vector::<i32>::new();
    match nms_boxes(&bboxes, &confidences, conf_threshold, nms_threshold, &mut indices, 1.0, 0) {
        Ok(_) => {},
        Err(err) => {
            println!("Can't run NMSBoxes on detections due the error {:?}", err);
        }
    };
    for (i, _) in indices.iter().enumerate() {
        match bboxes.get(i) {
            Ok(bbox) => {
                let class_name = class_names[i];
                let mut kb = KalmanBlobie::new_with_time(&bbox, max_points_in_track, last_time, sec_diff);
                kb.set_class_name(class_name.to_string());
                tmp_blobs.push(kb);
            },
            Err(err) => {
                panic!("Can't extract bbox from filtered bboxes due the error {:?}", err);
            }
        }
    }
    return tmp_blobs;
}


fn main() {
    // thread::spawn(|| {
    //     job_worker();
    // });
    run().unwrap()
}
