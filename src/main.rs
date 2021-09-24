use opencv::{
    prelude::*,
    core,
    highgui,
    videoio,
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

mod tracking;
use tracking::{
    KalmanBlobie,
    KalmanBlobiesTracker,
};

mod settings;
use settings::{
    AppSettings,
};

mod polygons;
use polygons::{
    ConvexPolygon,
};

fn run() -> opencv::Result<()> {
    let app_settings = AppSettings::new_settings("./data/conf.toml");
    println!("Settings are: {:?}", app_settings);

    let output_width: i32 = app_settings.output.width;
    let output_height: i32 = app_settings.output.height;
    let conf_threshold: f32 = app_settings.detection.conf_threshold;
    let nms_threshold: f32 = app_settings.detection.nms_threshold;
    let max_points_in_track: usize = app_settings.tracking.max_points_in_track;

    const COCO_CLASSNAMES: &'static [&'static str] = &["person", "bicycle", "car", "motorbike", "aeroplane", "bus", "train", "truck", "boat", "traffic light", "fire hydrant", "stop sign", "parking meter", "bench", "bird", "cat", "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra", "giraffe", "backpack", "umbrella", "handbag", "tie", "suitcase", "frisbee", "skis", "snowboard", "sports ball", "kite", "baseball bat", "baseball glove", "skateboard", "surfboard", "tennis racket", "bottle", "wine glass", "cup", "fork", "knife", "spoon", "bowl", "banana", "apple", "sandwich", "orange", "broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair", "sofa", "pottedplant", "bed", "diningtable", "toilet", "tvmonitor", "laptop", "mouse", "remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink", "refrigerator", "book", "clock", "vase", "scissors", "teddy bear", "hair drier", "toothbrush"];
    // const COCO_CLASSNAMES: &'static [&'static str] = &["background", "aeroplane", "bicycle", "bird", "boat", "bottle", "bus", "car", "cat", "chair", "cow", "diningtable", "dog", "horse", "motorbike", "person", "pottedplant", "sheep", "sofa", "train", "tvmonitor"];
    const COCO_FILTERED_CLASSNAMES: &'static [&'static str] = &["car", "motorbike", "bus", "train", "truck"];
    const CLASSES_NUM: usize = COCO_CLASSNAMES.len();
    let default_scalar: core::Scalar = core::Scalar::default();

    // Define default tracker for detected objects (blobs storage)
    let mut tracker = KalmanBlobiesTracker::default();

    let video_src = &app_settings.input.video_src;
    let weights_src = &app_settings.detection.network_weights;
    let cfg_src = &app_settings.detection.network_cfg;
    let network_type = app_settings.detection.network_type.to_lowercase();
    let window = &app_settings.output.window_name;

    let mut convex_polygons = vec![];
    for road_lane in app_settings.road_lanes.iter() {
        convex_polygons.push(road_lane.convert_to_convex_polygon());
    }

    // Prepare output window
    match highgui::named_window(window, 1) {
        Ok(_) => {},
        Err(err) =>{
            panic!("Can't give a name to output window due the error: {:?}", err)
        }
    };
    match highgui::resize_window(window, output_width, output_height) {
        Ok(_) => {},
        Err(err) =>{
            panic!("Can't resize output window due the error: {:?}", err)
        }
    }
    println!("Available <videoio> backends: {:?}", videoio::get_backends()?);

    // Check if CUDA is an option at all
    let cuda_count = core::get_cuda_enabled_device_count()?;
    let cuda_available = cuda_count > 0;
    println!("CUDA is {}", if cuda_available { "available" } else { "not available" });
    
    // Prepare video
    let mut video_capture = match videoio::VideoCapture::from_file(video_src, videoio::CAP_ANY) {
        Ok(result) => {result},
        Err(err) => {
            panic!("Can't init '{}' due the error: {:?}", video_src, err);
        }
    };
    let opened = videoio::VideoCapture::is_opened(&video_capture)?;
    if !opened {
        panic!("Unable to open video '{}'", video_src);
    }

    // Prepare neural network
    let mut neural_net: Net; 
    let blob_scale;
    let net_size;
    let blob_mean;
    let blob_name;

    match network_type.as_ref() {
        "darknet" => {
            neural_net = match read_net(weights_src, cfg_src, "Darknet"){
                Ok(result) => result,
                Err(err) => {
                    panic!("Can't read network '{}' (with cfg '{}') due the error: {:?}", weights_src, cfg_src, err);
                }
            };
            blob_scale = 1.0/255.0;
            net_size = core::Size::new(416, 416);
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
            net_size = core::Size::new(300, 300);
            blob_mean = core::Scalar::from(127.5);
            blob_name = "data";
        },
        _ => {
            panic!("Only this network types are supported: Darknet / Caffe-Mobilenet-SSD. You've provided: '{}'", app_settings.detection.network_type);
        }
    };

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
    
    let mut frame = core::Mat::default();
    let mut resized_frame = core::Mat::default();
    let mut detections = core::Vector::<core::Mat>::new();

    /* Read first frame to determine image width/height */
    match video_capture.read(&mut frame) {
        Ok(_) => {},
        Err(_) => {
            panic!("Can't read first frame");
        }
    };
    let frame_cols = frame.cols() as f32;
    let frame_rows = frame.rows() as f32;

    loop {
        let all_now = Instant::now();

        match video_capture.read(&mut frame) {
            Ok(_) => {},
            Err(_) => {
                println!("Can't read next frame");
                break;
            }
        };

        let elapsed_capture = all_now.elapsed().as_millis() as f32;

        let blobimg = blob_from_image(&frame, blob_scale, net_size, blob_mean, true, false, core::CV_32F);
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
                    tmp_blobs = process_yolo_detections(&detections, conf_threshold, nms_threshold, frame_cols, frame_rows, max_points_in_track, COCO_CLASSNAMES, COCO_FILTERED_CLASSNAMES, CLASSES_NUM);
                } else {
                    /* Caffe's Mobilenet */
                    tmp_blobs = process_mobilenet_detections(&detections, conf_threshold, frame_cols, frame_rows, max_points_in_track, COCO_CLASSNAMES, COCO_FILTERED_CLASSNAMES);
                }

                // Match blobs
                tracker.match_to_existing(&mut tmp_blobs);
                for (_, b) in tracker.objects.iter() {
                    b.draw_track(&mut frame);
                    b.draw_center(&mut frame);
                    b.draw_predicted(&mut frame);
                    b.draw_rectangle(&mut frame);
                    b.draw_class_name(&mut frame);
                }
            }
            Err(err) => {
                println!("Can't process input of neural network due the error {:?}", err);
            }
        }
        let elapsed_detection = 1000.0 / detection_now.elapsed().as_millis() as f32;

        for polygon in convex_polygons.iter() {
            polygon.draw_on_mat(&mut frame);
        }

        match resize(&mut frame, &mut resized_frame, core::Size::new(output_width, output_height), 1.0, 1.0, 1) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't resize output frame due the error {:?}", err);
            }
        }

        if resized_frame.size()?.width > 0 {
            highgui::imshow(window, &mut resized_frame)?;
        }
        let key = highgui::wait_key(10)?;
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

