use crate::lib::cv::RawFrame;
use crate::lib::cv::Rect as RectCV;
use crate::lib::utils;

#[cfg(feature = "opencv-backend")]
use od_opencv::{DnnBackend, DnnTarget, Model, model::ModelTrait};
#[cfg(feature = "opencv-backend")]
use opencv::{core::Mat, prelude::MatTraitManual};

#[cfg(all(feature = "ort-backend", not(feature = "opencv-backend")))]
use od_opencv::{Model, ModelUltralyticsOrt};

#[cfg(all(
    feature = "tensorrt-backend",
    not(feature = "opencv-backend"),
    not(feature = "ort-backend")
))]
use od_opencv::{Model, ModelUltralyticsRt};

pub enum Detector {
    #[cfg(feature = "opencv-backend")]
    OpenCV(Box<dyn ModelTrait>),

    #[cfg(all(feature = "ort-backend", not(feature = "opencv-backend")))]
    Ort(ModelUltralyticsOrt),

    #[cfg(all(
        feature = "tensorrt-backend",
        not(feature = "opencv-backend"),
        not(feature = "ort-backend")
    ))]
    TensorRT(ModelUltralyticsRt),
}

impl Detector {
    #[cfg(feature = "opencv-backend")]
    pub fn new(weights: &str, net_size: (i32, i32), network_cfg: Option<&str>) -> Self {
        let cuda_available = utils::is_cuda_available();
        println!(
            "CUDA is {}",
            if cuda_available {
                "'available'"
            } else {
                "'not available'"
            }
        );

        let dnn_backend = if cuda_available {
            DnnBackend::Cuda
        } else {
            DnnBackend::OpenCV
        };
        let dnn_target = if cuda_available {
            DnnTarget::Cuda
        } else {
            DnnTarget::Cpu
        };
        println!(
            "Using OpenCV DNN backend with {:?}/{:?}",
            dnn_backend, dnn_target
        );

        let format = utils::detect_model_format(weights)
            .unwrap_or_else(|e| panic!("Can't read weights file '{}': {}", weights, e));
        println!("Detected model format: {}", format);

        let model: Box<dyn ModelTrait> = match format {
            utils::ModelFileFormat::DarknetWeights => {
                let cfg = network_cfg.unwrap_or_else(|| {
                    panic!(
                        "Darknet weights '{}' require a .cfg file. Set 'network_cfg' in config.",
                        weights
                    )
                });
                match Model::darknet(cfg, weights, net_size, dnn_backend, dnn_target) {
                    Ok(model) => Box::new(model),
                    Err(err) => panic!(
                        "Can't read Darknet network '{}' / '{}' due the error: {:?}",
                        cfg, weights, err
                    ),
                }
            }
            utils::ModelFileFormat::Onnx => {
                match Model::opencv(weights, net_size, dnn_backend, dnn_target) {
                    Ok(model) => Box::new(model),
                    Err(err) => panic!(
                        "Can't read ONNX network '{}' due the error: {:?}",
                        weights, err
                    ),
                }
            }
            other => panic!(
                "Unsupported model format '{}' for OpenCV DNN backend. Use .weights (Darknet) or .onnx (ONNX).",
                other
            ),
        };
        Detector::OpenCV(model)
    }

    #[cfg(all(feature = "ort-backend", not(feature = "opencv-backend")))]
    pub fn new(weights: &str, net_size: (i32, i32), _network_cfg: Option<&str>) -> Self {
        let cuda_available = utils::is_cuda_available();
        println!(
            "CUDA is {}",
            if cuda_available {
                "'available'"
            } else {
                "'not available'"
            }
        );

        #[cfg(feature = "ort-cuda")]
        let backend_name = if cuda_available { "CUDA" } else { "CPU" };
        #[cfg(not(feature = "ort-cuda"))]
        let backend_name = "CPU";

        println!("Using ORT backend ({})", backend_name);

        let net_size_u32 = (net_size.0 as u32, net_size.1 as u32);

        #[cfg(feature = "ort-cuda")]
        let model_result = if cuda_available {
            Model::ort_cuda(weights, net_size_u32)
        } else {
            Model::ort(weights, net_size_u32)
        };

        #[cfg(not(feature = "ort-cuda"))]
        let model_result = Model::ort(weights, net_size_u32);

        match model_result {
            Ok(model) => Detector::Ort(model),
            Err(err) => panic!(
                "Can't create ORT model '{}' due the error: {:?}",
                weights, err
            ),
        }
    }

