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
    dnn::read_net_from_tensorflow,
    dnn::blob_from_image
};

fn run() -> opencv::Result<()> {
    const OUTPUT_WIDTH: i32 = 500;
    const OUTPUT_HEIGHT: i32 = 500;
    let video_src = "./data/sample_960_540.mp4";
    let window = "Tiny YOLO v4";
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

    Ok(())
}

fn main() {
    run().unwrap()
}
