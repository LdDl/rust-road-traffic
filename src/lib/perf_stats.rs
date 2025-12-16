use std::time::{Duration, Instant};

/// Performance statistics for detection pipeline.
/// Accumulates timing data and prints averages every N frames.
pub struct PerfStats {
    /// Number of frames between stats output
    interval: u32,
    /// Current frame count
    frame_count: u32,
    /// Accumulated timings
    inference_total: Duration,
    postprocess_total: Duration,
    tracking_total: Duration,
}

impl PerfStats {
    pub fn new(interval: u32) -> Self {
        PerfStats {
            interval,
            frame_count: 0,
            inference_total: Duration::ZERO,
            postprocess_total: Duration::ZERO,
            tracking_total: Duration::ZERO,
        }
    }

    /// Record timings for a single frame and print stats if interval reached.
    ///
    /// # Arguments
    /// * `inference` - Time for neural_net.forward() (preprocessing + inference + NMS)
    /// * `postprocess` - Time for process_yolo_detections()
    /// * `tracking` - Time for tracker.match_objects()
    pub fn record(&mut self, inference: Duration, postprocess: Duration, tracking: Duration) {
        self.inference_total += inference;
        self.postprocess_total += postprocess;
        self.tracking_total += tracking;
        self.frame_count += 1;

        if self.frame_count >= self.interval {
            self.print_and_reset();
        }
    }

    fn print_and_reset(&mut self) {
        let n = self.frame_count as f64;

        let avg_inference = self.inference_total.as_secs_f64() * 1000.0 / n;
        let avg_postprocess = self.postprocess_total.as_secs_f64() * 1000.0 / n;
        let avg_tracking = self.tracking_total.as_secs_f64() * 1000.0 / n;
        let avg_total = avg_inference + avg_postprocess + avg_tracking;

        // Estimate FPS based on processing time (not including capture/display)
        let estimated_fps = if avg_total > 0.0 { 1000.0 / avg_total } else { 0.0 };

        println!(
            "[PerfStats] Last {} frames avg: inference={:.2}ms, postprocess={:.2}ms, tracking={:.2}ms | total={:.2}ms (~{:.1} FPS)",
            self.frame_count,
            avg_inference,
            avg_postprocess,
            avg_tracking,
            avg_total,
            estimated_fps
        );

        // Reset
        self.frame_count = 0;
        self.inference_total = Duration::ZERO;
        self.postprocess_total = Duration::ZERO;
        self.tracking_total = Duration::ZERO;
    }
}

/// Simple RAII timer that returns elapsed duration when dropped.
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn start() -> Self {
        Timer { start: Instant::now() }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}
