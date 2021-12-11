use std::fs;

use serde_derive::Deserialize;
use toml;

#[derive(Deserialize, Debug)]
pub struct AppSettings {
    pub input: InputSettings,
    pub output: OutputSettings,
    pub detection: DetectionSettings,
    pub tracking: TrackingSettings,
    pub equipment_info: EquipmentInfo,
    pub road_lanes: Vec<RoadLanesSettings>,
    pub worker: WorkerSettings,
    pub rest_api: RestAPISettings,
    pub redis_publisher: RedisPublisherSettings
}

#[derive(Deserialize, Debug)]
pub struct InputSettings {
    pub video_src: String,
    pub typ: String,
}

#[derive(Deserialize, Debug)]
pub struct OutputSettings {
    pub enable: bool,
    pub width: i32,
    pub height: i32,
    pub window_name: String,
}

#[derive(Deserialize, Debug)]
pub struct DetectionSettings {
    pub network_weights: String,
    pub network_cfg: String,
    pub network_type: String,
    pub conf_threshold: f32,
    pub nms_threshold: f32,
}

#[derive(Deserialize, Debug)]
pub struct TrackingSettings {
    pub max_points_in_track: usize,
}

#[derive(Deserialize, Debug)]
pub struct EquipmentInfo {
    pub id: String,
}

#[derive(Deserialize, Debug)]
pub struct RoadLanesSettings {
    pub lane_number: u16,
    pub lane_direction: u8,
    pub geometry: Vec<[i32; 2]>,
    pub geometry_wgs84: Vec<[f32; 2]>,
    pub color_rgb: [i16; 3],
}

#[derive(Deserialize, Debug)]
pub struct WorkerSettings {
    pub reset_data_milliseconds: u64,
}

#[derive(Deserialize, Debug)]
pub struct RestAPISettings {
    pub host: String,
    pub back_end_port: i32,
    pub api_scope: String,
}

#[derive(Deserialize, Debug)]
pub struct RedisPublisherSettings {
    pub enable: bool,
    pub host: String,
    pub port: i32,
    pub password: String,
    pub db_index: i32,
    pub channel_name: String,
}

use crate::lib::polygons::ConvexPolygon;
use crate::lib::spatial::SpatialConverter;
use std::collections::HashSet;
use std::collections::HashMap;
use opencv::core::Point;
use opencv::core::Point2f;
use opencv::core::Scalar;
use chrono::Utc;

impl RoadLanesSettings {
    pub fn convert_to_convex_polygon(&self) -> ConvexPolygon{
        let geom = self.geometry
            .iter()
            .map(|pt| Point::new(pt[0], pt[1]))
            .collect();
        let geom_f32 = self.geometry
            .iter()
            .map(|pt| Point2f::new(pt[0] as f32, pt[1] as f32))
            .collect();
        let geom_wgs84 = self.geometry_wgs84
            .iter()
            .map(|pt| Point2f::new(pt[0], pt[1]))
            .collect();

        let mut geojson_poly = vec![];
        let mut poly_element = vec![];
        for v in self.geometry_wgs84.iter() {
            poly_element.push(vec![v[0], v[1]]);
        }
        poly_element.push(vec![self.geometry_wgs84[0][0], self.geometry_wgs84[0][1]]);
        geojson_poly.push(poly_element);

        return ConvexPolygon{
            id: format!("dir_{}_lane_{}", self.lane_direction, self.lane_number),
            coordinates: geom,
            coordinates_wgs84: geojson_poly,
            // RGB to OpenCV = [B, G, R]. So use reverse order
            color: Scalar::from((self.color_rgb[2] as f64, self.color_rgb[1] as f64, self.color_rgb[0] as f64)),
            avg_speed: -1.0,
            sum_intensity: 0,
            estimated_avg_speed: 0.0,
            estimated_sum_intensity: 0,
            road_lane_num: self.lane_number,
            road_lane_direction: self.lane_direction,
            spatial_converter: SpatialConverter::new(&geom_f32, &geom_wgs84),
            blobs: HashSet::new(),
            statistics: HashMap::new(),
            period_start: Utc::now(),
            period_end: None,
        }
    }
}

impl AppSettings {
    pub fn new_settings(filename: &str) -> Self {
        let toml_contents = fs::read_to_string(filename).expect("Something went wrong reading the file");
        let app_settings = match toml::from_str::<AppSettings>(&toml_contents) {
            Ok(result) => result,
            Err(err) => {
                panic!("Can't parse TOML configuration file due the error: {:?}", err);
            }
        };
        return app_settings;
    }
}

use std::fmt;
impl fmt::Display for AppSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Equipment ID: {}\n\tVideo input: {}\n\tNetwork type:{}\n\tNetwork configuration:{}\n\tNetwork weights:{}\n\tRefresh data (millis): {}\n\tBack-end host: {}\n\tBack-end port: {}",
            self.equipment_info.id,
            self.input.video_src,
            self.detection.network_type,
            self.detection.network_weights,
            self.detection.network_cfg,
            self.worker.reset_data_milliseconds,
            self.rest_api.host,
            self.rest_api.back_end_port,
        )
    }
}