use crate::lib::draw::primitives::pixel::set_pixel;

/// Renders a string using a built-in 4x5 bitmap font at the given scale.
///
/// Each glyph is 4 pixels wide and 5 pixels tall (base size). The `scale`
/// parameter multiplies every pixel, so `scale = 2` produces 8x10 glyphs,
/// `scale = 3` produces 12x15, etc. Character spacing is `1 * scale` pixels.
///
/// Characters without a glyph are rendered as a `3 * scale`-pixel wide space.
/// Supported characters: `A-Z` (case-insensitive), `0-9`, and `` -_.,:/()[] ``.
///
/// # Arguments
///
/// * `bytes` - Mutable BGR image buffer.
/// * `step`  - Row stride in bytes.
/// * `w`, `h` - Image dimensions in pixels.
/// * `start_x`, `start_y` - Top-left origin of the first character.
/// * `text`  - The string to render.
/// * `color` - BGR color `[B, G, R]`.
/// * `scale` - Integer scaling factor. `1` = base 4x5, `2` = 8x10, etc.
///
/// # Example
///
/// ```
/// // 200x20 BGR image buffer
/// let mut buf = vec![0u8; 12_000];
/// // scale=1: tiny 4x5 text
/// draw_text(&mut buf, 600, 200, 20, 2, 2, "HELLO", [255, 255, 255], 1);
/// // scale=3: large 12x15 text
/// draw_text(&mut buf, 600, 200, 20, 2, 2, "HI", [0, 255, 0], 3);
/// ```
pub fn draw_text(
    bytes: &mut [u8],
    step: usize,
    w: usize,
    h: usize,
    start_x: i32,
    start_y: i32,
    text: &str,
    color: [u8; 3],
    scale: i32,
) {
    let scale = scale.max(1);
    let mut cursor_x = start_x;
    for ch in text.chars() {
        if let Some(glyph) = bitmap_glyph(ch) {
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..4 {
                    if bits & (1 << (3 - col)) != 0 {
                        // Fill a scale x scale block for this pixel
                        for sy in 0..scale {
                            for sx in 0..scale {
                                let px = cursor_x + col * scale + sx;
                                let py = start_y + row as i32 * scale + sy;
                                set_pixel(bytes, step, w, h, px, py, color);
                            }
                        }
                    }
                }
            }
            // 4px glyph + 1px spacing, scaled
            cursor_x += 5 * scale;
        } else {
            // unknown char = space, scaled
            cursor_x += 3 * scale;
        }
    }
}

/// Returns a 4-wide x 5-tall bitmap glyph for the given character.
///
/// Each `u8` in the returned array represents one row, with the 4
/// most-significant bits encoding pixel columns left-to-right
/// (bit 3 = leftmost, bit 0 = rightmost).
fn bitmap_glyph(ch: char) -> Option<[u8; 5]> {
    Some(match ch {
        '0' => [0b0110, 0b1001, 0b1001, 0b1001, 0b0110],
        '1' => [0b0010, 0b0110, 0b0010, 0b0010, 0b0111],
        '2' => [0b0110, 0b1001, 0b0010, 0b0100, 0b1111],
        '3' => [0b0110, 0b1001, 0b0010, 0b1001, 0b0110],
        '4' => [0b1010, 0b1010, 0b1111, 0b0010, 0b0010],
        '5' => [0b1111, 0b1000, 0b1110, 0b0001, 0b1110],
        '6' => [0b0110, 0b1000, 0b1110, 0b1001, 0b0110],
        '7' => [0b1111, 0b0001, 0b0010, 0b0100, 0b0100],
        '8' => [0b0110, 0b1001, 0b0110, 0b1001, 0b0110],
        '9' => [0b0110, 0b1001, 0b0111, 0b0001, 0b0110],
        'a' | 'A' => [0b0110, 0b1001, 0b1111, 0b1001, 0b1001],
        'b' | 'B' => [0b1110, 0b1001, 0b1110, 0b1001, 0b1110],
        'c' | 'C' => [0b0110, 0b1001, 0b1000, 0b1001, 0b0110],
        'd' | 'D' => [0b1110, 0b1001, 0b1001, 0b1001, 0b1110],
        'e' | 'E' => [0b1111, 0b1000, 0b1110, 0b1000, 0b1111],
        'f' | 'F' => [0b1111, 0b1000, 0b1110, 0b1000, 0b1000],
        'g' | 'G' => [0b0110, 0b1000, 0b1011, 0b1001, 0b0110],
        'h' | 'H' => [0b1001, 0b1001, 0b1111, 0b1001, 0b1001],
        'i' | 'I' => [0b1110, 0b0100, 0b0100, 0b0100, 0b1110],
        'j' | 'J' => [0b0001, 0b0001, 0b0001, 0b1001, 0b0110],
        'k' | 'K' => [0b1001, 0b1010, 0b1100, 0b1010, 0b1001],
        'l' | 'L' => [0b1000, 0b1000, 0b1000, 0b1000, 0b1111],
        'm' | 'M' => [0b1001, 0b1111, 0b1111, 0b1001, 0b1001],
        'n' | 'N' => [0b1001, 0b1101, 0b1111, 0b1011, 0b1001],
        'o' | 'O' => [0b0110, 0b1001, 0b1001, 0b1001, 0b0110],
        'p' | 'P' => [0b1110, 0b1001, 0b1110, 0b1000, 0b1000],
        'q' | 'Q' => [0b0110, 0b1001, 0b1001, 0b1010, 0b0101],
        'r' | 'R' => [0b1110, 0b1001, 0b1110, 0b1010, 0b1001],
        's' | 'S' => [0b0111, 0b1000, 0b0110, 0b0001, 0b1110],
        't' | 'T' => [0b1111, 0b0100, 0b0100, 0b0100, 0b0100],
        'u' | 'U' => [0b1001, 0b1001, 0b1001, 0b1001, 0b0110],
        'v' | 'V' => [0b1001, 0b1001, 0b1001, 0b0110, 0b0110],
        'w' | 'W' => [0b1001, 0b1001, 0b1111, 0b1111, 0b1001],
        'x' | 'X' => [0b1001, 0b0110, 0b0110, 0b0110, 0b1001],
        'y' | 'Y' => [0b1001, 0b1001, 0b0110, 0b0100, 0b0100],
        'z' | 'Z' => [0b1111, 0b0010, 0b0100, 0b1000, 0b1111],
        '-' => [0b0000, 0b0000, 0b1111, 0b0000, 0b0000],
        '_' => [0b0000, 0b0000, 0b0000, 0b0000, 0b1111],
        '.' => [0b0000, 0b0000, 0b0000, 0b0000, 0b0100],
        ',' => [0b0000, 0b0000, 0b0000, 0b0010, 0b0100],
        ':' => [0b0000, 0b0100, 0b0000, 0b0100, 0b0000],
        '(' => [0b0010, 0b0100, 0b0100, 0b0100, 0b0010],
        ')' => [0b0100, 0b0010, 0b0010, 0b0010, 0b0100],
        '[' => [0b0110, 0b0100, 0b0100, 0b0100, 0b0110],
        ']' => [0b0110, 0b0010, 0b0010, 0b0010, 0b0110],
        '/' => [0b0001, 0b0010, 0b0100, 0b1000, 0b0000],
        ' ' => [0b0000, 0b0000, 0b0000, 0b0000, 0b0000],
        _ => return None,
    })
}
