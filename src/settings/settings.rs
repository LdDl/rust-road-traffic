use std::fs;

use chrono::Utc;
use serde::{ Deserialize, Serialize };
use toml;
use std::error::Error;
use std::fmt;

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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InputSettings {
    pub video_src: String,
    pub typ: String,
    pub scale_x: Option<f32>,
    pub scale_y: Option<f32>,
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
}

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
    pub max_points_in_track: usize,
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
}

use crate::lib::zones::Zone;
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

        Zone::new(
            format!("dir_{}_lane_{}", setting.lane_direction, setting.lane_number),
            geom,
            geom_epsg4326,
            geom_epsg3857,
            Scalar::from((setting.color_rgb[2] as f64, setting.color_rgb[1] as f64, setting.color_rgb[0] as f64)),
            setting.lane_number,
            setting.lane_direction
        )
    }
}

impl AppSettings {
    pub fn new(filename: &str) -> Self {
        let toml_contents = fs::read_to_string(filename).expect("Something went wrong reading the file");
        let mut app_settings = match toml::from_str::<AppSettings>(&toml_contents) {
            Ok(result) => result,
            Err(err) => {
                panic!("Can't parse TOML configuration file due the error: {:?}", err);
            }
        };
        match app_settings.debug {
            None => {
                app_settings.debug = Some(DebugSettings{
                    enable: false,
                });
            },
            _ => {  }
        }
        match app_settings.input.scale_x {
            None => { 
                app_settings.input.scale_x = Some(1.0);
            }, 
            _ => {  }
        }
        match app_settings.input.scale_y {
            None => { 
                app_settings.input.scale_y = Some(1.0);
            }, 
            _ => {  }
        }
        return app_settings;
    }
    pub fn save(&self, filename: &str) -> Result<(), Box<dyn Error>>{
        fs::copy(filename, filename.to_owned() + &format!(".{}.bak", Utc::now().format("%Y-%m-%dT%H-%M-%S-%f")))?;
        let docs = toml::to_string(self)?;
        fs::write(filename, docs.to_string())?;
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
        }
    }
}

impl fmt::Display for AppSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Equipment ID: {}\n\tVideo input: {}\n\tNetwork weights:{}\n\tNetwork configuration:{:?}\n\tRefresh data (millis): {}\n\tBack-end host: {}\n\tBack-end port: {}",
            self.equipment_info.id,
            self.input.video_src,
            self.detection.network_weights,
            self.detection.network_cfg,
            self.worker.reset_data_milliseconds,
            self.rest_api.host,
            self.rest_api.back_end_port,
        )
    }
}
