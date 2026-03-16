use crate::lib::cv::RawFrame;

pub struct ThreadedFrame {
    pub frame: RawFrame,
    pub overall_seconds: f32,
    pub current_second: f32,
}
