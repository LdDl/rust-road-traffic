use opencv::{
    core,
    highgui,
    videoio,
    prelude::*,
    imgcodecs::*,
    imgproc::rectangle,
    imgproc::resize,
    dnn::Net,
    dnn::DNN_BACKEND_CUDA,
    dnn::DNN_TARGET_CUDA,
    dnn::DNN_TARGET_CUDA_FP16,
    dnn::read_net,
    dnn::blob_from_image
};

fn run() -> opencv::Result<()> {
    const OUTPUT_WIDTH: i32 = 500;
    const OUTPUT_HEIGHT: i32 = 500;
    const CONF_THRESHOLD: f32 = 0.3;
    const COCO_CLASSNAMES: &'static [&'static str] = &["person", "bicycle", "car", "motorcycle", "airplane", "bus", "train", "truck", "boat", "traffic light", "fire hydrant", "street sign", "stop sign", "parking meter", "bench", "bird", "cat", "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra", "giraffe", "hat", "backpack", "umbrella", "shoe", "eye glasses", "handbag", "tie", "suitcase", "frisbee", "skis", "snowboard", "sports ball", "kite", "baseball bat", "baseball glove", "skateboard", "surfboard", "tennis racket", "bottle", "plate", "wine glass", "cup", "fork", "knife", "spoon", "bowl", "banana", "apple", "sandwich", "orange", "broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair", "couch", "potted plant", "bed", "mirror", "dining table", "window", "desk", "toilet", "door", "tv", "laptop", "mouse", "remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink", "refrigerator", "blender", "book", "clock", "vase", "scissors", "teddy bear", "hair drier", "toothbrush"];
    let video_src = "./data/sample_960_540.mp4";
    let weights_src = "./data/yolov4-tiny.weights";
    let cfg_src = "./data/yolov4-tiny.cfg";
    let window = "Tiny YOLO v4";

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

    loop {
        match video_capture.read(&mut frame) {
            Ok(_) => {},
            Err(_) => {
                println!("Can't read next frame");
                break;
            }
        };
        match resize(&mut frame, &mut resized_frame, core::Size::new(OUTPUT_WIDTH, OUTPUT_HEIGHT), 1.0, 1.0, 1) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't resize output frame due the error {:?}", err);
            }
        }

        let blobimg = blob_from_image(&frame, 0.00392, core::Size::new(608, 608), core::Scalar::default(), true, false, core::CV_32F);
        match neural_net.set_input(&blobimg.unwrap(), "", 1.0, core::Scalar::default()){
            Ok(_) => {},
            Err(err) => {
                println!("Can't set input of neural network due the error {:?}", err);
            }
        };
        match neural_net.forward(&mut detections, &out_layers_names) {
            Ok(_) => {}
            Err(err) => {
                println!("Can't process input of neural network due the error {:?}", err);
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
