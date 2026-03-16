use crate::lib::draw::primitives::pixel::set_pixel;

/// Draws a filled circle using brute-force distance check.
///
/// Every pixel within Euclidean distance `r` from `(cx, cy)` is painted.
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
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                set_pixel(bytes, step, w, h, cx + dx, cy + dy, color);
            }
        }
    }
}
