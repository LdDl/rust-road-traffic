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
    /// - GStreamer pipeline: contains ` ! `, starts with known source element, has sink/caps
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
    } else if is_gstreamer_pipeline(src) {
        SourceKind::GStreamer
    } else if let Ok(idx) = src.parse::<i32>() {
        SourceKind::Camera(format!("/dev/video{}", idx))
    } else if src.starts_with("/dev/video") {
        SourceKind::Camera(src.to_string())
    } else {
        SourceKind::File
    }
}

/// Validate that `src` is a GStreamer pipeline.
///
/// Requirements (all must be true):
/// 1. Contains ` ! ` (element separator with spaces)
/// 2. First token (before first ` ! `) is a known GStreamer source element
/// 3. Contains a sink element (`appsink`, `fdsink`, `fakesink`, `filesink`, `autovideosink`)
///    OR caps with `width=(int)` (implicit sink will be appended later)
fn is_gstreamer_pipeline(src: &str) -> bool {
    if !src.contains(" ! ") {
        return false;
    }

    // Known GStreamer source elements for video capture
    const GST_SOURCE_ELEMENTS: &[&str] = &[
        "nvarguscamerasrc",
        "v4l2src",
        "videotestsrc",
        "rtspsrc",
        "uridecodebin",
        "filesrc",
        "souphttpsrc",
        "tcpclientsrc",
        "udpsrc",
        "multifilesrc",
        "rpicamsrc",
        "libcamerasrc",
        "ksvideosrc",
        "avfvideosrc",
        "autovideosrc",
    ];

    // First element of the pipeline (before first ` ! `)
    let first_segment = src.split(" ! ").next().unwrap_or("");
    let first_token = first_segment.split_whitespace().next().unwrap_or("");

    let has_known_source = GST_SOURCE_ELEMENTS.iter().any(|el| first_token == *el);

    if !has_known_source {
        return false;
    }

    // Must have a sink or caps with width=(int) (meaning user specified format)
    const GST_SINK_ELEMENTS: &[&str] =
        &["appsink", "fdsink", "fakesink", "filesink", "autovideosink"];

    let has_sink = GST_SINK_ELEMENTS
        .iter()
        .any(|el| src.split_whitespace().any(|token| token == *el));

    let has_caps = src.contains("width=(int)");

    has_sink || has_caps
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
            if !std::path::Path::new(dev).exists() {
                return Err(CaptureError::ProbeFailed(format!(
                    "Camera device not found: {}",
                    dev
                )));
            }
            cmd.args(["-f", "v4l2"]);
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

    // gst-launch-1.0 parses pipeline elements separated by `!`.
    // Each segment is: element_name [property=value ...] [caps_string]
    // Caps like "video/x-raw, width=(int)640, height=(int)480" must be a single argument.
    let mut cmd = Command::new("gst-launch-1.0");
    // Suppress status messages on stdout
    cmd.arg("-q");

    for (i, segment) in gst_pipeline.split(" ! ").enumerate() {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if i > 0 {
            cmd.arg("!");
        }
        // Split segment into tokens, but keep caps (containing parentheses) as one arg.
        // Example: "v4l2src device=/dev/video0" → ["v4l2src", "device=/dev/video0"]
        // Example: "video/x-raw, format=(string)YUY2, width=(int)1280" → one arg
        if segment.contains("=(") {
            // Caps filter: remove spaces after commas so gst-launch-1.0 parses it as one token.
            // "video/x-raw, format=(string)BGR, width=(int)640" → "video/x-raw,format=(string)BGR,width=(int)640"
            let caps = segment.replace(", ", ",");
            cmd.arg(caps);
        } else {
            cmd.args(segment.split_whitespace());
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    // just helper for test assertions (SourceKind doesn't derive Debug)
    fn source_kind_name(kind: &SourceKind) -> &'static str {
        match kind {
            SourceKind::File => "File",
            SourceKind::Rtsp => "Rtsp",
            SourceKind::Camera(_) => "Camera",
            SourceKind::GStreamer => "GStreamer",
        }
    }

    #[test]
    fn gst_nvarguscamerasrc_appsink() {
        let p = "nvarguscamerasrc sensor-id=0 ! video/x-raw(memory:NVMM), width=(int)1280, height=(int)720, format=(string)NV12, framerate=(fraction)30/1 ! nvvidconv flip-method=0 ! video/x-raw, width=(int)1280, height=(int)720, format=(string)BGRx ! videoconvert ! video/x-raw, format=(string)BGR ! appsink";
        assert!(is_gstreamer_pipeline(p));
    }

    #[test]
    fn gst_v4l2src_appsink() {
        let p = "v4l2src device=/dev/video0 ! video/x-raw, width=(int)640, height=(int)480 ! videoconvert ! appsink";
        assert!(is_gstreamer_pipeline(p));
    }

    #[test]
    fn gst_videotestsrc_fakesink() {
        let p = "videotestsrc ! video/x-raw, width=(int)320, height=(int)240 ! fakesink";
        assert!(is_gstreamer_pipeline(p));
    }

    #[test]
    fn gst_v4l2src_fdsink() {
        let p = "v4l2src device=/dev/video0 ! videoconvert ! video/x-raw, format=(string)BGR, width=(int)640, height=(int)480 ! fdsink fd=1";
        assert!(is_gstreamer_pipeline(p));
    }

    #[test]
    fn gst_uridecodebin_autovideosink() {
        let p = "uridecodebin uri=file:///tmp/test.mp4 ! videoconvert ! autovideosink";
        assert!(is_gstreamer_pipeline(p));
    }

    #[test]
    fn gst_rpicamsrc_appsink() {
        let p = "rpicamsrc ! video/x-raw, width=(int)1280, height=(int)720 ! appsink";
        assert!(is_gstreamer_pipeline(p));
    }

    #[test]
    fn gst_libcamerasrc_appsink() {
        let p = "libcamerasrc ! video/x-raw, width=(int)640, height=(int)480 ! appsink";
        assert!(is_gstreamer_pipeline(p));
    }

    #[test]
    fn gst_caps_without_explicit_sink() {
        // No sink element, but has width=(int) caps — valid (sink appended by spawn_gstreamer)
        let p = "v4l2src device=/dev/video0 ! video/x-raw, width=(int)640, height=(int)480 ! videoconvert";
        assert!(is_gstreamer_pipeline(p));
    }

    // Invalid: not GStreamer
    #[test]
    fn not_gst_regular_file() {
        assert!(!is_gstreamer_pipeline("./data/video.mp4"));
    }

    #[test]
    fn not_gst_file_with_exclamation_no_spaces() {
        assert!(!is_gstreamer_pipeline("./data/my!video.mp4"));
    }

    #[test]
    fn not_gst_file_with_spaced_exclamation() {
        // Has ` ! ` but first token is not a known source element
        assert!(!is_gstreamer_pipeline("./data/my ! video.mp4"));
    }

    #[test]
    fn not_gst_rtsp_with_exclamation_in_password() {
        assert!(!is_gstreamer_pipeline(
            "rtsp://user:p@ss!word@192.168.1.1:554/stream"
        ));
    }

    #[test]
    fn not_gst_empty_string() {
        assert!(!is_gstreamer_pipeline(""));
    }

    #[test]
    fn not_gst_just_exclamation() {
        assert!(!is_gstreamer_pipeline(" ! "));
    }

    #[test]
    fn not_gst_unknown_source_with_sink() {
        // Has ` ! ` and appsink, but source is not in allowlist
        assert!(!is_gstreamer_pipeline("hackersrc ! appsink"));
    }

    #[test]
    fn not_gst_known_source_no_sink_no_caps() {
        // Known source but no sink and no width=(int) caps
        assert!(!is_gstreamer_pipeline("v4l2src ! videoconvert"));
    }

    #[test]
    fn not_gst_source_in_middle() {
        // v4l2src is not the first token
        assert!(!is_gstreamer_pipeline("someprefix v4l2src ! appsink"));
    }

    #[test]
    fn not_gst_camera_number() {
        assert!(!is_gstreamer_pipeline("0"));
    }

    #[test]
    fn not_gst_dev_video() {
        assert!(!is_gstreamer_pipeline("/dev/video0"));
    }

    #[test]
    fn detect_rtsp() {
        assert!(matches!(
            detect_source_kind("rtsp://192.168.1.1:554/stream"),
            SourceKind::Rtsp
        ));
    }

    #[test]
    fn detect_rtsps() {
        assert!(matches!(
            detect_source_kind("rtsps://secure.cam/live"),
            SourceKind::Rtsp
        ));
    }

    #[test]
    fn detect_rtsp_with_credentials() {
        assert!(matches!(
            detect_source_kind("rtsp://admin:p@ss!w0rd@10.0.0.1/h264"),
            SourceKind::Rtsp
        ));
    }

    #[test]
    fn detect_camera_number() {
        match detect_source_kind("0") {
            SourceKind::Camera(dev) => assert_eq!(dev, "/dev/video0"),
            other => panic!("expected Camera, got {:?}", source_kind_name(&other)),
        }
    }

    #[test]
    fn detect_camera_number_2() {
        match detect_source_kind("2") {
            SourceKind::Camera(dev) => assert_eq!(dev, "/dev/video2"),
            other => panic!("expected Camera, got {:?}", source_kind_name(&other)),
        }
    }

    #[test]
    fn detect_camera_dev_path() {
        match detect_source_kind("/dev/video1") {
            SourceKind::Camera(dev) => assert_eq!(dev, "/dev/video1"),
            other => panic!("expected Camera, got {:?}", source_kind_name(&other)),
        }
    }

    #[test]
    fn detect_file_mp4() {
        assert!(matches!(
            detect_source_kind("./data/video.mp4"),
            SourceKind::File
        ));
    }

    #[test]
    fn detect_file_absolute() {
        assert!(matches!(
            detect_source_kind("/home/user/video.avi"),
            SourceKind::File
        ));
    }

    #[test]
    fn detect_gstreamer() {
        let p =
            "v4l2src device=/dev/video0 ! video/x-raw, width=(int)640, height=(int)480 ! appsink";
        assert!(matches!(detect_source_kind(p), SourceKind::GStreamer));
    }

    #[test]
    fn detect_file_not_gstreamer_despite_exclamation() {
        // Has `!` but not ` ! ` with spaces and not a known source
        assert!(matches!(
            detect_source_kind("./video!test.mp4"),
            SourceKind::File
        ));
    }

    #[test]
    fn fps_fraction_30_1() {
        assert!((parse_frame_rate("30/1") - 30.0).abs() < 0.01);
    }

    #[test]
    fn fps_fraction_25_1() {
        assert!((parse_frame_rate("25/1") - 25.0).abs() < 0.01);
    }

    #[test]
    fn fps_fraction_30000_1001() {
        assert!((parse_frame_rate("30000/1001") - 29.97).abs() < 0.01);
    }

    #[test]
    fn fps_plain_number() {
        assert!((parse_frame_rate("60") - 60.0).abs() < 0.01);
    }

    #[test]
    fn fps_invalid_fallback() {
        assert!((parse_frame_rate("N/A") - 30.0).abs() < 0.01);
    }

    #[test]
    fn fps_zero_denominator() {
        assert!((parse_frame_rate("30/0") - 30.0).abs() < 0.01);
    }

    #[test]
    fn fps_empty() {
        assert!((parse_frame_rate("") - 30.0).abs() < 0.01);
    }

    #[test]
    fn gst_parse_width() {
        let caps = "video/x-raw, width=(int)1280, height=(int)720";
        assert_eq!(parse_gst_int(caps, "width"), Some(1280));
    }

    #[test]
    fn gst_parse_height() {
        let caps = "video/x-raw, width=(int)1280, height=(int)720";
        assert_eq!(parse_gst_int(caps, "height"), Some(720));
    }

    #[test]
    fn gst_parse_missing_key() {
        let caps = "video/x-raw, width=(int)1280";
        assert_eq!(parse_gst_int(caps, "height"), None);
    }

    #[test]
    fn gst_parse_framerate() {
        let caps = "video/x-raw, framerate=(fraction)30/1";
        let fps = parse_gst_fraction(caps, "framerate").unwrap();
        assert!((fps - 30.0).abs() < 0.01);
    }

    #[test]
    fn gst_parse_framerate_missing() {
        let caps = "video/x-raw, width=(int)640";
        assert!(parse_gst_fraction(caps, "framerate").is_none());
    }
}
