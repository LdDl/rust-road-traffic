use crate::lib::draw::primitives::pixel::set_pixel_unchecked;

/// Draws a 1-pixel wide line using Bresenham's algorithm with Cohen-Sutherland
/// clipping.
///
/// The line endpoints are first clipped to the image rectangle `[0, w) x [0, h)`.
/// If the line is fully outside the image, nothing is drawn. After clipping,
/// pixels are written without per-pixel bounds checks for maximum throughput.
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
    let (cx0, cy0, cx1, cy1) = match clip_line(x0, y0, x1, y1, w as i32, h as i32) {
        Some(c) => c,
        None => return,
    };

    let mut x = cx0;
    let mut y = cy0;
    let dx = (cx1 - cx0).abs();
    let dy = -(cy1 - cy0).abs();
    let sx = if cx0 < cx1 { 1 } else { -1 };
    let sy = if cy0 < cy1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        set_pixel_unchecked(bytes, step, x as usize, y as usize, color);
        if x == cx1 && y == cy1 {
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
/// X-offset for mostly-vertical ones). Each sub-line is independently
/// clipped to the image boundaries.
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

// https://en.wikipedia.org/wiki/Cohen%E2%80%93Sutherland_algorithm
const INSIDE: u8 = 0b0000;
const LEFT: u8 = 0b0001;
const RIGHT: u8 = 0b0010;
const BOTTOM: u8 = 0b0100;
const TOP: u8 = 0b1000;

#[inline]
fn outcode(x: i32, y: i32, w: i32, h: i32) -> u8 {
    let mut code = INSIDE;
    if x < 0 {
        code |= LEFT;
    } else if x >= w {
        code |= RIGHT;
    }
    if y < 0 {
        code |= TOP;
    } else if y >= h {
        code |= BOTTOM;
    }
    code
}

/// Clips a line segment to `[0, w) x [0, h)` using Cohen-Sutherland.
///
/// Returns `Some((x0, y0, x1, y1))` with clipped coordinates, or `None`
/// if the line is entirely outside the rectangle.
fn clip_line(
    mut x0: i32,
    mut y0: i32,
    mut x1: i32,
    mut y1: i32,
    w: i32,
    h: i32,
) -> Option<(i32, i32, i32, i32)> {
    let mut code0 = outcode(x0, y0, w, h);
    let mut code1 = outcode(x1, y1, w, h);

    loop {
        if (code0 | code1) == INSIDE {
            return Some((x0, y0, x1, y1));
        }
        if (code0 & code1) != INSIDE {
            return None;
        }

        let code_out = if code0 != INSIDE { code0 } else { code1 };
        let (x, y);

        if code_out & TOP != 0 {
            x = x0 + (x1 - x0) * (0 - y0) / (y1 - y0);
            y = 0;
        } else if code_out & BOTTOM != 0 {
            x = x0 + (x1 - x0) * (h - 1 - y0) / (y1 - y0);
            y = h - 1;
        } else if code_out & RIGHT != 0 {
            y = y0 + (y1 - y0) * (w - 1 - x0) / (x1 - x0);
            x = w - 1;
        } else {
            y = y0 + (y1 - y0) * (0 - x0) / (x1 - x0);
            x = 0;
        }

        if code_out == code0 {
            x0 = x;
            y0 = y;
            code0 = outcode(x0, y0, w, h);
        } else {
            x1 = x;
            y1 = y;
            code1 = outcode(x1, y1, w, h);
        }
    }
}
