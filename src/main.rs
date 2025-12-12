use chrono::Utc;
use opencv::{
    prelude::*,
    core::Scalar,
    core::Size,
    core::Mat,
    core::Vector,
    core::Rect as RectCV,
    core::get_cuda_enabled_device_count,
    highgui::named_window,
    highgui::resize_window,
    highgui::imshow,
    highgui::wait_key,
    videoio::VideoCapture,
    imgproc::resize,
    imgcodecs::imencode,
    dnn::DNN_BACKEND_CUDA,
    dnn::DNN_TARGET_CUDA,
    dnn::DNN_BACKEND_OPENCV,
    dnn::DNN_TARGET_CPU,
};

use od_opencv::{
    model_format::ModelFormat,
    model_format::ModelVersion,
    model::new_from_file,
    model::ModelTrait,
};

mod lib;
use lib::data_storage::new_datastorage;
use lib::draw;
use lib::tracker::{
    new_tracker_from_type,
    TrackerTrait,
    SpatialInfo
};
use lib::detection::process_yolo_detections;
use lib::zones::Zone;
use lib::dataset_collector::DatasetCollector;

mod settings;
use settings::AppSettings;

mod video_capture;
use video_capture::{
    get_video_capture,
    ThreadedFrame
};

use lib::publisher::RedisConnection;

mod rest_api;

use std::env;
use std::time::Duration as STDDuration;
use std::time::SystemTime;
use std::process;
use std::thread;
use std::sync::mpsc;
use std::fmt;
use std::collections::HashSet;
use std::iter::FromIterator;

const EMPTY_FRAMES_LIMIT: u16 = 60;

fn get_sys_time_in_secs() -> u64 {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(n) => n.as_secs(),
        Err(_) => panic!("SystemTime before UNIX EPOCH!"),
    }
}

#[derive(Debug)]
struct AppVideoError{typ: i16}
impl fmt::Display for AppVideoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.typ {
            1 => write!(f, "Can't open video"),
            2 => write!(f, "Can't make probe for video"),
            _ => write!(f, "Undefined application video error")
        }
    }
}

#[derive(Debug)]
enum AppError {
    VideoError(AppVideoError),
    OpenCVError(opencv::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::VideoError(e) => write!(f, "{}", e),
            AppError::OpenCVError(e) => write!(f, "{}", e),
        }
    }
}

impl From<AppVideoError> for AppError {
    fn from(e: AppVideoError) -> Self {
        AppError::VideoError(e)
    }
}

impl From<opencv::Error> for AppError {
    fn from(e: opencv::Error) -> Self {
        AppError::OpenCVError(e)
    }
}

fn probe_video(capture: &mut VideoCapture) ->  Result<(f32, f32, f32), AppError> {
    let fps = capture.get(opencv::videoio::CAP_PROP_FPS)? as f32;
    let frame_cols = capture.get(opencv::videoio::CAP_PROP_FRAME_WIDTH)? as f32;
    let frame_rows = capture.get(opencv::videoio::CAP_PROP_FRAME_HEIGHT)? as f32;

    // Is it better to get width/height from frame information?
    // let mut frame = Mat::default();
    // match capture.read(&mut frame) {
    //     Ok(_) => {},
    //     Err(_) => {
    //         return Err(AppError::VideoError(AppVideoError{typ: 2}));
    //     }
    // };
    // let frame_cols = frame.cols() as f32;
    // let frame_rows = frame.rows() as f32;
    Ok((frame_cols, frame_rows, fps))
}

fn prepare_neural_net(mf: ModelFormat, mv: ModelVersion, weights: &str, configuration: Option<String>, net_size: (i32, i32)) -> Result<Box<dyn ModelTrait>, AppError> {

    /* Check if CUDA is an option at all */
    let cuda_count = get_cuda_enabled_device_count()?;
    let cuda_available = cuda_count > 0;
    println!("CUDA is {}", if cuda_available { "'available'" } else { "'not available'" });
    println!("Model format is '{:?}'", mf);
    println!("Model type is '{:?}'", mv);

    // Hacky way to convert Option<String> to Option<&str>
    let configuration_str = configuration.as_deref();

    let neural_net = match new_from_file(
        weights,
        configuration_str,
        (net_size.0, net_size.1),
        mf, mv,
        if cuda_available { DNN_BACKEND_CUDA } else { DNN_BACKEND_OPENCV },
        if cuda_available { DNN_TARGET_CUDA } else { DNN_TARGET_CPU },
        vec![]
    ) {
        Ok(result) => result,
        Err(err) => {
            panic!("Can't read network '{}' (with cfg '{:?}') due the error: {:?}", weights, configuration, err);
        }
    };
    Ok(neural_net)
}

