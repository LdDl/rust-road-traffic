
use opencv::{
    core::Point,
};

pub fn euclidean_distance(p1: Point, p2: Point) -> f32 {
    let x_squared = i32::pow(i32::abs(p1.x - p2.x), 2);
    let y_squared = i32::pow(i32::abs(p1.y - p2.y), 2);
    let sum_f32 = (x_squared + y_squared) as f32;
    return f32::sqrt(sum_f32)
}

mod tests {
    use super::*;
    #[test]
    fn test_euclidean_distance() {
        let p1 = Point::new(341, 264);
        let p2 = Point::new(421, 427);
        let ans = euclidean_distance(p1, p2);
        assert_eq!(181.57367651, ans);
    }
}