use crate::lib::cv::RawFrame;

pub struct ThreadedFrame {
    pub frame: RawFrame,
    /// Seconds since capture start. Media time (frame index / fps) for files,
    /// wall clock for live sources. f64 so that the interval between two frames
    /// stays exact after days of uptime; f32 loses ~10 ms per step past ~1e5 s.
    pub timestamp: f64,
}
