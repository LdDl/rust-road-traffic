use std::fmt;

use crate::lib::cv::RawFrame;
use std::str::FromStr;

use crate::lib::constants::EPSILON;
use crate::lib::cv::Scalar;
use crate::lib::draw::primitives::{draw_line_thick, scalar_to_bgr};
use crate::lib::spatial::Point2f;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VirtualLineDirection {
    Inbound,
    Outbound,
}

impl fmt::Display for VirtualLineDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VirtualLineDirection::Inbound => write!(f, "inbound"),
            VirtualLineDirection::Outbound => write!(f, "outbound"),
        }
    }
}

impl Default for VirtualLineDirection {
    fn default() -> Self {
        VirtualLineDirection::Inbound
    }
}

impl FromStr for VirtualLineDirection {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "inbound" => Ok(VirtualLineDirection::Inbound),
            "outbound" => Ok(VirtualLineDirection::Outbound),
            // Legacy support for old config files
            "lrtb" => Ok(VirtualLineDirection::Inbound),
            "rlbt" => Ok(VirtualLineDirection::Outbound),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
pub struct VirtualLine {
    pub line: [[i32; 2]; 2],
    pub line_cvf: [Point2f; 2],
    pub color_cv: Scalar,
    pub color: [i16; 3],
    pub direction: VirtualLineDirection,
}

impl VirtualLine {
    pub fn new_from_cv(a: Point2f, b: Point2f, _direction: VirtualLineDirection) -> Self {
        VirtualLine {
            line: [[a.x as i32, a.y as i32], [b.x as i32, b.y as i32]],
            line_cvf: [a, b],
            color_cv: Scalar::from((0.0, 0.0, 0.0)),
            color: [0, 0, 0],
            direction: _direction,
        }
    }
    pub fn new_from(ab: [[i32; 2]; 2], _direction: VirtualLineDirection) -> Self {
        VirtualLine {
            line: ab,
            line_cvf: [
                Point2f::new(ab[0][0] as f32, ab[0][1] as f32),
                Point2f::new(ab[1][0] as f32, ab[1][1] as f32),
            ],
            color_cv: Scalar::from((0.0, 0.0, 0.0)),
            color: [0, 0, 0],
            direction: _direction,
        }
    }
    pub fn set_color_rgb(&mut self, r: i16, g: i16, b: i16) {
        self.color_cv = Scalar::from((b as f64, g as f64, r as f64)); // BGR
        self.color = [r, g, b];
    }
    // is_left returns true if the given point is to the left side of the vertical AB or if the given point is above of the horizontal AB
    // Points exactly on the line (within epsilon) are treated as "not left" for consistency
    pub fn is_left(&self, cx: f32, cy: f32) -> bool {
        let a = self.line_cvf[0];
        let b = self.line_cvf[1];
        let cross = (b.x - a.x) * (cy - a.y) - (b.y - a.y) * (cx - a.x);
        // Use small epsilon to handle floating-point imprecision on boundary
        cross > EPSILON
    }
    pub fn clone(&self) -> Self {
        VirtualLine {
            line: self.line,
            line_cvf: self.line_cvf,
            color_cv: self.color_cv,
            color: self.color,
            direction: self.direction,
        }
    }
    pub fn draw_on_frame(&self, img: &mut RawFrame) {
        let w = img.cols() as usize;
        let h = img.rows() as usize;
        let step = img.step();
        let bytes = img.data_bytes_mut();
        draw_line_thick(
            bytes,
            step,
            w,
            h,
            self.line[0][0],
            self.line[0][1],
            self.line[1][0],
            self.line[1][1],
            scalar_to_bgr(&self.color_cv),
            2,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_vertical_line() {
        let vertical_line = VirtualLine::new_from_cv(
            Point2f::new(4.0, 3.0),
            Point2f::new(5.0, 10.0),
            VirtualLineDirection::Inbound,
        );
        let c = Point2f::new(3.0, 8.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_left);

        let c = Point2f::new(5.0, 10.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_left);

        let c = Point2f::new(4.0, 3.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_left);

        let c = Point2f::new(3.9, 3.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_left);

        let c = Point2f::new(5.1, 4.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_left);

        let c = Point2f::new(35.1, 19.2);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_left);

        let c = Point2f::new(-5.0, 8.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_left);

        let c = Point2f::new(6.0, -4.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_left);

        let c = Point2f::new(-2.0, -3.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_left);
    }
    #[test]
    fn test_horizontal_line() {
        let vertical_line = VirtualLine::new_from_cv(
            Point2f::new(4.0, 6.0),
            Point2f::new(9.0, 6.4),
            VirtualLineDirection::Inbound,
        );
        let c = Point2f::new(3.0, 8.0);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_above);

        let c = Point2f::new(5.0, 3.0);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_above);

        let c = Point2f::new(0.0, 5.5);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_above);

        let c = Point2f::new(0.0, 6.5);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_above);

        let c = Point2f::new(10.0, 5.5);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_above);

        let c = Point2f::new(35.1, 8.0);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_above);

        let c = Point2f::new(2.0, 6.5);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_above);

        let c = Point2f::new(-2.0, 3.0);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_above);

        let c = Point2f::new(75.0, 15.0);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_above);
    }
}
