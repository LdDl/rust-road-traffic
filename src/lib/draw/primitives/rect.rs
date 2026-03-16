use crate::lib::draw::primitives::pixel::set_pixel;

/// Draws a corner-only rounded rectangle.
///
/// Only the four corners are rendered: each corner consists of a quarter-arc
/// (midpoint circle algorithm) and short straight arms extending from it.
/// The middle portions of the edges are left empty, producing a clean
/// bracket-style bounding box that does not obscure the object.
///
/// ```text
///    ---              ---
///   /                    \
///   |                    |
///
///
///   |                    |
///   \                    /
///    ---              ---
/// ```
///
/// # Arguments
///
/// * `bytes` - Mutable BGR image buffer.
/// * `step`  - Row stride in bytes.
/// * `w`, `h` - Image dimensions in pixels.
/// * `x1`, `y1` - Top-left corner of the bounding box.
/// * `x2`, `y2` - Bottom-right corner of the bounding box.
/// * `r`     - Corner arc radius in pixels.
/// * `color` - BGR color `[B, G, R]`.
/// * `thickness` - Stroke width in pixels.
///
/// # Example
///
/// ```
/// // 100x100 BGR image buffer
/// let mut buf = vec![0u8; 90_000];
/// draw_rounded_rect(&mut buf, 300, 100, 100, 10, 10, 90, 90, 6, [0, 255, 0], 2);
/// ```
pub fn draw_rounded_rect(
    bytes: &mut [u8],
    step: usize,
    w: usize,
    h: usize,
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
    r: usize,
    color: [u8; 3],
    thickness: usize,
) {
    let rect_w = x2.saturating_sub(x1);
    let rect_h = y2.saturating_sub(y1);
    // Arm length: 15% of the shorter side, but at least r
    let arm = (rect_w.min(rect_h) * 15 / 100)
        .max(r)
        .min(rect_w / 2 - r)
        .min(rect_h / 2 - r);

    for t in 0..thickness {
        // Top-left horizontal arm
        let yt = y1 + t;
        if yt < h {
            for x in (x1 + r)..=(x1 + r + arm) {
                set_pixel(bytes, step, w, h, x as i32, yt as i32, color);
            }
        }
        // Top-right horizontal arm
        if yt < h {
            for x in (x2 - r - arm)..=(x2.saturating_sub(r)) {
                set_pixel(bytes, step, w, h, x as i32, yt as i32, color);
            }
        }
        // Bottom-left horizontal arm
        let yb = y2.saturating_sub(t);
        if yb < h && yb > y1 {
            for x in (x1 + r)..=(x1 + r + arm) {
                set_pixel(bytes, step, w, h, x as i32, yb as i32, color);
            }
        }
        // Bottom-right horizontal arm
        if yb < h && yb > y1 {
            for x in (x2 - r - arm)..=(x2.saturating_sub(r)) {
                set_pixel(bytes, step, w, h, x as i32, yb as i32, color);
            }
        }

        // Top-left vertical arm
        let xl = x1 + t;
        if xl < w {
            for y in (y1 + r)..=(y1 + r + arm) {
                set_pixel(bytes, step, w, h, xl as i32, y as i32, color);
            }
        }
        // Bottom-left vertical arm
        if xl < w {
            for y in (y2 - r - arm)..=(y2.saturating_sub(r)) {
                set_pixel(bytes, step, w, h, xl as i32, y as i32, color);
            }
        }
        // Top-right vertical arm
        let xr = x2.saturating_sub(t);
        if xr < w && xr > x1 {
            for y in (y1 + r)..=(y1 + r + arm) {
                set_pixel(bytes, step, w, h, xr as i32, y as i32, color);
            }
        }
        // Bottom-right vertical arm
        if xr < w && xr > x1 {
            for y in (y2 - r - arm)..=(y2.saturating_sub(r)) {
                set_pixel(bytes, step, w, h, xr as i32, y as i32, color);
            }
        }
    }

    // Quarter-circle corners
    draw_quarter_arc(bytes, step, w, h, x1 + r, y1 + r, r, 0, color, thickness);
    draw_quarter_arc(bytes, step, w, h, x2 - r, y1 + r, r, 1, color, thickness);
    draw_quarter_arc(bytes, step, w, h, x2 - r, y2 - r, r, 2, color, thickness);
    draw_quarter_arc(bytes, step, w, h, x1 + r, y2 - r, r, 3, color, thickness);
}

/// Draws a quarter arc using the midpoint circle algorithm.
///
/// * `quadrant`: `0` = top-left, `1` = top-right, `2` = bottom-right, `3` = bottom-left.
/// * `thickness` is achieved by drawing concentric arcs from `r - thickness + 1` to `r`.
fn draw_quarter_arc(
    bytes: &mut [u8],
    step: usize,
    w: usize,
    h: usize,
    cx: usize,
    cy: usize,
    r: usize,
    quadrant: u8,
    color: [u8; 3],
    thickness: usize,
) {
    if r == 0 {
        return;
    }
    let start = r.saturating_sub(thickness.saturating_sub(1));
    for radius in start..=r {
        let mut x = radius as i32;
        let mut y = 0i32;
        let mut err = 1 - x;

        while x >= y {
            let points: [(i32, i32); 2] = match quadrant {
                0 => [(-x, -y), (-y, -x)],
                1 => [(x, -y), (y, -x)],
                2 => [(x, y), (y, x)],
                _ => [(-x, y), (-y, x)],
            };
            for (dx, dy) in points {
                set_pixel(bytes, step, w, h, cx as i32 + dx, cy as i32 + dy, color);
            }
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }
}
