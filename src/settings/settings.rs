use std::fs;

use serde_derive::Deserialize;
use toml;

#[derive(Deserialize, Debug)]
pub struct AppSettings {
    pub input: InputSettings,
    pub output: OutputSettings,
    pub detection: DetectionSettings,
    pub tracking: TrackingSettings,
    pub road_lanes: Vec<RoadLanesSettings>,
}

#[derive(Deserialize, Debug)]
pub struct InputSettings {
    pub video_src: String,
}

#[derive(Deserialize, Debug)]
pub struct OutputSettings {
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
pub struct RoadLanesSettings {
    pub lane_number: u16,
    pub lane_direction: u8,
    pub geometry: Vec<[i32; 2]>,
    pub geometry_wgs84: Vec<[f32; 2]>,
    pub color_rgb: [i16; 3],
}

use crate::lib::polygons::ConvexPolygon;
use crate::lib::spatial::SpatialConverter;
use std::collections::HashSet;
use opencv::core::Point;
use opencv::core::Point2f;
use opencv::core::Scalar;
use uuid::Uuid;

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
        return ConvexPolygon{
            id: Uuid::new_v4(),
            coordinates: geom,
            // RGB to OpenCV = [B, G, R]. So use reverse order
            color: Scalar::from((self.color_rgb[2] as f64, self.color_rgb[1] as f64, self.color_rgb[0] as f64)),
            avg_speed: 0.0,
            sum_intensity: 0,
            road_lane_num: self.lane_number,
            road_lane_direction: self.lane_direction,
            spatial_converter: SpatialConverter::new(&geom_f32, &geom_wgs84),
            blobs: HashSet::new(),
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
