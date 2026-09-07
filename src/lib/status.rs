//! What the running application can tell about itself, for `GET /api/status`
use std::sync::RwLock;
use std::time::Instant;

use serde::Serialize;
use utoipa::ToSchema;

/// Video source and how many frames are getting through
#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct InputStatus {
    /// The `video_src` from the configuration
    pub video_src: String,
    /// "live" for a camera or a stream, "file" for a video file
    pub kind: String,
    pub width: u32,
    pub height: u32,
    /// Frame rate the source reports
    pub fps: f32,
    /// Frames in the file, -1 for a live source
    pub total_frames: f32,
    /// Only every N-th decoded frame is processed
    pub process_every_nth_frame: u64,
    /// Frames the detector has processed since start
    pub frames_processed: u64,
    /// Frames dropped because the detector was busy. Live sources only
    pub frames_dropped: u64,
    /// Frames per second the detector actually keeps up with, measured over
    /// the last second. `null` until the first second is over
    pub processing_fps: Option<f32>,
    /// Timestamp of the last processed frame, in seconds since capture start
    pub last_frame_at: Option<f64>,
}

/// Inference backend and model
#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct DetectionStatus {
    /// "opencv", "ort" or "tensorrt"
    pub backend: String,
    /// Whether a CUDA device was found at startup
    pub cuda_available: bool,
    pub model: String,
    pub net_width: Option<i32>,
    pub net_height: Option<i32>,
    /// How long the last frame took, in milliseconds
    pub inference_ms: Option<f32>,
    pub postprocess_ms: Option<f32>,
    pub tracking_ms: Option<f32>,
}

/// Tracker in use
#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct TrackingStatus {
    /// Human-readable description, the same one that goes into the log at startup
    pub description: String,
}

/// Everything the app learns about itself while it runs.
///
/// Shared between the detection loop, which writes, and the REST API, which
/// reads: the two live in different threads and neither can reach the other's
/// locals
#[derive(Debug)]
pub struct RuntimeStatus {
    started: Instant,
    input: RwLock<InputStatus>,
    detection: RwLock<DetectionStatus>,
    tracking: RwLock<TrackingStatus>,
    /// Frames counted towards the current one-second rate window
    rate_window: RwLock<RateWindow>,
}

#[derive(Debug)]
struct RateWindow {
    started: Instant,
    frames: u64,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeStatus {
    pub fn new() -> Self {
        let now = Instant::now();
        RuntimeStatus {
            started: now,
            input: RwLock::new(InputStatus::default()),
            detection: RwLock::new(DetectionStatus::default()),
            tracking: RwLock::new(TrackingStatus::default()),
            rate_window: RwLock::new(RateWindow {
                started: now,
                frames: 0,
            }),
        }
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    /// Records what the video source turned out to be, once it is open
    pub fn set_input(&self, input: InputStatus) {
        if let Ok(mut current) = self.input.write() {
            *current = input;
        }
    }

    pub fn set_detection(&self, detection: DetectionStatus) {
        if let Ok(mut current) = self.detection.write() {
            *current = detection;
        }
    }

    pub fn set_tracking(&self, description: String) {
        if let Ok(mut current) = self.tracking.write() {
            current.description = description;
        }
    }

    /// Records a processed frame: its timings, the timestamp it carried and the
    /// total number of frames dropped so far. The processing rate is measured
    /// over one-second windows rather than over the whole uptime, so that it
    /// reflects what the detector is doing now
    pub fn frame_processed(
        &self,
        timestamp: f64,
        dropped_total: u64,
        inference_ms: f32,
        postprocess_ms: f32,
        tracking_ms: f32,
    ) {
        let measured_fps = self.tick_rate();
        if let Ok(mut input) = self.input.write() {
            input.frames_processed += 1;
            input.frames_dropped = dropped_total;
            input.last_frame_at = Some(timestamp);
            if let Some(fps) = measured_fps {
                input.processing_fps = Some(fps);
            }
        }
        if let Ok(mut detection) = self.detection.write() {
            detection.inference_ms = Some(inference_ms);
            detection.postprocess_ms = Some(postprocess_ms);
            detection.tracking_ms = Some(tracking_ms);
        }
    }

    /// Counts the frame into the current window and closes the window once a
    /// second has passed, returning the rate it measured
    fn tick_rate(&self) -> Option<f32> {
        let mut window = self.rate_window.write().ok()?;
        window.frames += 1;
        let elapsed = window.started.elapsed();
        if elapsed.as_secs_f32() < 1.0 {
            return None;
        }
        let fps = window.frames as f32 / elapsed.as_secs_f32();
        window.started = Instant::now();
        window.frames = 0;
        Some(fps)
    }

    pub fn input(&self) -> InputStatus {
        self.input.read().map(|i| i.clone()).unwrap_or_default()
    }

    pub fn detection(&self) -> DetectionStatus {
        self.detection.read().map(|d| d.clone()).unwrap_or_default()
    }

    pub fn tracking(&self) -> TrackingStatus {
        self.tracking.read().map(|t| t.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_timings_are_recorded() {
        let status = RuntimeStatus::new();
        status.set_input(InputStatus {
            video_src: "x.mp4".to_string(),
            kind: "file".to_string(),
            ..Default::default()
        });
        status.frame_processed(0.5, 3, 7.0, 0.5, 0.1);
        status.frame_processed(1.0, 4, 8.0, 0.5, 0.1);

        let input = status.input();
        assert_eq!(input.video_src, "x.mp4");
        assert_eq!(input.frames_processed, 2);
        assert_eq!(input.frames_dropped, 4);
        assert_eq!(input.last_frame_at, Some(1.0));
        // Less than a second has passed, so there is no rate to report yet
        assert_eq!(input.processing_fps, None);

        let detection = status.detection();
        assert_eq!(detection.inference_ms, Some(8.0));
        assert_eq!(detection.tracking_ms, Some(0.1));
    }

    #[test]
    fn rate_is_measured_once_the_window_is_over() {
        let status = RuntimeStatus::new();
        status.frame_processed(0.0, 0, 1.0, 0.0, 0.0);
        assert_eq!(status.input().processing_fps, None);
        std::thread::sleep(std::time::Duration::from_millis(1050));
        status.frame_processed(1.0, 0, 1.0, 0.0, 0.0);
        let fps = status
            .input()
            .processing_fps
            .expect("a full window must report a rate");
        assert!(
            (fps - 2.0).abs() < 0.5,
            "two frames in about a second, got {fps}"
        );
    }
}
