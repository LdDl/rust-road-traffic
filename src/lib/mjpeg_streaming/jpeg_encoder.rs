use turbojpeg::{Compressor, Image, PixelFormat, Subsamp};

/// JPEG encoder using libjpeg-turbo (via turbojpeg crate).
///
/// Uses SIMD acceleration (SSE2/AVX2 on x86, NEON on ARM).
/// Reuses the [`Compressor`] across calls to avoid re-initialization overhead.
pub struct JpegEncoder {
    width: usize,
    height: usize,
    compressor: Compressor,
}

impl JpegEncoder {
    /// Creates a new JPEG encoder.
    ///
    /// # Arguments
    ///
    /// * `width`, `height` - Frame dimensions in pixels.
    /// * `quality` - JPEG quality 1-100 (higher = better quality, larger file).
    pub fn new(width: u32, height: u32, quality: i32) -> Self {
        let mut compressor = Compressor::new().expect("failed to create turbojpeg compressor");
        compressor
            .set_quality(quality)
            .expect("failed to set quality");
        compressor
            .set_subsamp(Subsamp::Sub2x2)
            .expect("failed to set subsamp");

        Self {
            width: width as usize,
            height: height as usize,
            compressor,
        }
    }

    /// Encodes a raw BGR24 frame to JPEG.
    ///
    /// # Arguments
    ///
    /// * `bgr_data` - Raw pixel data in BGR24 format (3 bytes per pixel, row-major).
    pub fn encode(&mut self, bgr_data: &[u8]) -> Result<Vec<u8>, turbojpeg::Error> {
        let image = Image {
            pixels: bgr_data,
            width: self.width,
            pitch: self.width * 3,
            height: self.height,
            format: PixelFormat::BGR,
        };
        self.compressor.compress_to_vec(image)
    }
}
