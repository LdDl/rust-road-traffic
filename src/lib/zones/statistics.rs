use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;

#[derive(Debug)]
pub struct VehicleTypeParameters {
    pub avg_speed: f32,
    pub sum_intensity: u32,
    pub avg_headway: f32,
}

impl VehicleTypeParameters {
    pub fn default() -> Self {
        VehicleTypeParameters {
            avg_speed: -1.0,
            sum_intensity: 0,
            avg_headway: 0.0,
        }
    }
}

#[derive(Debug)]
pub struct Statistics {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub vehicles_data: HashMap<String, VehicleTypeParameters>,
}

impl Statistics {
    pub fn default() -> Self {
        Statistics {
            period_start: TimeZone::with_ymd_and_hms(&Utc, 1970, 1, 1, 0, 0, 0).unwrap(),
            period_end: TimeZone::with_ymd_and_hms(&Utc, 1970, 1, 1, 0, 0, 0).unwrap(),
            vehicles_data: HashMap::new(),
        }
    }
}

