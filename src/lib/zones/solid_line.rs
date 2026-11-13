use crate::lib::constants::EPSILON;
use crate::lib::cv::RawFrame;
use crate::lib::draw::primitives::draw_line_thick;

/// Color used to show a solid line on the video frame and in the GeoJSON the
/// UI reads. RGB, the same yellow the solid line has always been drawn with.
pub const SOLID_LINE_COLOR_RGB: [i16; 3] = [255, 255, 0];

/// A solid line as the UI edits it: two endpoints in image coordinates and the
/// margin the violation rule classifies sides with.
///
/// The settings keep the same data embedded in a zone's `rules.solid_line`,
/// which is what the violation engine reads; this type is the part of that
/// rule that is displayed and can be changed through the REST API.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidLine {
    pub line: [[f32; 2]; 2],
    pub margin_px: f32,
}

impl SolidLine {
    pub fn new(line: [[f32; 2]; 2], margin_px: f32) -> Self {
        SolidLine { line, margin_px }
    }

    /// A line is worth keeping when both endpoints exist and do not coincide:
    /// a zero length segment has no sides for the rule to classify
    pub fn is_valid(&self) -> bool {
        if !self.line.iter().flatten().all(|value| value.is_finite()) {
            return false;
        }
        let dx = self.line[1][0] - self.line[0][0];
        let dy = self.line[1][1] - self.line[0][1];
        (dx * dx + dy * dy).sqrt() > EPSILON
    }

    /// Draws the line over the frame the same way the configuration has always
    /// been shown: a black outline with a yellow line on top.
    pub fn draw_on_frame(&self, img: &mut RawFrame) {
        if !self.is_valid() {
            return;
        }
        let w = img.cols() as usize;
        let h = img.rows() as usize;
        let step = img.step();
        let bytes = img.data_bytes_mut();

        let x1 = self.line[0][0].round() as i32;
        let y1 = self.line[0][1].round() as i32;
        let x2 = self.line[1][0].round() as i32;
        let y2 = self.line[1][1].round() as i32;

        // Black outline. Frame colors are BGR.
        draw_line_thick(bytes, step, w, h, x1, y1, x2, y2, [0, 0, 0], 7);
        // Yellow solid line. Frame colors are BGR.
        draw_line_thick(bytes, step, w, h, x1, y1, x2, y2, [0, 255, 255], 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_normal_line_is_valid() {
        assert!(SolidLine::new([[10.0, 20.0], [30.0, 40.0]], 8.0).is_valid());
    }

    #[test]
    fn a_degenerate_line_is_not_valid() {
        assert!(!SolidLine::new([[10.0, 20.0], [10.0, 20.0]], 8.0).is_valid());
    }

    #[test]
    fn a_non_finite_line_is_not_valid() {
        assert!(!SolidLine::new([[f32::NAN, 20.0], [30.0, 40.0]], 8.0).is_valid());
    }

    #[test]
    fn drawing_a_line_puts_the_yellow_line_on_the_frame() {
        let mut frame = RawFrame::new(100, 100);
        let line = SolidLine::new([[10.0, 50.0], [90.0, 50.0]], 8.0);
        line.draw_on_frame(&mut frame);

        // Frame colors are BGR: yellow is [0, 255, 255]
        let pixel = (50 * 100 + 50) * 3;
        assert_eq!(&frame.data[pixel..pixel + 3], &[0, 255, 255]);
    }

    #[test]
    fn a_degenerate_line_draws_nothing() {
        let mut frame = RawFrame::new(10, 10);
        let line = SolidLine::new([[5.0, 5.0], [5.0, 5.0]], 8.0);
        line.draw_on_frame(&mut frame);
        assert!(frame.data.iter().all(|value| *value == 0));
    }
}
