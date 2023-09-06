use opencv::{
    core::Mat,
    core::Point2i,
    core::Point2f,
    core::Scalar,
    imgproc::line,
    imgproc::LINE_8,
};

#[derive(Debug)]
pub struct VirtualLine {
    pub line: [[i32; 2]; 2],
    pub line_cv: [Point2f; 2],
    pub color_cv: Scalar,
    pub color: [i16; 3],
    // 0 - left->right, top->bottom
    // 1 - right->left, bottom->top
    pub direction: u8,
}

impl VirtualLine {
    pub fn new_from_cv(a: Point2f, b: Point2f, _direction: u8) -> Self {
        VirtualLine {
            line: [[a.x as i32, a.y as i32], [b.x as i32, b.y as i32]],
            line_cv: [a, b],
            color_cv: Scalar::from((0.0, 0.0, 0.0)),
            color: [0, 0, 0],
            direction: _direction,
        }
    }
    pub fn new_from(ab: [[i32; 2]; 2], _direction: u8) -> Self {
        VirtualLine {
            line: ab,
            line_cv: [Point2f::new(ab[0][0] as f32, ab[0][1] as f32), Point2f::new(ab[1][0] as f32, ab[1][1] as f32)],
            color_cv: Scalar::from((0.0, 0.0, 0.0)),
            color: [0, 0, 0],
            direction: _direction,
        }
    }
    pub fn set_color(&mut self, r: i16, g: i16, b: i16) {
        self.color_cv = Scalar::from((r as f64, g as f64, b as f64));
        self.color = [r, g, b];
    }
    // is_left returns true if the given point is to the left side of the vertical AB or if the given point is above of the horizontal AB
    pub fn is_left(&self, cx: f32, cy: f32) -> bool {
        let a = self.line_cv[0];
        let b = self.line_cv[1];
        (b.x - a.x)*(cy - a.y) - (b.y - a.y)*(cx - a.x) > 0.0
    }
    pub fn clone(&self) -> Self {
        VirtualLine {
            line: self.line,
            line_cv: self.line_cv,
            color_cv: self.color_cv,
            color: self.color,
            direction: self.direction,
        }
    }
    pub fn draw_on_mat(&self, img: &mut Mat) {
        let a = Point2i::new(self.line[0][0], self.line[0][1]);
        let b = Point2i::new(self.line[1][0], self.line[1][1]);
        match line(img, a, b, self.color_cv, 2, LINE_8, 0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw virtual line for polygon due the error: {:?}", err)
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_vertical_line() {
        let vertical_line = VirtualLine::new_from_cv(Point2f::new(4.0, 3.0), Point2f::new(5.0, 10.0), 0);
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
        let vertical_line = VirtualLine::new_from_cv(Point2f::new(4.0, 6.0), Point2f::new(9.0, 6.4), 0);
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