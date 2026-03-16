use std::io::Read;
use std::process::{Child, Command, Stdio};

use crate::lib::cv::RawFrame;

/// Video source metadata.
pub struct VideoCaptureInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub total_frames: f32,
}

/// Errors from video capture.
#[derive(Debug)]
pub enum CaptureError {
    OpenFailed(String),
    ProbeFailed(String),
    ProcessError(String),
    Io(std::io::Error),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::OpenFailed(s) => write!(f, "Failed to open video source: {}", s),
            CaptureError::ProbeFailed(s) => write!(f, "Probe failed: {}", s),
            CaptureError::ProcessError(s) => write!(f, "Process error: {}", s),
            CaptureError::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for CaptureError {}

impl From<std::io::Error> for CaptureError {
    fn from(e: std::io::Error) -> Self {
        CaptureError::Io(e)
    }
}

/// Detected source kind based on `video_src` string.
enum SourceKind {
    File,
    Rtsp,
    Camera(String),
    GStreamer,
}

/// Video capture via ffmpeg or gst-launch-1.0 subprocess.
///
/// Supports file, RTSP, V4L2 camera, and GStreamer pipeline sources.
/// Output pixel format: BGR24, row-major, no padding.
pub struct VideoSource {
    child: Child,
    width: u32,
    height: u32,
    fps: f32,
    total_frames: f32,
    frame_size: usize,
    buf: Vec<u8>,
}

impl VideoSource {
    /// Open a video source. Source type is auto-detected from `video_src`:
    /// - `rtsp://...` / `rtsps://...` means RTSP (ffmpeg, TCP transport)
    /// - Contains `!` => GStreamer pipeline (gst-launch-1.0)
    /// - Parseable as i32 => V4L2 camera `/dev/video{N}` (ffmpeg)
    /// - Starts with `/dev/video` => V4L2 camera (ffmpeg)
    /// - Otherwise => just file (ffmpeg)
    pub fn open(video_src: &str) -> Result<Self, CaptureError> {
        let kind = detect_source_kind(video_src);
        let info = probe_video(video_src, &kind)?;

        println!(
            "Video probe: {{Width: {}px | Height: {}px | FPS: {} | Total frames: {}}}",
            info.width, info.height, info.fps, info.total_frames
        );

        let child = spawn_subprocess(video_src, &kind, &info)?;
        let frame_size = info.width as usize * info.height as usize * 3;

        Ok(Self {
            child,
            width: info.width,
            height: info.height,
            fps: info.fps,
            total_frames: info.total_frames,
            frame_size,
            buf: vec![0u8; frame_size],
        })
    }

    /// Read the next frame. Returns `Ok(None)` on EOF.
    pub fn read_frame(&mut self) -> Result<Option<RawFrame>, CaptureError> {
        let stdout =
            self.child.stdout.as_mut().ok_or_else(|| {
                CaptureError::ProcessError("subprocess stdout not available".into())
            })?;

        let mut total_read = 0;
        while total_read < self.frame_size {
            match stdout.read(&mut self.buf[total_read..self.frame_size]) {
                Ok(0) => return Ok(None), // EOF
                Ok(n) => total_read += n,
                Err(e) => return Err(CaptureError::Io(e)),
            }
        }

        Ok(Some(RawFrame {
            data: self.buf.clone(),
            width: self.width,
            height: self.height,
        }))
    }

    pub fn width(&self) -> f32 {
        self.width as f32
    }

    pub fn height(&self) -> f32 {
        self.height as f32
    }

    pub fn fps(&self) -> f32 {
        self.fps
    }

    pub fn total_frames(&self) -> f32 {
        self.total_frames
    }
}

impl Drop for VideoSource {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn detect_source_kind(src: &str) -> SourceKind {
    if src.starts_with("rtsp://") || src.starts_with("rtsps://") {
        SourceKind::Rtsp
    } else if src.contains('!') {
        SourceKind::GStreamer
    } else if let Ok(idx) = src.parse::<i32>() {
        SourceKind::Camera(format!("/dev/video{}", idx))
    } else if src.starts_with("/dev/video") {
        SourceKind::Camera(src.to_string())
    } else {
        SourceKind::File
    }
}

/// Probe video source for width, height, fps, total_frames.
fn probe_video(video_src: &str, kind: &SourceKind) -> Result<VideoCaptureInfo, CaptureError> {
    match kind {
        SourceKind::GStreamer => probe_gstreamer_pipeline(video_src),
        _ => probe_ffprobe(video_src, kind),
    }
}

/// Probe with ffprobe (file, RTSP, camera).
fn probe_ffprobe(video_src: &str, kind: &SourceKind) -> Result<VideoCaptureInfo, CaptureError> {
    let mut cmd = Command::new("ffprobe");

    match kind {
        SourceKind::Rtsp => {
            cmd.args(["-rtsp_transport", "tcp"]);
        }
        SourceKind::Camera(dev) => {
            cmd.args(["-f", "v4l2"]);
            // Use device path instead of original src for ffprobe
            cmd.args([
                "-v",
                "quiet",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height,r_frame_rate,nb_frames",
                "-of",
                "json",
                dev,
            ]);

            return run_ffprobe(cmd);
        }
        _ => {}
    }

    cmd.args([
        "-v",
        "quiet",
        "-select_streams",
        "v:0",
        "-show_entries",
        "stream=width,height,r_frame_rate,nb_frames",
        "-of",
        "json",
        video_src,
    ]);

    run_ffprobe(cmd)
}

fn run_ffprobe(mut cmd: Command) -> Result<VideoCaptureInfo, CaptureError> {
    let output = cmd
        .output()
        .map_err(|e| CaptureError::ProbeFailed(format!("Failed to run ffprobe: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CaptureError::ProbeFailed(format!(
            "ffprobe exited with {}: {}",
            output.status, stderr
        )));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| CaptureError::ProbeFailed(format!("Failed to parse ffprobe JSON: {}", e)))?;

