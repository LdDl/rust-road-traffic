// Own Point type (API-compatible with opencv::core::Point_)

#[derive(Clone, Default, Debug, Copy, PartialEq)]
pub struct Point_<T> {
	pub x: T,
	pub y: T,
}

pub type Point = Point2i;
pub type Point2d = Point_<f64>;
pub type Point2f = Point_<f32>;
pub type Point2i = Point_<i32>;
pub type Point2l = Point_<i64>;

impl<T> Point_<T> {
    pub fn new(x: T, y: T) -> Self {
		Self { x, y }
	}
}

// Conversions between our Point_ and opencv::core::Point_
impl From<opencv::core::Point2f> for Point_<f32> {
    fn from(p: opencv::core::Point2f) -> Self {
        Self { x: p.x, y: p.y }
    }
}

impl From<Point_<f32>> for opencv::core::Point2f {
    fn from(p: Point_<f32>) -> Self {
        opencv::core::Point2f::new(p.x, p.y)
    }
}

impl From<opencv::core::Point2i> for Point_<i32> {
    fn from(p: opencv::core::Point2i) -> Self {
        Self { x: p.x, y: p.y }
    }
}

impl From<Point_<i32>> for opencv::core::Point2i {
    fn from(p: Point_<i32>) -> Self {
        opencv::core::Point2i::new(p.x, p.y)
    }
}

// Conversion between Point_ and tuple
impl<T> From<(T, T)> for Point_<T> {
    fn from(t: (T, T)) -> Self {
        Self { x: t.0, y: t.1 }
    }
}

impl<T> From<Point_<T>> for (T, T) {
    fn from(p: Point_<T>) -> Self {
        (p.x, p.y)
    }
}
