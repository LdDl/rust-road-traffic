use opencv::{
    core,
    prelude::*,
    videoio,
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

fn main() {
    println!("Here will be opencv stuff soon");
}
