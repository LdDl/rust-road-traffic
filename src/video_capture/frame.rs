use opencv::{
    core::Mat,
};

use chrono::{
    DateTime,
    Utc
};

pub struct ThreadedFrame {
    pub frame: Mat,
    pub overall_seconds: f32,
    pub current_second: f32
}
