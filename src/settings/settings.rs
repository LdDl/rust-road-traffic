use std::fs;

use serde_derive::Deserialize;
use toml;

#[derive(Deserialize, Debug)]
pub struct AppSettings {
    pub output: OutputSettings,
    pub detection: DetectionSettings,
    pub tracking: TrackingSettings,
}

#[derive(Deserialize, Debug)]
pub struct OutputSettings {
    pub width: i32,
    pub height: i32,
}

#[derive(Deserialize, Debug)]
pub struct DetectionSettings {
    pub conf_threshold: f32,
    pub nms_threshold: f32,
}

#[derive(Deserialize, Debug)]
pub struct TrackingSettings {
    pub max_points_in_track: usize,
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