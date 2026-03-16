use std::fmt;
use std::io;

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

#[derive(Debug)]
pub enum DetectorError {
    Io(io::Error),
    Config(String),
    ModelLoad(String),
    UnsupportedFormat(String),
}

impl fmt::Display for DetectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DetectorError::Io(e) => write!(f, "I/O error: {}", e),
            DetectorError::Config(msg) => write!(f, "Configuration error: {}", msg),
            DetectorError::ModelLoad(msg) => write!(f, "Model load error: {}", msg),
            DetectorError::UnsupportedFormat(msg) => write!(f, "Unsupported format: {}", msg),
        }
    }
}

impl std::error::Error for DetectorError {}

impl From<io::Error> for DetectorError {
    fn from(e: io::Error) -> Self {
        DetectorError::Io(e)
    }
}

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
    pub fn new(
        weights: &str,
        net_size: Option<(i32, i32)>,
        network_cfg: Option<&str>,
    ) -> Result<Self, DetectorError> {
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

        let format = utils::detect_model_format(weights)?;
        println!("Detected model format: {}", format);

        let model: Box<dyn ModelTrait> = match format {
            utils::ModelFileFormat::DarknetWeights => {
                let cfg = network_cfg.ok_or_else(|| {
                    DetectorError::Config(format!(
                        "Darknet weights '{}' require a .cfg file. Set 'network_cfg' in config.",
                        weights
                    ))
                })?;
                let cfg_net_size = utils::parse_darknet_cfg_net_size(cfg)?;
                println!(
                    "OpenCV Darknet network input size: {}x{} (from {})",
                    cfg_net_size.0, cfg_net_size.1, cfg
                );
                Model::darknet(cfg, weights, cfg_net_size, dnn_backend, dnn_target)
                    .map(|m| Box::new(m) as Box<dyn ModelTrait>)
                    .map_err(|e| {
                        DetectorError::ModelLoad(format!(
                            "Can't load Darknet network '{}' / '{}': {:?}",
                            cfg, weights, e
                        ))
                    })?
            }
            utils::ModelFileFormat::Onnx => {
                let net_size = net_size.ok_or_else(|| {
                    DetectorError::Config(format!(
                        "ONNX model '{}' requires net_width/net_height in config.",
                        weights
                    ))
                })?;
                println!(
                    "OpenCV ONNX network input size: {}x{}",
                    net_size.0, net_size.1
                );
                Model::opencv(weights, net_size, dnn_backend, dnn_target)
                    .map(|m| Box::new(m) as Box<dyn ModelTrait>)
                    .map_err(|e| {
                        DetectorError::ModelLoad(format!(
                            "Can't load ONNX network '{}': {:?}",
                            weights, e
                        ))
                    })?
            }
            other => {
                return Err(DetectorError::UnsupportedFormat(format!(
                    "'{}' for OpenCV DNN backend. Use .weights (Darknet) or .onnx (ONNX).",
                    other
                )));
            }
        };
        Ok(Detector::OpenCV(model))
    }

    #[cfg(all(feature = "ort-backend", not(feature = "opencv-backend")))]
    pub fn new(
        weights: &str,
        net_size: Option<(i32, i32)>,
        _network_cfg: Option<&str>,
    ) -> Result<Self, DetectorError> {
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
        let net_size = net_size.ok_or_else(|| {
            DetectorError::Config(format!(
                "ONNX model '{}' requires net_width/net_height in config.",
                weights
            ))
        })?;
        println!("ORT ONNX network input size: {}x{}", net_size.0, net_size.1);

        let net_size_u32 = (net_size.0 as u32, net_size.1 as u32);

        #[cfg(feature = "ort-cuda")]
        let model_result = if cuda_available {
            Model::ort_cuda(weights, net_size_u32)
        } else {
            Model::ort(weights, net_size_u32)
        };

        #[cfg(not(feature = "ort-cuda"))]
        let model_result = Model::ort(weights, net_size_u32);

        model_result
            .map(Detector::Ort)
            .map_err(|e| DetectorError::ModelLoad(format!("Can't load ORT model '{}': {:?}", weights, e)))
    }

    #[cfg(all(
        feature = "tensorrt-backend",
        not(feature = "opencv-backend"),
        not(feature = "ort-backend")
    ))]
    pub fn new(
        weights: &str,
        _net_size: Option<(i32, i32)>,
        _network_cfg: Option<&str>,
    ) -> Result<Self, DetectorError> {
        println!("Using TensorRT backend");
        let model = Model::tensorrt(weights).map_err(|e| {
            DetectorError::ModelLoad(format!("Can't load TensorRT model '{}': {:?}", weights, e))
        })?;
        let (w, h) = model.input_size();
        println!(
            "TensorRT network input size: {}x{} (from engine)",
            w, h
        );
        Ok(Detector::TensorRT(model))
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