fn process_mobilenet_detections(detections: &core::Vector::<core::Mat>, conf_threshold: f32, frame_cols: f32, frame_rows: f32, max_points_in_track: usize, classes: &'static [&'static str], filtered_classes: &'static [&'static str]) -> Vec<KalmanBlobie> {
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
                    let bbox = core::Rect::new(left, top, width, height);
                    let mut kb = KalmanBlobie::new(&bbox, max_points_in_track);
                    kb.set_class_name(class_name.to_string());
                    tmp_blobs.push(kb);
                }
            }
        }
    }
    return tmp_blobs;
}

fn process_yolo_detections(detections: &core::Vector::<core::Mat>, conf_threshold: f32, nms_threshold: f32, frame_cols: f32, frame_rows: f32, max_points_in_track: usize, classes: &'static [&'static str], filtered_classes: &'static [&'static str], classes_num: usize) -> Vec<KalmanBlobie> {
    let mut tmp_blobs = vec![];
    let outs = detections.len();
    let mut class_names = vec![];
    let mut confidences = core::Vector::<f32>::new();
    let mut bboxes = core::Vector::<core::Rect>::new();
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
                    let bbox = core::Rect::new(left as i32, top as i32, width as i32, height as i32);
                    class_names.push(class_name);
                    confidences.push(confidence);
                    bboxes.push(bbox);
                }
            }
        }
    }
    let mut indices = core::Vector::<i32>::new();
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
                let mut kb = KalmanBlobie::new(&bbox, max_points_in_track);
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
    run().unwrap()
}
