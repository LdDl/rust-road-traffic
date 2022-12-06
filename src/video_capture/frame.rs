use opencv::{
    core::Mat,
};

use chrono::{
    DateTime,
    Utc
};

pub struct ThreadedFrame {
    pub frame: Mat,
    pub last_time: DateTime<Utc>,
    pub sec_diff: f64,
    pub capture_millis: f32,
}