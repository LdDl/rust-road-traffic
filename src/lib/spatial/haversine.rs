use opencv::{
    core::Point2f
};
use std::f32::consts::PI;
const EARTH_RADIUS_KM: f32 = 6371.0;

// deg2rad Converts degrees to radians
//
// d - degrees value
//
fn deg2rad(d: f32) -> f32 {
    return d * PI / 180.0;
}

// haversine
// 
// See ref. https://en.wikipedia.org/wiki/Great-circle_distance#:~:text=The%20great%2Dcircle%20distance%2C%20orthodromic,line%20through%20the%20sphere's%20interior).
// src - source point containing longitude/latitude [x;y]
// dst - target point containing longitude/latitude [x;y]
// 
pub fn haversine(src: Point2f, dst: Point2f) -> f32 {
    let lat1 = deg2rad(src.y);
	let lon1 = deg2rad(src.x);
	let lat2 = deg2rad(dst.y);
	let lon2 = deg2rad(dst.x);
    let diff_lat = lat2 - lat1;
	let diff_lon = lon2 - lon1;
    let a = f32::powi(f32::sin(diff_lat / 2.0), 2) + f32::cos(lat1)*f32::cos(lat2)*f32::powi(f32::sin(diff_lon/2.0), 2);
    let c = 2.0 * f32::atan2(f32::sqrt(a), f32::sqrt(1.0 - a));
	let km = c * EARTH_RADIUS_KM;
    return km;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_haversine() {
        let src = Point2f::new(6.602018, 52.036769);
        let dst = Point2f::new(6.603560, 52.036730);
        let dist = haversine(src, dst);
        let correct_great_circle_distance = 0.10556793 as f32;
        assert_eq!(dist, correct_great_circle_distance);
    }
}