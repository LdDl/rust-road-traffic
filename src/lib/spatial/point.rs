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