    let stream = parsed["streams"]
        .as_array()
        .and_then(|s| s.first())
        .ok_or_else(|| CaptureError::ProbeFailed("No video streams found".into()))?;

    let width = stream["width"]
        .as_u64()
        .ok_or_else(|| CaptureError::ProbeFailed("Missing width".into()))? as u32;

    let height = stream["height"]
        .as_u64()
        .ok_or_else(|| CaptureError::ProbeFailed("Missing height".into()))? as u32;

    let fps_str = stream["r_frame_rate"].as_str().unwrap_or("30/1");
    let fps = parse_frame_rate(fps_str);

    // nb_frames may be "N/A" or missing for streams
    let total_frames = stream["nb_frames"]
        .as_str()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(-1.0);

    Ok(VideoCaptureInfo {
        width,
        height,
        fps,
        total_frames,
    })
}

/// Parse width/height/fps from GStreamer pipeline string.
/// Looks for patterns like `width=(int)1280`, `height=(int)720`, `framerate=(fraction)30/1`.
fn probe_gstreamer_pipeline(pipeline: &str) -> Result<VideoCaptureInfo, CaptureError> {
    let width = parse_gst_int(pipeline, "width").ok_or_else(|| {
        CaptureError::ProbeFailed("Cannot parse width from GStreamer pipeline".into())
    })?;

    let height = parse_gst_int(pipeline, "height").ok_or_else(|| {
        CaptureError::ProbeFailed("Cannot parse height from GStreamer pipeline".into())
    })?;

    let fps = parse_gst_fraction(pipeline, "framerate").unwrap_or(30.0);

    Ok(VideoCaptureInfo {
        width,
        height,
        fps,
        total_frames: -1.0,
    })
}

/// Parse `key=(int)VALUE` from GStreamer caps string.
fn parse_gst_int(s: &str, key: &str) -> Option<u32> {
    let pattern = format!("{}=(int)", key);
    let idx = s.find(&pattern)?;
    let after = &s[idx + pattern.len()..];
    let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse().ok()
}

/// Parse `framerate=(fraction)NUM/DEN` from GStreamer caps string.
fn parse_gst_fraction(s: &str, key: &str) -> Option<f32> {
    let pattern = format!("{}=(fraction)", key);
    let idx = s.find(&pattern)?;
    let after = &s[idx + pattern.len()..];
    let frac_str: String = after
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '/')
        .collect();
    parse_frame_rate(&frac_str).into()
}

/// Spawn ffmpeg or gst-launch-1.0 subprocess.
fn spawn_subprocess(
    video_src: &str,
    kind: &SourceKind,
    info: &VideoCaptureInfo,
) -> Result<Child, CaptureError> {
    match kind {
        SourceKind::GStreamer => spawn_gstreamer(video_src, info),
        _ => spawn_ffmpeg(video_src, kind),
    }
}

fn spawn_ffmpeg(video_src: &str, kind: &SourceKind) -> Result<Child, CaptureError> {
    let mut cmd = Command::new("ffmpeg");

    match kind {
        SourceKind::Rtsp => {
            cmd.args(["-rtsp_transport", "tcp"]);
        }
        SourceKind::Camera(dev) => {
            cmd.args(["-f", "v4l2"]);
            cmd.args([
                "-i", dev, "-f", "rawvideo", "-pix_fmt", "bgr24", "-v", "quiet", "-nostdin",
                "pipe:1",
            ]);

            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::null());
            cmd.stdin(Stdio::null());

            return cmd
                .spawn()
                .map_err(|e| CaptureError::OpenFailed(format!("Failed to spawn ffmpeg: {}", e)));
        }
        _ => {}
    }

    cmd.args([
        "-i", video_src, "-f", "rawvideo", "-pix_fmt", "bgr24", "-v", "quiet", "-nostdin", "pipe:1",
    ]);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());

    cmd.spawn()
        .map_err(|e| CaptureError::OpenFailed(format!("Failed to spawn ffmpeg: {}", e)))
}

/// Spawn gst-launch-1.0 with the pipeline, replacing `appsink` with `fdsink fd=1`.
/// The pipeline must output BGR format before the sink element.
fn spawn_gstreamer(pipeline: &str, _info: &VideoCaptureInfo) -> Result<Child, CaptureError> {
    // Replace appsink with fdsink for stdout output
    let mut gst_pipeline = pipeline.to_string();
    if gst_pipeline.contains("appsink") {
        gst_pipeline = gst_pipeline.replace("appsink", "fdsink fd=1");
    } else {
        // Append fdsink if no sink specified
        gst_pipeline.push_str(" ! fdsink fd=1");
    }

    let mut cmd = Command::new("gst-launch-1.0");
    cmd.args(gst_pipeline.split_whitespace());

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());

    cmd.spawn()
        .map_err(|e| CaptureError::OpenFailed(format!("Failed to spawn gst-launch-1.0: {}", e)))
}

/// Parse frame rate string "num/den" into f32.
fn parse_frame_rate(s: &str) -> f32 {
    if let Some((num_str, den_str)) = s.split_once('/') {
        let num: f32 = num_str.parse().unwrap_or(30.0);
        let den: f32 = den_str.parse().unwrap_or(1.0);
        if den > 0.0 { num / den } else { 30.0 }
    } else {
        s.parse().unwrap_or(30.0)
    }
}
