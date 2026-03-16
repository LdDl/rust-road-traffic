use std::io::Read;
use std::path::Path;
use std::process::Command;

/// Check if a CUDA-capable GPU is available.
///
/// Detection order:
/// 1. `nvidia-smi --query-gpu=count` - desktop Linux with NVIDIA drivers
/// 2. `/dev/nvhost-ctrl-gpu` - NVIDIA Jetson (Tegra) devices
/// 3. `/dev/nvidia0` - fallback for desktop Linux
///
/// # Returns
/// - `true` if at least one CUDA-capable GPU is detected
/// - `false` otherwise
///
/// # Platform notes
/// - Desktop Linux: `nvidia-smi` (installed with NVIDIA driver package)
/// - Jetson Nano/TX2/Xavier/Orin: `/dev/nvhost-ctrl-gpu` (no `nvidia-smi` on Tegra)
/// - Windows: `nvidia-smi` is at `C:\Windows\System32\nvidia-smi.exe` (installed with driver, but I've not tested it)
/// - Does not require CUDA toolkit - only the driver
pub fn is_cuda_available() -> bool {
    // 1. Try nvidia-smi (desktop GPUs)
    if let Ok(output) = Command::new("nvidia-smi")
        .args(["--query-gpu=count", "--format=csv,noheader"])
        .output()
    {
        if output.status.success() {
            let count = String::from_utf8_lossy(&output.stdout)
                .trim()
                .lines()
                .next()
                .and_then(|s| s.trim().parse::<i32>().ok())
                .unwrap_or(0);
            return count > 0;
        }
    }
    // 2. Jetson (Tegra) - GPU controller device
    if Path::new("/dev/nvhost-ctrl-gpu").exists() {
        return true;
    }
    // 3. Desktop Linux fallback
    Path::new("/dev/nvidia0").exists()
}

/// Detected model file format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelFileFormat {
    /// ONNX format (protobuf, contains "onnx" in header)
    Onnx,
    /// Darknet weights (classic YOLOv3/v4/v7 .weights format)
    DarknetWeights,
    /// TensorRT serialized engine
    TensorRtEngine,
    /// Unknown format
    Unknown,
}

impl std::fmt::Display for ModelFileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelFileFormat::Onnx => write!(f, "ONNX"),
            ModelFileFormat::DarknetWeights => write!(f, "Darknet weights"),
            ModelFileFormat::TensorRtEngine => write!(f, "TensorRT engine"),
            ModelFileFormat::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Detect the model file format by examining file contents.
///
/// Detection logic:
/// - ONNX: protobuf format - the string "onnx" appears in the first 4KB (from opset domain "ai.onnx")
/// - Darknet weights: header is 3+ little-endian i32 values; major version = 0, minor = 1 or 2
/// - TensorRT engine: no reliable magic bytes, detected by `.engine` / `.trt` extension
/// - Falls back to `Unknown` if none match
pub fn detect_model_format(path: &str) -> std::io::Result<ModelFileFormat> {
    let mut file = std::fs::File::open(path)?;
    let mut header = [0u8; 4096];
    let bytes_read = file.read(&mut header)?;
    let header = &header[..bytes_read];

    // ONNX: protobuf contains "onnx" (from opset domain "ai.onnx")
    if bytes_read >= 4 && header.windows(4).any(|w| w == b"onnx") {
        return Ok(ModelFileFormat::Onnx);
    }

    // Darknet weights: [major: i32 LE, minor: i32 LE, revision: i32 LE, ...]
    // major = 0, minor = 1 or 2
    if bytes_read >= 12 {
        let major = i32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let minor = i32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        if major == 0 && (minor == 1 || minor == 2) {
            return Ok(ModelFileFormat::DarknetWeights);
        }
    }

    // TensorRT engine: extension-based (no reliable magic bytes)
    if path.ends_with(".engine") || path.ends_with(".trt") {
        return Ok(ModelFileFormat::TensorRtEngine);
    }

    Ok(ModelFileFormat::Unknown)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn cuda_check_does_not_panic() {
        // Just verify the function runs without panicking on any system
        let _ = is_cuda_available();
    }

    #[test]
    fn detect_format_nonexistent_file() {
        let result = detect_model_format("/tmp/nonexistent_model_file_12345.bin");
        assert!(result.is_err());
    }

    #[test]
    fn detect_format_onnx() {
        let path = "/tmp/test_detect_format.onnx";
        let mut data = vec![0u8; 64];
        data[10..17].copy_from_slice(b"ai.onnx");
        fs::write(path, &data).unwrap();
        let fmt = detect_model_format(path).unwrap();
        assert_eq!(fmt, ModelFileFormat::Onnx);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn detect_format_darknet() {
        let path = "/tmp/test_detect_format.weights";
        let mut data = vec![0u8; 64];
        data[0..4].copy_from_slice(&0i32.to_le_bytes()); // major
        data[4..8].copy_from_slice(&2i32.to_le_bytes()); // minor
        data[8..12].copy_from_slice(&0i32.to_le_bytes()); // revision
        fs::write(path, &data).unwrap();
        let fmt = detect_model_format(path).unwrap();
        assert_eq!(fmt, ModelFileFormat::DarknetWeights);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn detect_format_tensorrt_by_extension() {
        let path = "/tmp/test_detect_format.engine";
        let data = vec![0xFFu8; 64];
        fs::write(path, &data).unwrap();
        let fmt = detect_model_format(path).unwrap();
        assert_eq!(fmt, ModelFileFormat::TensorRtEngine);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn detect_format_unknown() {
        let path = "/tmp/test_detect_format.bin";
        let data = vec![0xFFu8; 64];
        fs::write(path, &data).unwrap();
        let fmt = detect_model_format(path).unwrap();
        assert_eq!(fmt, ModelFileFormat::Unknown);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn model_file_format_display() {
        assert_eq!(format!("{}", ModelFileFormat::Onnx), "ONNX");
        assert_eq!(
            format!("{}", ModelFileFormat::DarknetWeights),
            "Darknet weights"
        );
        assert_eq!(
            format!("{}", ModelFileFormat::TensorRtEngine),
            "TensorRT engine"
        );
        assert_eq!(format!("{}", ModelFileFormat::Unknown), "Unknown");
    }
}
