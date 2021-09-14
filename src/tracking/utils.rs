
use opencv::{
    core::Point,
};

pub fn euclidean_distance(p1: Point, p2: Point) -> f32 {
    let x_squared = i32::pow(i32::abs(p1.x - p2.x), 2);
    let y_squared = i32::pow(i32::abs(p1.x - p2.x), 2);
    let sum_f32 = (x_squared + y_squared) as f32;
    return f32::sqrt(sum_f32)
}