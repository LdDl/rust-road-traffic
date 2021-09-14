use opencv::{
    prelude::*,
    core,
    highgui,
    videoio,
    imgproc::resize,
    imgproc::rectangle,
    imgproc::put_text,
    imgproc::FONT_HERSHEY_SIMPLEX,
    imgproc::LINE_8,
    dnn::DNN_BACKEND_CUDA,
    dnn::DNN_TARGET_CUDA,
    dnn::read_net,
    dnn::blob_from_image,
    dnn::nms_boxes
};

use std::time::{Instant};

mod tracking;
use tracking::{
    KalmanWrapper,
    KalmanModelType,
    KalmanBlobie,
    KalmanBlobiesTracker,
};

fn run() -> opencv::Result<()> {
    const OUTPUT_WIDTH: i32 = 500;
    const OUTPUT_HEIGHT: i32 = 500;
    const CONF_THRESHOLD: f32 = 0.1;
    const COCO_CLASSNAMES: &'static [&'static str] = &["person", "bicycle", "car", "motorbike", "aeroplane", "bus", "train", "truck", "boat", "traffic light", "fire hydrant", "stop sign", "parking meter", "bench", "bird", "cat", "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra", "giraffe", "backpack", "umbrella", "handbag", "tie", "suitcase", "frisbee", "skis", "snowboard", "sports ball", "kite", "baseball bat", "baseball glove", "skateboard", "surfboard", "tennis racket", "bottle", "wine glass", "cup", "fork", "knife", "spoon", "bowl", "banana", "apple", "sandwich", "orange", "broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair", "sofa", "pottedplant", "bed", "diningtable", "toilet", "tvmonitor", "laptop", "mouse", "remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink", "refrigerator", "book", "clock", "vase", "scissors", "teddy bear", "hair drier", "toothbrush"];
    const COCO_FILTERED_CLASSNAMES: &'static [&'static str] = &["car", "motorbike", "bus", "train", "truck"];
    const CLASSES_NUM: usize = COCO_CLASSNAMES.len();
    const NMS_THRESHOLD: f32 = 0.3;
    const PICKED_KALMAN_MODEL: KalmanModelType = KalmanModelType::ConstantVelocity;
    const MAX_POINTS_IN_TRACK: usize = 100;

    // Define default tracker for detected objects (blobs storage)
    let tracker = KalmanBlobiesTracker::default();

    let video_src = "./data/sample_960_540.mp4";
    let weights_src = "./data/yolov4-tiny.weights";
    let cfg_src = "./data/yolov4-tiny.cfg";
    let window = "Tiny YOLO v4";

    // Testing Kalman filter
    // let mut kf = KalmanWrapper::new(PICKED_KALMAN_MODEL);
    // // test struggling
    // let xs = vec![311, 312, 313, 311, 311, 312, 312, 313, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 311, 311, 311, 311, 311, 310, 311, 311, 311, 310, 310, 308, 307, 308, 308, 308, 307, 307, 307, 308, 307, 307, 307, 307, 307, 308, 307, 309, 306, 307, 306, 307, 308, 306, 306, 306, 305, 307, 307, 307, 306, 306, 306, 307, 307, 308, 307, 307, 308, 307, 306, 308, 309, 309, 309, 309, 308, 309, 309, 309, 308, 311, 311, 307, 311, 307, 313, 311, 307, 311, 311, 306, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312];
    // let ys = vec![5, 6, 8, 10, 11, 12, 12, 13, 16, 16, 18, 18, 19, 19, 20, 20, 22, 22, 23, 23, 24, 24, 28, 30, 32, 35, 39, 42, 44, 46, 56, 58, 70, 60, 52, 64, 51, 70, 70, 70, 66, 83, 80, 85, 80, 98, 79, 98, 61, 94, 101, 94, 104, 94, 107, 112, 108, 108, 109, 109, 121, 108, 108, 120, 122, 122, 128, 130, 122, 140, 122, 122, 140, 122, 134, 141, 136, 136, 154, 155, 155, 150, 161, 162, 169, 171, 181, 175, 175, 163, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178];
    // for (i, _) in xs.iter().enumerate() {
    //     println!("Step#{}:", i);
    //     let x = xs[i];
    //     let y = ys[i];
    //     println!("\tpoint {} {}", x, y);
    //     let predicted = kf.predict();
    //     println!("\tpredicted {:?}", predicted);
    //     let state = kf.correct(x as f32, y as f32);
    //     println!("\tstate {:?}", state);
    // }
    // return Ok(());

    // Prepare output window
    match highgui::named_window(window, 1) {
        Ok(_) => {},
        Err(err) =>{
            panic!("Can't give a name to output window due the error: {:?}", err)
        }
    };
    match highgui::resize_window(window, OUTPUT_WIDTH, OUTPUT_HEIGHT) {
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
    let mut neural_net = match read_net(weights_src, cfg_src, "Darknet"){
        Ok(result) => result,
        Err(err) => {
            panic!("Can't read network '{}' (with cfg '{}') due the error: {:?}", weights_src, cfg_src, err);
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
        match video_capture.read(&mut frame) {
            Ok(_) => {},
            Err(_) => {
                println!("Can't read next frame");
                break;
            }
        };

        let blobimg = blob_from_image(&frame, 1.0/255.0, core::Size::new(416, 416), core::Scalar::default(), true, false, core::CV_32F);
        match neural_net.set_input(&blobimg.unwrap(), "", 1.0, core::Scalar::default()){
            Ok(_) => {},
            Err(err) => {
                println!("Can't set input of neural network due the error {:?}", err);
            }
        };

        let now = Instant::now();
        match neural_net.forward(&mut detections, &out_layers_names) {
            Ok(_) => {
                let outs = detections.len();
                let mut class_names = vec![];
                let mut confidences = core::Vector::<f32>::new();
                let mut bboxes = core::Vector::<core::Rect>::new();
                for o in 0..outs {
                    let output = detections.get(o).unwrap();
                    let data_ptr = output.data_typed::<f32>().unwrap();
                    for (i, _) in data_ptr.iter().enumerate().step_by(CLASSES_NUM + 5) {
                        let mut class_id = 0 as usize;
                        let mut max_probability = 0.0;
                        for j in 5..(CLASSES_NUM + 5) {
                            if data_ptr[i+j] > max_probability {
                                max_probability = data_ptr[i+j];
                                class_id = (j-5) % CLASSES_NUM;
                            }
                        }
                        let class_name = COCO_CLASSNAMES[class_id];
                        if COCO_FILTERED_CLASSNAMES.contains(&class_name) {
                            let confidence = max_probability * data_ptr[i+4];
                            if confidence > CONF_THRESHOLD {
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
                match nms_boxes(&bboxes, &confidences, CONF_THRESHOLD, NMS_THRESHOLD, &mut indices, 1.0, 0) {
                    Ok(_) => {},
                    Err(err) => {
                        println!("Can't run NMSBoxes on detections due the error {:?}", err);
                    }
                };
                let mut tmp_blobs = vec![];
                for (i, _) in indices.iter().enumerate() {
                    match bboxes.get(i) {
                        Ok(bbox) => {
                            let kb = KalmanBlobie::new(&bbox, PICKED_KALMAN_MODEL, MAX_POINTS_IN_TRACK);
                            tmp_blobs.push(kb);
                            let class_name = class_names[i];
                            let anchor = core::Point::new(bbox.x + 2, bbox.y + 3);
                            match put_text(&mut frame, &class_name, anchor, FONT_HERSHEY_SIMPLEX, 1.5, core::Scalar::from((0.0, 255.0, 255.0)), 2, LINE_8, false) {
                                Ok(_) => {},
                                Err(err) => {
                                    println!("Can't display classname of object due the error {:?}", err);
                                }
                            };
                        },
                        Err(err) => {
                            panic!("Can't extract bbox from filtered bboxes due the error {:?}", err);
                        }
                    }
                }
                for b in tmp_blobs{
                    b.draw_center(&mut frame);
                    b.draw_predicted(&mut frame);
                    b.draw_rectangle(&mut frame);
                }
            }
            Err(err) => {
                println!("Can't process input of neural network due the error {:?}", err);
            }
        }
        println!("Elapes milliseconds to detect object on image: {}", now.elapsed().as_millis());

        match resize(&mut frame, &mut resized_frame, core::Size::new(OUTPUT_WIDTH, OUTPUT_HEIGHT), 1.0, 1.0, 1) {
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
    }
    Ok(())
}

fn main() {
    run().unwrap()
}
