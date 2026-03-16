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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuda_check_does_not_panic() {
        // Just verify the function runs without panicking on any system
        let _ = is_cuda_available();
    }
}
