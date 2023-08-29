use opencv::{
    core::Mat,
};

use chrono::{
    DateTime,
    Utc
};

pub struct ThreadedFrame {
    pub frame: Mat,
    pub current_second: f32
}