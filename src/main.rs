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
    imgcodecs::imencode,
    dnn::DNN_BACKEND_CUDA,
    dnn::DNN_TARGET_CUDA,
    dnn::Net,
    dnn::read_net,
    dnn::blob_from_image,
};

use chrono::{
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
    process_yolo_detections
};

mod settings;
use settings::{
    AppSettings,
};

mod video_capture;
use video_capture::{
    get_video_capture,
    ThreadedFrame
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
use std::error::Error;
use std::error;
use std::fmt;
use ctrlc;

const VIDEOCAPTURE_POS_MSEC: i32 = 0;
const COCO_FILTERED_CLASSNAMES: &'static [&'static str] = &["car", "motorbike", "bus", "train", "truck"];
const BLOB_SCALE: f64 = 1.0 / 255.0;
const BLOB_NAME: &'static str = "";
const EMPTY_FRAMES_LIMIT: u16 = 60;

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

fn probe_video(capture: &mut VideoCapture) ->  Result<(f32, f32, f64), AppError> {
    let fps = capture.get(opencv::videoio::CAP_PROP_FPS)?;
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
    return Ok((frame_cols, frame_rows, fps));
}

fn prepare_neural_net(weights: &str, configuration: &str) -> Result<(Net, Vector<String>), AppError> {
    let mut neural_net = match read_net(weights, configuration, "Darknet"){
        Ok(result) => result,
        Err(err) => {
            panic!("Can't read network '{}' (with cfg '{}') due the error: {:?}", weights, configuration, err);
        }
    };

    let out_layers_names = neural_net.get_unconnected_out_layers_names()?;

    /* Check if CUDA is an option at all */
    let cuda_count = get_cuda_enabled_device_count()?;
    let cuda_available = cuda_count > 0;
    println!("CUDA is {}", if cuda_available { "'available'" } else { "'not available'" });

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

    Ok((neural_net, out_layers_names))
}