    #[cfg(all(
        feature = "tensorrt-backend",
        not(feature = "opencv-backend"),
        not(feature = "ort-backend")
    ))]
    pub fn new(weights: &str, net_size: (i32, i32), _network_cfg: Option<&str>) -> Self {
        println!("Using TensorRT backend");
        let net_size_u32 = (net_size.0 as u32, net_size.1 as u32);
        match Model::tensorrt(weights, net_size_u32) {
            Ok(model) => Detector::TensorRT(model),
            Err(err) => panic!(
                "Can't create TensorRT model '{}' due the error: {:?}",
                weights, err
            ),
        }
    }

    pub fn detect_frame(
        &mut self,
        frame: &RawFrame,
        conf_threshold: f32,
        nms_threshold: f32,
    ) -> Result<(Vec<RectCV>, Vec<usize>, Vec<f32>), String> {
        match self {
            #[cfg(feature = "opencv-backend")]
            Detector::OpenCV(model) => {
                let mut mat = Mat::new_rows_cols_with_default(
                    frame.rows(),
                    frame.cols(),
                    opencv::core::CV_8UC3,
                    opencv::core::Scalar::all(0.0),
                )
                .map_err(|e| e.to_string())?;
                mat.data_bytes_mut()
                    .map_err(|e| e.to_string())?
                    .copy_from_slice(&frame.data);
                let (rects, ids, confs) = model
                    .forward(&mat, conf_threshold, nms_threshold)
                    .map_err(|e| e.to_string())?;
                let bboxes = rects
                    .iter()
                    .map(|r| RectCV::new(r.x, r.y, r.width, r.height))
                    .collect();
                Ok((bboxes, ids, confs))
            }

            #[cfg(all(feature = "ort-backend", not(feature = "opencv-backend")))]
            Detector::Ort(model) => {
                let img = raw_frame_to_image_buffer(frame);
                let (bboxes, ids, confs) = model
                    .forward(&img, conf_threshold, nms_threshold)
                    .map_err(|e| format!("{:?}", e))?;
                let rects = bboxes
                    .iter()
                    .map(|b| RectCV::new(b.x, b.y, b.width, b.height))
                    .collect();
                Ok((rects, ids, confs))
            }

            #[cfg(all(
                feature = "tensorrt-backend",
                not(feature = "opencv-backend"),
                not(feature = "ort-backend")
            ))]
            Detector::TensorRT(model) => {
                let img = raw_frame_to_image_buffer(frame);
                let (bboxes, ids, confs) = model
                    .forward(&img, conf_threshold, nms_threshold)
                    .map_err(|e| format!("{:?}", e))?;
                let rects = bboxes
                    .iter()
                    .map(|b| RectCV::new(b.x, b.y, b.width, b.height))
                    .collect();
                Ok((rects, ids, confs))
            }
        }
    }
}

/// RawFrame (BGR24) => ImageBuffer for ort/tensorrt backends.
/// Clones data since frame is needed later for drawing.
#[cfg(not(feature = "opencv-backend"))]
fn raw_frame_to_image_buffer(frame: &RawFrame) -> od_opencv::ImageBuffer {
    let arr = ndarray::Array3::from_shape_vec(
        (frame.height as usize, frame.width as usize, 3),
        frame.data.clone(),
    )
    .expect("RawFrame dimensions mismatch");
    od_opencv::ImageBuffer::from_bgr(arr)
}
