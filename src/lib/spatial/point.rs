// Directly copied from OpenCV

#[derive(Clone, Default, Debug)]
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