fn run(settings: &AppSettings, path_to_config: &str, tracker: &mut dyn TrackerTrait, neural_net: &mut dyn ModelTrait, verbose: bool) -> Result<(), AppError> {
    println!("Verbose is '{}'", verbose);
    println!("REST API is '{}'", settings.rest_api.enable);
    println!("Redis publisher is '{}'", settings.redis_publisher.enable);

    let (enable_mjpeg, mjpeg_quality) = match &settings.rest_api.mjpeg_streaming {
        Some(v) => {
            let enabled = v.enable & settings.rest_api.enable; // Logical 'And' to prevent MJPEG when API is disabled
            (enabled, v.quality)
        }
        None => (false, 80)
    };

    println!("MJPEG is '{}' (quality: {})", enable_mjpeg, mjpeg_quality);

    /* Preprocess spatial data */
    let data_storage = new_datastorage(settings.equipment_info.id.clone(), verbose);
    let target_classes = HashSet::from_iter(settings.detection.target_classes.to_owned().unwrap_or(vec![]));
    let net_classes = settings.detection.net_classes.to_owned();
    let net_classes_set = HashSet::from_iter(net_classes.clone());
    let class_colors = draw::ClassColors::new(&net_classes);

    for road_lane in settings.road_lanes.iter() {
        let mut zone = Zone::from(road_lane);
        zone.set_target_classes(if !target_classes.is_empty() {
            &target_classes
        } else {
            &net_classes_set
        });
        match data_storage.write().unwrap().insert_zone(zone) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't insert zone due the error {:?}", err);
            }
        };
    }

    /* Initialize dataset collector if enabled */
    let mut dataset_collector: Option<DatasetCollector> = match &settings.dataset_collector {
        Some(dc_settings) if dc_settings.enabled => {
            match DatasetCollector::new(dc_settings.clone(), &net_classes) {
                Ok(collector) => Some(collector),
                Err(err) => {
                    println!("[WARNING] Can't initialize DatasetCollector: {}. Feature disabled.", err);
                    None
                }
            }
        },
        _ => None
    };

    // let data_storage_threaded = data_storage.clone();

    println!("Press `Ctrl-C` to stop main programm");
    ctrlc::set_handler(move || {
        println!("Ctrl+C has been pressed! Exit in 2 seconds");
        thread::sleep(STDDuration::from_secs(2));
        process::exit(1);
    }).expect("Error setting `Ctrl-C` handler");

    /* Start statistics ("threading" is obsolete because of business-logic error) */
    let reset_time = settings.worker.reset_data_milliseconds;
    let next_reset = reset_time as f32 / 1000.0;
    let ds_worker = data_storage.clone();
    
    /* Redis publisher */
    let redis_enabled = settings.redis_publisher.enable;
    let redis_worker = data_storage.clone();
    let redis_conn = match redis_enabled {
        true => {
            let redis_host = settings.redis_publisher.host.to_owned();
            let redis_port = settings.redis_publisher.port;
            let redis_password = settings.redis_publisher.password.to_owned();
            let redis_db_index = settings.redis_publisher.db_index;
            let redis_channel = settings.redis_publisher.channel_name.to_owned();
            let mut redis_conn = match redis_password.chars().count() {
                0 => {
                    RedisConnection::new(redis_host, redis_port, redis_db_index, redis_worker)
                },
                _ => {
                    RedisConnection::new_with_password(redis_host, redis_port, redis_db_index, redis_password, redis_worker)
                }
            };
            if redis_channel.chars().count() != 0 {
                redis_conn.set_channel(redis_channel);
            }
            Some(redis_conn)
        },
        false => {
            None
        }
    };

    /* Start REST API if needed */ 
    let overwrite_file = path_to_config.to_string();
    let (tx_mjpeg, rx_mjpeg) = mpsc::sync_channel(0);
    if settings.rest_api.enable {
        let settings_clone = settings.clone();
        let ds_api = data_storage.clone();
        thread::spawn(move || {
            match rest_api::start_rest_api(settings_clone.rest_api.host.clone(), settings_clone.rest_api.back_end_port, ds_api, enable_mjpeg, rx_mjpeg, settings_clone, &overwrite_file) {
                Ok(_) => {},
                Err(err) => {
                    println!("Can't start API due the error: {:?}", err)
                }
            }
        });
    }

    /* Probe video */
    let mut video_capture = get_video_capture(&settings.input.video_src, settings.input.typ.clone());
    let opened = VideoCapture::is_opened(&video_capture).map_err(AppError::from)?;
    if !opened {
        return Err(AppError::VideoError(AppVideoError{typ: 1}))
    }
    let (width, height, fps) = probe_video(&mut video_capture)?;
    println!("Video probe: {{Width: {width}px | Height: {height}px | FPS: {fps}}}");
    // Create imshow() if needed
    let window = &settings.output.window_name;
    let output_width: i32 = settings.output.width;
    let output_height: i32 = settings.output.height;
    if settings.output.enable {
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

    /* Start capture loop */
    let (tx_capture, rx_capture): (mpsc::SyncSender<ThreadedFrame>, mpsc::Receiver<ThreadedFrame>) = mpsc::sync_channel(0);
    thread::spawn(move || {
        let mut frames_counter: f32 = 0.0;
        let mut total_seconds: f32 = 0.0;
        let mut overall_seconds: f32 = 0.0;
        let mut empty_frames_countrer: u16 = 0;
        // @experimental
        let skip_every_n_frame = 2;
        // @todo: remove hardcode
        // let fps = 18.0;
        loop {
            let mut read_frame = Mat::default();
            match video_capture.read(&mut read_frame) {
                Ok(_) => {},
                Err(_) => {
                    println!("Can't read next frame");
                    break;
                }
            };
            if read_frame.empty() {
                if verbose {
                    println!("[WARNING]: Empty frame");
                }
                empty_frames_countrer += 1;
                if empty_frames_countrer >= EMPTY_FRAMES_LIMIT {
                    println!("Too many empty frames");
                    break
                }
                continue;
            }
            frames_counter += 1.0;
            let second_fraction = total_seconds + (frames_counter / fps);
            if frames_counter >= fps {
                total_seconds += 1.0;
                overall_seconds += 1.0;
                frames_counter = 0.0;
            }
            if frames_counter as i32 % skip_every_n_frame != 0 {
                continue;
            }
            // println!("Frame {frames_counter} | Second: {total_seconds} | Fraction: {second_fraction}");


            /* Send frame and capture info */
            let frame = ThreadedFrame{
                frame: read_frame,
                overall_seconds: overall_seconds,
                current_second: second_fraction,
            };

            match tx_capture.send(frame) {
                Ok(_)=>{},
                Err(_err) => {
                    // Closed channel?
                    // println!("Error on send frame to detection thread: {}", _err)
                }
            };

            // println!("Total seconds: {}", total_seconds);
            if total_seconds >= next_reset {
                println!("Reset timer due analytics. Current local time is: {}", second_fraction);
                total_seconds = 0.0;
                let mut ds_writer = ds_worker.write().expect("Bad DS");
                if ds_writer.period_end == ds_writer.period_start {
                    // First iteration
                    ds_writer.period_end = Utc::now();
                    ds_writer.period_start = ds_writer.period_end - chrono::Duration::milliseconds(reset_time);
                } else {
                    // Next iterations
                    ds_writer.period_start = ds_writer.period_end;
                    ds_writer.period_end += chrono::Duration::milliseconds(reset_time);
                }
                
                match ds_writer.update_statistics() {
                    Ok(_) => {
                        // Do not forget to drop mutex explicitly since we possible need to work with DS in REST API and Redis
                        drop(ds_writer)
                    },
                    Err(err) => {
                        println!("Can't update statistics due the error: {}", err);
                    }
                }
                if redis_enabled {
                    redis_conn.as_ref().unwrap().push_statistics();
                }
            }
        }
        match video_capture.release() {
            Ok(_) => {
                println!("Video capture has been closed successfully");
            },
            Err(err) => {
                println!("Can't release video capturer due the error: {}", err);
            }
        };
    });

    /* Detection thread */
    let conf_threshold: f32 = settings.detection.conf_threshold;
    let nms_threshold: f32 = settings.detection.nms_threshold;
    let max_points_in_track: usize = settings.tracking.max_points_in_track;
    let mut resized_frame = Mat::default();

    let ds_tracker = data_storage.clone();
    
    let tracker_dt = 1.0/fps;

    /* Can't create colors as const/static currently */
    let trajectory_scalar: Scalar = Scalar::from((0.0, 255.0, 0.0));
    let trajectory_scalar_inverse: Scalar = draw::invert_color(&trajectory_scalar);
    for received in rx_capture {
        // println!("Received frame from capture thread: {}", received.current_second);
        let mut frame = received.frame.clone();
        let (nms_bboxes, nms_classes_ids, nms_confidences) = match neural_net.forward(&frame, conf_threshold, nms_threshold) {
            Ok((a, b, c)) => { (a, b, c) },
            Err(err) => {
                println!("Can't process input of neural network due the error {:?}", err);
                continue;
            }
        };
        
        /* Process detected objects and match them to existing ones */
        let mut tmp_detections = process_yolo_detections(
            &nms_bboxes,
            nms_classes_ids,
            nms_confidences,
            width,
            height,
            max_points_in_track,
            &net_classes,
            &target_classes,
            tracker_dt,
        );

        let relative_time = received.overall_seconds;
        match tracker.match_objects(&mut tmp_detections, relative_time) {
            Ok(_) => {},
            Err(err) => {
                println!("Can't match objects due the error: {:?}", err);
                continue;
            }
        };

        /* Dataset collection - save raw frame and annotations */
        if let Some(ref mut collector) = dataset_collector {
            // Gather data from matched detections
            let mut dc_bboxes: Vec<RectCV> = Vec::with_capacity(tmp_detections.blobs.len());
            let mut dc_track_ids: Vec<uuid::Uuid> = Vec::with_capacity(tmp_detections.blobs.len());
            let mut dc_track_ages: Vec<usize> = Vec::with_capacity(tmp_detections.blobs.len());
            let mut dc_class_names: Vec<String> = Vec::with_capacity(tmp_detections.blobs.len());

            // Get track ages from the tracker's internal objects (not from detection blobs)
            let tracker_objects = tracker.get_engine_objects();

            for (i, blob) in tmp_detections.blobs.iter().enumerate() {
                let track_id = blob.get_id();

                // Get the actual track age from the tracker
                let track_age = tracker_objects
                    .get(&track_id)
                    .map(|obj| obj.get_track().len())
                    .unwrap_or(0);

                let bbox = blob.get_bbox();
                dc_bboxes.push(RectCV::new(
                    bbox.x as i32,
                    bbox.y as i32,
                    bbox.width as i32,
                    bbox.height as i32,
                ));
                dc_track_ids.push(track_id);
                dc_track_ages.push(track_age);
                dc_class_names.push(tmp_detections.class_names[i].clone());
            }

            // Use raw frame (before any drawing) for dataset
            if let Err(err) = collector.process_frame(
                &received.frame,
                &dc_bboxes,
                &dc_class_names,
                &dc_track_ids,
                &dc_track_ages,
            ) {
                println!("[DatasetCollector] Error processing frame: {}", err);
            }
        }

        let ds_guard = ds_tracker.read().expect("DataStorage is poisoned [RWLock]");
        let zones = ds_guard.zones.read().expect("Spatial data is poisoned [RWLock]");
        
        // Reset current occupancy for zones 
        let current_ut = get_sys_time_in_secs();
        for (_, zone_guarded) in zones.iter() {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.current_statistics.occupancy = 0;
            zone.current_statistics.last_time = current_ut;
            zone.current_statistics.last_time_relative = relative_time;
            drop(zone);
        }

        let object_ids: Vec<uuid::Uuid> = tracker.get_engine_objects()
            .iter()
            .filter_map(|(id, obj)| {
                if obj.get_no_match_times() <= 1 {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        
        for object_id in object_ids {
            let object = tracker.get_object(&object_id).unwrap();
            if object.get_no_match_times() > 1 {
                // Skip, since object is lost for a while
                // println!("Object {} is lost for a while", object_id);
                continue;
            }
            let track: &Vec<mot_rs::utils::Point> = object.get_track();
            let track_len = track.len();
            let last_point = &track[track_len - 1];
            let last_point_x = last_point.x;
            let last_point_y = last_point.y;

            let last_before_point = if track_len >= 2 {
                let pt = &track[track_len - 2];
                Some((pt.x, pt.y))
            } else {
                None
            };

            // Get the vehicle's previous zone from tracking if possible
            let previous_zone_id = {
                let vehicle_zones = ds_guard.vehicle_last_zone_cross.read().expect("Vehicle zones is poisoned [RWLock]");
                let result = vehicle_zones.get(&object_id).cloned();
                drop(vehicle_zones);
                result
            };
            let object_extra = tracker.get_object_extra_mut(&object_id).unwrap();
            let times = &object_extra.times;
            let last_time = times[times.len() - 1];
            for (_, zone_guarded) in zones.iter() {
                let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
                if !zone.contains_point(last_point_x, last_point_y) {
                    continue
                }
                zone.current_statistics.occupancy += 1; // Increment current load to match number of objects in zone

                let projected_pt = zone.project_to_skeleton(last_point_x, last_point_y);
                let pixels_per_meters = zone.get_skeleton_ppm();

                let crossed = if let Some(before_point) = last_before_point {
                    zone.crossed_virtual_line(last_point_x, last_point_y, before_point.0, before_point.1)
                } else {
                    false
                };
                // Only provide zone_id_from when vehicle actually crosses the virtual line
                let zone_id_from = if crossed {
                    previous_zone_id.clone()
                } else {
                    None
                };
                match object_extra.spatial_info {
                    Some(ref mut spatial_info) => {
                        spatial_info.update_avg(last_time, last_point_x, last_point_y, projected_pt.0, projected_pt.1, pixels_per_meters);
                        zone.register_or_update_object(object_id, last_time, relative_time, spatial_info.speed, object_extra.get_classname(), crossed, zone_id_from);
                    },
                    None => {
                        object_extra.spatial_info = Some(SpatialInfo::new(last_time, last_point_x, last_point_y, projected_pt.0, projected_pt.1));
                        zone.register_or_update_object(object_id, last_time, relative_time, -1.0, object_extra.get_classname(), crossed, zone_id_from);
                    }
                }
                // Only update vehicle zone tracking when vehicle crosses virtual line
                if crossed {
                    let mut vehicle_zones = ds_guard.vehicle_last_zone_cross.write().expect("Vehicle zones is poisoned [RWLock]");
                    vehicle_zones.insert(object_id, zone.id.clone());
                    drop(vehicle_zones);
                }
                drop(zone);
                // Vehicle can only be in one zone at a time
                break;
            }
        }
        
        if enable_mjpeg || settings.output.enable {
            for (_, v) in zones.iter() {
                let zone = v.lock().expect("Mutex poisoned");
                zone.draw_geom(&mut frame);
                zone.draw_skeleton(&mut frame);
                zone.draw_current_intensity(&mut frame);
                zone.draw_virtual_line(&mut frame);
                drop(zone);
            }
        }

        // We need drop here explicitly, since we need to release lock on zones for MJPEG / REST API / Redis publisher and statistics threads
        drop(zones);
        drop(ds_guard);
        
        /* Imshow + re-stream input video as MJPEG */
        if enable_mjpeg || settings.output.enable {
            draw::draw_track(&mut frame, tracker, &class_colors);
            
            if settings.output.enable {
                match resize(&frame, &mut resized_frame, Size::new(output_width, output_height), 1.0, 1.0, 1) {
                    Ok(_) => {},
                    Err(err) => {
                        panic!("Can't resize output frame due the error {:?}", err);
                    }
                }
                if resized_frame.size()?.width > 0 {
                    imshow(window, &resized_frame)?;
                }
                let key = wait_key(10)?;
                if key == 27 /* esc */ || key == 115 /* s */ || key == 83 /* S */ {
                    break;
                }
            }
        }
        if enable_mjpeg {
            let mut buffer = Vector::<u8>::new();
            // IMWRITE_JPEG_QUALITY = 1, quality value 0-100
            let params = Vector::<i32>::from_slice(&[1, mjpeg_quality]);
            let encoded = imencode(".jpg", &frame, &mut buffer, &params).unwrap();
            if !encoded {
                println!("image has not been encoded");
                continue;
            }
            match tx_mjpeg.send(buffer) {
                Ok(_)=>{},
                Err(_err) => {
                    println!("Error on send frame to MJPEG thread: {}", _err)
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
    let app_settings = AppSettings::new(path_to_config);
    println!("Settings are:\n\t{}", app_settings);

    let mut tracker = new_tracker_from_type(
        &app_settings.tracking.typ.as_deref().unwrap_or("iou_naive")
    );
    println!("Tracker is:\n\t{}", tracker);

    let model_format = match app_settings.detection.get_nn_format() {
        Ok(mf) => mf,
        Err(err) => {
            println!("Can't get model format due the error: {}", err);
            return
        }
    };

    let model_version = match app_settings.detection.get_nn_version() {
        Ok(mf) => mf,
        Err(err) => {
            println!("Can't get model version due the error: {}", err);
            return
        }
    };

    let mut neural_net = match prepare_neural_net(model_format, model_version, &app_settings.detection.network_weights, app_settings.detection.network_cfg.clone(), (app_settings.detection.net_width, app_settings.detection.net_height)) {
        Ok(nn) => nn,
        Err(err) => {
            println!("Can't prepare neural network due the error: {}", err);
            return
        }
    };

    let verbose = match &app_settings.debug {
        Some(x) => { x.enable },
        None => { false }
    };
    
    match run(&app_settings, path_to_config, &mut *tracker, &mut *neural_net, verbose) {
        Ok(_) => {},
        Err(_err) => {
            println!("Error in main thread: {}", _err);
        }
    };
}
