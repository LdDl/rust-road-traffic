use std::collections::VecDeque;

use crate::lib::spatial::haversine;
use crate::lib::constants::EPSILON;

pub struct ObjectExtra {
    class_name: String,
    confidence: f32,
    // Timestamps along the whole track
    pub times: VecDeque<f32>,
    pub estimated_velocity: f32,
    pub spatial_info: Option<SpatialInfo>,
}

impl ObjectExtra {
    pub fn new(class_name: String, confidence: f32, max_track_len: usize) -> Self {
        Self {
            class_name,
            confidence,
            times: VecDeque::with_capacity(max_track_len),
            estimated_velocity: -1.0,
            spatial_info: None,
        }
    }

    pub fn get_classname(&self) -> String {
        self.class_name.clone()
    }
}

pub struct SpatialInfo {
    pub first_time: f32,
    pub first_x_projected: f32,
    pub first_y_projected: f32,
    pub last_time: f32,
    pub last_lon: f32,
    pub last_lat: f32,
    pub last_x: f32,
    pub last_y: f32,
    pub last_x_projected: f32,
    pub last_y_projected: f32,
    pub distance_traveled: f32,
    pub speed: f32,
}

impl SpatialInfo {
    pub fn new(_time: f32, _x: f32, _y: f32, _x_projected: f32, _y_projected: f32) -> Self {
        Self {
            first_time: _time,
            first_x_projected: _x_projected,
            first_y_projected: _y_projected,
            last_time: _time,
            last_lon: -1.0,
            last_lat: -1.0,
            last_x: _x,
            last_y: _y,
            last_x_projected: _x_projected,
            last_y_projected: _y_projected,
            distance_traveled: -1.0,
            speed: -1.0,
        }
    }

    pub fn new_wgs84(_time: f32, _lon: f32, _lat: f32, _x: f32, _y: f32) -> Self {
        Self {
            first_time: _time,
            first_x_projected: -1.0,
            first_y_projected: -1.0,
            last_time: _time,
            last_lon: _lon,
            last_lat: _lat,
            last_x: _x,
            last_y: _y,
            last_x_projected: -1.0,
            last_y_projected: -1.0,
            distance_traveled: -1.0,
            speed: -1.0,
        }
    }

    // Same as update(), but calculations are done between first and last points
    // This approach helps to avoid situation when distance between two points is approx. 0
    pub fn update_avg(&mut self, _time: f32, _x: f32, _y: f32, _x_projected: f32, _y_projected: f32, pixels_per_meter: f32) {
        // Update position tracking regardless of speed calculation validity
        self.last_time = _time;
        self.last_x = _x;
        self.last_y = _y;
        self.last_x_projected = _x_projected;
        self.last_y_projected = _y_projected;

        // Guard against invalid pixels_per_meter (uninitialized or zero)
        if pixels_per_meter <= 0.0 {
            return;
        }
        let time_diff = (_time - self.first_time).abs();
        // Guard against zero time difference
        if time_diff < EPSILON {
            return;
        }
        let distance_pixels = ((_x_projected - self.first_x_projected).powi(2) + (_y_projected - self.first_y_projected).powi(2)).sqrt();
        let distance_meters = distance_pixels / pixels_per_meter;
        let velocity = distance_meters / time_diff; // meters per second
        self.speed = velocity * 3.6; // convert m/s to km/h
    }

    pub fn update(&mut self, _time: f32, _x: f32, _y: f32, _x_projected: f32, _y_projected: f32, pixels_per_meter: f32) {
        // Capture previous position before updating
        let prev_x_projected = self.last_x_projected;
        let prev_y_projected = self.last_y_projected;
        let prev_time = self.last_time;

        // Update position tracking
        self.last_time = _time;
        self.last_x = _x;
        self.last_y = _y;
        self.last_x_projected = _x_projected;
        self.last_y_projected = _y_projected;

        // Guard against invalid pixels_per_meter (uninitialized or zero)
        if pixels_per_meter <= 0.0 {
            return;
        }
        let time_diff = _time - prev_time;
        // Guard against zero or negative time difference
        if time_diff < EPSILON {
            return;
        }
        let distance_pixels = ((_x_projected - prev_x_projected).powi(2) + (_y_projected - prev_y_projected).powi(2)).sqrt();
        let distance_meters = distance_pixels / pixels_per_meter;
        let velocity = distance_meters / time_diff; // meters per second
        self.speed = velocity * 3.6; // convert m/s to km/h
    }

    fn update_by_wgs84(&mut self, _time: f32, _lon: f32, _lat: f32, _x: f32, _y: f32) {
        let distance = haversine(self.last_lon, self.last_lat, _lon, _lat) * 1000.0;
        let time_diff = _time - self.last_time;
        let velocity = distance / time_diff; // meters per second
        self.distance_traveled = distance;
        self.speed = velocity * 3.6; // convert m/s to km/h

        self.last_time = _time;
        self.last_lon = _lon;
        self.last_lat = _lat;
        self.last_x = _x;
        self.last_y = _y;
    }
}
