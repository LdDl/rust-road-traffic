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
    pub lane_number: i32,
    pub lane_direction: i16,
    pub geometry: Vec<[i32; 2]>,
    pub color_rgb: [i16; 3],
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