fn run(settings_path: &str, settings: &AppSettings, tracker: &mut KalmanBlobiesTracker, neural_net: &mut Net, neural_net_out_layers: Vector<String>, verbose: bool) -> Result<(), AppError> {
    println!("Verbose is '{}'", verbose);
    println!("REST API is '{}'", settings.rest_api.enable);

    let enable_mjpeg = match &settings.rest_api.mjpeg_streaming {
        Some(v) => { v.enable }
        None => { false }
    };
    println!("MJPEG is '{}'", settings.rest_api.enable);

    /* Preprocess spatial data */
    let data_storage = DataStorage::new_with_id(settings.equipment_info.id.clone(), verbose);
    let scale_x = match settings.input.scale_x {
        Some(x) => { x },
        None => { 1.0 }
    };
    let scale_y = match settings.input.scale_y {
        Some(y) => { y },
        None => { 1.0 }
    };
    for road_lane in settings.road_lanes.iter() {
        let mut polygon = road_lane.convert_to_convex_polygon();
        polygon.scale_geom(scale_x, scale_y);    
        polygon.set_target_classes(COCO_FILTERED_CLASSNAMES);
        data_storage.insert_polygon(polygon);
    }

    println!("Press `Ctrl-C` to stop main programm");
    ctrlc::set_handler(move || {
        println!("Ctrl+C has been pressed! Exit in 2 seconds");
        thread::sleep(STDDuration::from_secs(2));
        process::exit(1);
    }).expect("Error setting `Ctrl-C` handler");

    /* Start statistics thread */
    let ds_threaded = data_storage.get_arc_copy();
    {  
        let ds_worker = ds_threaded.clone();
        let reset_time = settings.worker.reset_data_milliseconds;
        thread::spawn(move || {
            DataStorage::start_data_worker(ds_worker, reset_time, verbose);
        });
    }
    
    /* Start REST API if needed */ 
    let (tx_mjpeg, rx_mjpeg) = mpsc::sync_channel(25);
    if settings.rest_api.enable {
        let settings_clone = settings.clone();
        let ds_api = ds_threaded.clone();
        thread::spawn(move || {
            match rest_api::start_rest_api(settings_clone.rest_api.host.clone(), settings_clone.rest_api.back_end_port, ds_api, enable_mjpeg, rx_mjpeg, settings_clone, "") {
                Ok(_) => {},
                Err(err) => {
                    println!("Can't start API due the error: {:?}", err)
                }
            }
        });
    }

    /* Probe video */
    let mut video_capture = get_video_capture(&settings.input.video_src, settings.input.typ.clone());
    let opened = VideoCapture::is_opened(&video_capture).map_err(|err| AppError::from(err))?;
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
    let (tx_capture, rx_capture): (mpsc::SyncSender<ThreadedFrame>, mpsc::Receiver<ThreadedFrame>) = mpsc::sync_channel(25);
    thread::spawn(move || {
        let mut frames_counter = 0.0;
        let mut total_seconds = 0.0;
        let mut empty_frames_countrer: u16 = 0;
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
                frames_counter = 0.0;
            }

            /* Re-stream input video as MJPEG */
            if enable_mjpeg {
                let mut buffer = Vector::<u8>::new();
                let params = Vector::<i32>::new();
                let encoded = imencode(".jpg", &read_frame, &mut buffer, &params).unwrap();
                if !encoded {
                    println!("image has not been encoded");
                    continue;
                }
                match tx_mjpeg.send(buffer) {
                    Ok(_)=>{},
                    Err(_err) => {
                        // Closed channel?
                        // println!("Error on send frame to MJPEG thread: {}", _err)
                    }
                };
            }

            /* Send frame and capture info */
            let frame = ThreadedFrame{
                frame: read_frame,
                current_second: second_fraction,
            };
            match tx_capture.send(frame) {
                Ok(_)=>{},
                Err(_err) => {
                    // Closed channel?
                    // println!("Error on send frame to detection thread: {}", _err)
                }
            };

            if total_seconds >= 2.0 {
                // break
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
    let net_size = Size::new(settings.detection.net_width, settings.detection.net_height);
    let blob_mean: Scalar = Scalar::new(0.0, 0.0, 0.0, 0.0);
    let mut detections = Vector::<Mat>::new();
    let conf_threshold: f32 = settings.detection.conf_threshold;
    let nms_threshold: f32 = settings.detection.nms_threshold;
    let coco_classnames = &settings.detection.net_classes;
    let max_points_in_track: usize = settings.tracking.max_points_in_track;
    let mut resized_frame = Mat::default();
    for received in rx_capture {
        let mut frame = received.frame.clone();
        let blobimg = blob_from_image(&frame, BLOB_SCALE, net_size, blob_mean, true, false, CV_32F)?;
        match neural_net.set_input(&blobimg, BLOB_NAME, 1.0, blob_mean) {
            Ok(_) => {},
            Err(err) => {
                println!("Can't set input of neural network due the error {:?}", err);
            }
        };

        println!("time to detect {}", received.current_second);

        match neural_net.forward(&mut detections, &neural_net_out_layers) {
            Ok(_) => {
                let mut tmp_blobs = process_yolo_detections(
                    &detections,
                    conf_threshold,
                    nms_threshold,
                    width,
                    height,
                    max_points_in_track,
                    &coco_classnames,
                    COCO_FILTERED_CLASSNAMES,
                    received.current_second,
                );
                for (b) in tmp_blobs.iter() {
                    if settings.output.enable {
                        b.draw_rectangle(&mut frame, Scalar::from((0.0, 255.0, 0.0)));
                    }
                }
                if settings.output.enable {
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
            },
            Err(err) => {
                println!("Can't process input of neural network due the error {:?}", err);
            }
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
    let app_settings = AppSettings::new_settings(path_to_config);
    println!("Settings are:\n\t{}", app_settings);

    let mut tracker = KalmanBlobiesTracker::default();
    println!("Tracker is:\n\t{}", tracker);

    let mut neural_net = match prepare_neural_net(&app_settings.detection.network_weights, &app_settings.detection.network_cfg) {
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
    
    match run(path_to_config.clone(), &app_settings, &mut tracker, &mut neural_net.0, neural_net.1, verbose) {
        Ok(_) => {},
        Err(_err) => {
            println!("Error in main thread: {}", _err);
        }
    };
}
