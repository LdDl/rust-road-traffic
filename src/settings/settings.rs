use std::fs;

use chrono::Utc;
use serde::{ Deserialize, Serialize };
use toml;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

// model_format is only available with opencv-backend
#[cfg(feature = "opencv-backend")]
use od_opencv::model_format::{ModelFormat, ModelVersion};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppSettings {
    pub input: InputSettings,
    pub debug: Option<DebugSettings>,
    pub output: OutputSettings,
    pub detection: DetectionSettings,
    pub tracking: TrackingSettings,
    pub equipment_info: EquipmentInfo,
    pub road_lanes: Vec<RoadLanesSettings>,
    pub worker: WorkerSettings,
    pub rest_api: RestAPISettings,
    pub redis_publisher: RedisPublisherSettings,
    pub dataset_collector: Option<DatasetCollectorSettings>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InputSettings {
    pub video_src: String,
    pub typ: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DebugSettings {
    pub enable: bool
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OutputSettings {
    pub enable: bool,
    pub width: i32,
    pub height: i32,
    pub window_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DetectionSettings {
    pub network_ver: Option<i32>,
    pub network_format: Option<String>,
    pub network_weights: String,
    pub network_cfg: Option<String>,
    pub conf_threshold: f32,
    pub nms_threshold: f32,
    pub net_width: i32,
    pub net_height: i32,
    pub net_classes: Vec<String>,
    pub target_classes: Option<Vec<String>>,
    /// Inference backend: "ort" for ONNX Runtime, "opencv" for OpenCV DNN.
    /// Default is "opencv".
    pub inference_backend: Option<String>,
    /// Print performance stats every N frames. 0 = disabled.
    #[serde(default)]
    pub perf_stats_interval: u32,
}

// These methods are only available with opencv-backend
#[cfg(feature = "opencv-backend")]
impl DetectionSettings {
    pub fn get_nn_format(&self) -> Result<ModelFormat,  Box<dyn Error>> {
        match self.network_format.clone() {
            Some(mf) => {
                match mf.to_lowercase().as_str() {
                    "darknet" => { Ok(ModelFormat::Darknet) },
                    "onnx" => { Ok(ModelFormat::ONNX) },
                    _ => {
                        return Err(format!("Can't prepare neural network due the unhandled format: {}", mf).into());
                    }
                }
            },
            None => { Ok(ModelFormat::Darknet) }
        }
    }
    pub fn get_nn_version(&self) -> Result<ModelVersion,  Box<dyn Error>> {
        match self.network_ver.clone() {
            Some(mv) => {
                match mv {
                    3 => { Ok(ModelVersion::V3) },
                    4 => { Ok(ModelVersion::V4) },
                    7 => { Ok(ModelVersion::V7) },
                    8 => { Ok(ModelVersion::V8) },
                    _ => {
                        return Err(format!("Can't prepare neural network due the unhandled version: {}", mv).into());
                    }
                }
            },
            None => { Ok(ModelVersion::V3) }
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TrackingSettings {
    // Either "bytetrack" or "iou_naive". Default is "iou_naive"
    #[serde(alias = "type")]
    pub typ: Option<String>,
    pub max_points_in_track: usize,
    // Either "centroid" or "bbox". Default is "centroid"
    pub kalman_filter: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EquipmentInfo {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoadLanesSettings {
    pub lane_number: u16,
    pub lane_direction: u8,
    pub geometry: Vec<[i32; 2]>,
    pub geometry_wgs84: Vec<[f32; 2]>,
    pub color_rgb: [i16; 3],
    pub virtual_line: Option<VirtualLineSettings>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VirtualLineSettings {
    pub geometry: [[i32; 2]; 2],
    pub color_rgb: [i16; 3],
    // 'lrtb' stands for "left->right, top->bottom"
    // 'rlbt' stands for "right->left, bottom->top"
    pub direction: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkerSettings {
    pub reset_data_milliseconds: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RestAPISettings {
    pub enable: bool,
    pub host: String,
    pub back_end_port: i32,
    pub api_scope: String,
    pub mjpeg_streaming: Option<MJPEGStreamingSettings>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RedisPublisherSettings {
    pub enable: bool,
    pub host: String,
    pub port: i32,
    pub password: String,
    pub db_index: i32,
    pub channel_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MJPEGStreamingSettings {
    pub enable: bool,
    /// JPEG quality for streaming (0-100). Lower = smaller files, faster streaming.
    /// Default is 80
    #[serde(default = "default_mjpeg_quality")]
    pub quality: i32,
}

fn default_mjpeg_quality() -> i32 { 80 }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DatasetCollectorSettings {
    pub enabled: bool,
    pub output_dir: String,
    /// Label format: "yolo" for standard YOLO format (class_id center_x center_y width height)
    #[serde(default = "default_label_format")]
    pub label_format: String,
    /// Minimum number of frames a track must exist before capturing
    #[serde(default = "default_min_track_age")]
    pub min_track_age: u32,
    /// Skip objects whose bounding box touches frame edges
    #[serde(default = "default_skip_edge_objects")]
    pub skip_edge_objects: bool,
    /// Margin in pixels to consider as "edge"
    #[serde(default = "default_edge_margin_pixels")]
    pub edge_margin_pixels: u32,
    /// Maximum number of captures per unique track ID
    #[serde(default = "default_max_captures_per_track")]
    pub max_captures_per_track: u32,
    /// Frames between captures for the same track (when max_captures_per_track > 1)
    #[serde(default = "default_capture_interval")]
    pub capture_interval: u32,
}

fn default_label_format() -> String { "yolo".to_string() }
fn default_min_track_age() -> u32 { 15 }
fn default_skip_edge_objects() -> bool { true }
fn default_edge_margin_pixels() -> u32 { 5 }
fn default_max_captures_per_track() -> u32 { 1 }
fn default_capture_interval() -> u32 { 30 }

use crate::lib::zones::Zone;
use crate::lib::zones::{VirtualLineDirection, VirtualLine};
use crate::lib::spatial::epsg::lonlat_to_meters;
use opencv::core::Point2f;
use opencv::core::Scalar;
use std::convert::From;

impl From<&RoadLanesSettings> for Zone {
    fn from(setting: &RoadLanesSettings) -> Self {
        let geom = setting.geometry
            .iter()
            .map(|pt| Point2f::new(pt[0] as f32, pt[1] as f32))
            .collect();

        let geom_epsg4326 = setting.geometry_wgs84
            .iter()
            .map(|pt| Point2f::new(pt[0], pt[1]))
            .collect();

        let geom_epsg3857 = setting.geometry_wgs84
            .iter()
            .map(|pt| {
                let lonlat = lonlat_to_meters(pt[0], pt[1]);
                Point2f::new(lonlat.0, lonlat.1)
            })
            .collect();

        let virtual_line = match &setting.virtual_line {
            Some(vl) => {
                if vl.geometry.len() != 2{
                    None
                } else {
                    let dir = VirtualLineDirection::from_str(&vl.direction).unwrap_or_default();
                    let a = Point2f::new(vl.geometry[0][0] as f32, vl.geometry[0][1] as f32);
                    let b = Point2f::new(vl.geometry[1][0] as f32, vl.geometry[1][1] as f32);
                    let mut line = VirtualLine::new_from_cv(a, b, dir);
                    line.set_color_rgb(vl.color_rgb[0], vl.color_rgb[1], vl.color_rgb[2]);
                    Some(line)
                }
            },
            None => {
                None
            }
        };

        Zone::new(
            format!("dir_{}_lane_{}", setting.lane_direction, setting.lane_number),
            geom,
            geom_epsg4326,
            geom_epsg3857,
            Scalar::from((setting.color_rgb[2] as f64, setting.color_rgb[1] as f64, setting.color_rgb[0] as f64)),
            setting.lane_number,
            setting.lane_direction,
            virtual_line
        )
    }
}

impl AppSettings {
    pub fn new(filename: &str) -> Self {
        let toml_contents = fs::read_to_string(filename).expect(&format!("Something went wrong reading the file: '{}'", &filename));
        let mut app_settings = match toml::from_str::<AppSettings>(&toml_contents) {
            Ok(result) => result,
            Err(err) => {
                panic!("Can't parse TOML configuration file due the error: {:?}", err);
            }
        };
        // Set default values
        if app_settings.tracking.typ.is_none() {
            app_settings.tracking.typ = Some("iou_naive".to_string());
        }
        if app_settings.tracking.kalman_filter.is_none() {
            app_settings.tracking.kalman_filter = Some("centroid".to_string());
        }
        // Check if tracker type is valid
        if app_settings.tracking.typ.is_some() {
            match app_settings.tracking.typ.as_ref().unwrap().as_str() {
                "iou_naive" => { },
                "bytetrack" => { },
                _ => {
                    panic!("Invalid tracker type: '{}'. Supported types are 'iou_naive' and 'bytetrack'.", app_settings.tracking.typ.as_ref().unwrap());
                }
            }
        }
        // Check if kalman filter type is valid
        if app_settings.tracking.kalman_filter.is_some() {
            match app_settings.tracking.kalman_filter.as_ref().unwrap().as_str() {
                "centroid" => { },
                "bbox" => { },
                _ => {
                    panic!("Invalid kalman filter type: '{}'. Supported types are 'centroid' and 'bbox'.", app_settings.tracking.kalman_filter.as_ref().unwrap());
                }
            }
        }
        match app_settings.debug {
            None => {
                app_settings.debug = Some(DebugSettings{
                    enable: false,
                });
            },
            _ => {  }
        }
        return app_settings;
    }
    pub fn save(&self, filename: &str) -> Result<(), Box<dyn Error>>{
        fs::copy(filename, filename.to_owned() + &format!(".{}.bak", Utc::now().format("%Y-%m-%dT%H-%M-%S-%f")))?;
        let docs = toml::to_string(self)?;
        fs::write(filename, docs)?;
        Ok(())
    }
    pub fn get_copy_no_roads(&self) -> AppSettings {
        AppSettings{
            input: self.input.clone(),
            debug: self.debug.clone(),
            output: self.output.clone(),
            detection: self.detection.clone(),
            tracking: self.tracking.clone(),
            equipment_info: self.equipment_info.clone(),
            road_lanes: Vec::new(),
            worker: self.worker.clone(),
            rest_api: self.rest_api.clone(),
            redis_publisher: self.redis_publisher.clone(),
            dataset_collector: self.dataset_collector.clone(),
        }
    }
}

impl fmt::Display for AppSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Equipment ID: {}\n\tVideo input: {}\n\tNetwork weights:{}\n\tNetwork configuration:{:?}\n\tTracker type:{}\n\tRefresh data (millis): {}\n\tBack-end host: {}\n\tBack-end port: {}",
            self.equipment_info.id,
            self.input.video_src,
            self.detection.network_weights,
            self.detection.network_cfg,
            self.tracking.typ.as_ref().unwrap_or(&"undefined".to_string()),
            self.worker.reset_data_milliseconds,
            self.rest_api.host,
            self.rest_api.back_end_port,
        )
    }
}
