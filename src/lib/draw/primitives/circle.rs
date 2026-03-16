use crate::lib::draw::primitives::pixel::set_pixel;

/// Draws a filled circle using scanline fill.
///
/// For each row within the circle's vertical extent, the horizontal boundary
/// is computed via integer square root, and only the pixels inside the circle
/// are drawn. No per-pixel distance check is performed.
/// Pixels outside the image boundaries are silently clipped.
///
/// # Arguments
///
/// * `bytes` - Mutable BGR image buffer.
/// * `step` - Row stride in bytes.
/// * `w`, `h` - Image dimensions in pixels.
/// * `cx`, `cy` - Circle center.
/// * `r` - Circle radius in pixels.
/// * `color` - BGR color `[B, G, R]`.
///
/// # Example
///
/// ```
/// // 10x10 BGR image buffer
/// let mut buf = vec![0u8; 300];
/// // Draw a red filled circle of radius 3 centered at (5, 5)
/// draw_filled_circle(&mut buf, 30, 10, 10, 5, 5, 3, [0, 0, 255]);
/// ```
pub fn draw_filled_circle(
    bytes: &mut [u8],
    step: usize,
    w: usize,
    h: usize,
    cx: i32,
    cy: i32,
    r: i32,
    color: [u8; 3],
) {
    let r_sq = r * r;
    for dy in -r..=r {
        // Use std `isqrt`, but need to know that it will panic
        // on negative input
        let dx = (r_sq - dy * dy).isqrt();
        for x in (cx - dx)..=(cx + dx) {
            set_pixel(bytes, step, w, h, x, cy + dy, color);
        }
    }
}
