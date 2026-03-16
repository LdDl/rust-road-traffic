use crate::lib::draw::primitives::pixel::set_pixel;

/// Draws a 1-pixel wide line using Bresenham's algorithm.
///
/// Handles all octants and clips to image boundaries via [`set_pixel`].
///
/// # Arguments
///
/// * `bytes` - Mutable BGR image buffer.
/// * `step` - Row stride in bytes.
/// * `w`, `h` - Image dimensions in pixels.
/// * `x0`, `y0` - Start point.
/// * `x1`, `y1` - End point.
/// * `color` - BGR color `[B, G, R]`.
///
/// # Example
///
/// ```
/// // 5x2 BGR image buffer
/// let mut buf = vec![0u8; 30];
/// // Draw a horizontal white line across row 0
/// draw_line(&mut buf, 15, 5, 2, 0, 0, 4, 0, [255, 255, 255]);
/// ```
pub fn draw_line(
    bytes: &mut [u8],
    step: usize,
    w: usize,
    h: usize,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 3],
) {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        set_pixel(bytes, step, w, h, x, y, color);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Draws a line with the given pixel thickness.
///
/// Achieved by drawing `thickness` parallel Bresenham lines offset in
/// the perpendicular direction (Y-offset for mostly-horizontal lines,
/// X-offset for mostly-vertical ones).
///
/// # Arguments
///
/// * `bytes` - Mutable BGR image buffer.
/// * `step` - Row stride in bytes.
/// * `w`, `h` - Image dimensions in pixels.
/// * `x0`, `y0` - Start point.
/// * `x1`, `y1` - End point.
/// * `color` - BGR color `[B, G, R]`.
/// * `thickness` - Line width in pixels. Values <= 1 fall back to [`draw_line`].
///
/// # Example
///
/// ```
/// // 10x10 BGR image buffer
/// let mut buf = vec![0u8; 300];
/// // Draw a 2px green horizontal line
/// draw_line_thick(&mut buf, 30, 10, 10, 0, 5, 9, 5, [0, 255, 0], 2);
/// ```
pub fn draw_line_thick(
    bytes: &mut [u8],
    step: usize,
    w: usize,
    h: usize,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: [u8; 3],
    thickness: i32,
) {
    if thickness <= 1 {
        draw_line(bytes, step, w, h, x0, y0, x1, y1, color);
        return;
    }
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let half = thickness / 2;
    if dx >= dy {
        for t in -half..(thickness - half) {
            draw_line(bytes, step, w, h, x0, y0 + t, x1, y1 + t, color);
        }
    } else {
        for t in -half..(thickness - half) {
            draw_line(bytes, step, w, h, x0 + t, y0, x1 + t, y1, color);
        }
    }
}
