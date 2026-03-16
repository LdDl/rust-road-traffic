use crate::lib::cv::Scalar;

/// Sets a single pixel in a BGR image buffer.
///
/// Pixels outside the image boundaries are silently ignored (no panic).
///
/// # Arguments
///
/// * `bytes` - Mutable image buffer (BGR, 3 bytes per pixel, row-major).
/// * `step` - Row stride in bytes (typically `width * 3` for continuous images).
/// * `w` - Image width in pixels.
/// * `h` - Image height in pixels.
/// * `x` - Horizontal coordinate (column). May be negative (will be clipped).
/// * `y` - Vertical coordinate (row). May be negative (will be clipped).
/// * `color` - BGR pixel value `[B, G, R]`.
///
/// # Example
///
/// ```
/// // 5x2 BGR image buffer
/// let mut buf = vec![0u8; 30];
/// set_pixel(&mut buf, 15, 5, 2, 2, 1, [255, 0, 0]);
/// // check blue channel
/// assert_eq!(buf[1 * 15 + 2 * 3], 255);
/// ```
#[inline]
pub fn set_pixel(
    bytes: &mut [u8],
    step: usize,
    w: usize,
    h: usize,
    x: i32,
    y: i32,
    color: [u8; 3],
) {
    if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
        let offset = y as usize * step + x as usize * 3;
        bytes[offset] = color[0];
        bytes[offset + 1] = color[1];
        bytes[offset + 2] = color[2];
    }
}

/// Sets a single pixel without bounds checking.
///
/// # Safety contract (not `unsafe` - caller must guarantee):
///
/// `x` and `y` must be within `[0, w)` and `[0, h)` respectively.
/// Out-of-bounds coordinates will cause a panic via slice indexing.
/// Use this only after clipping coordinates to the image rectangle.
#[inline]
pub fn set_pixel_unchecked(bytes: &mut [u8], step: usize, x: usize, y: usize, color: [u8; 3]) {
    let offset = y * step + x * 3;
    bytes[offset] = color[0];
    bytes[offset + 1] = color[1];
    bytes[offset + 2] = color[2];
}

/// Converts a [`Scalar`] (BGR `f64[4]`) to a packed BGR byte triple.
///
/// # Arguments
///
/// * `s` - Source color in `Scalar` format (channels: B, G, R, _).
///
/// # Example
///
/// ```
/// use crate::lib::cv::Scalar;
/// let white = Scalar::from((255.0, 255.0, 255.0));
/// assert_eq!(scalar_to_bgr(&white), [255, 255, 255]);
/// ```
#[inline]
pub fn scalar_to_bgr(s: &Scalar) -> [u8; 3] {
    [s[0] as u8, s[1] as u8, s[2] as u8]
}
