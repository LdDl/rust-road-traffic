use opencv::{
    core::Point2f
};
const EARTH_RADIUS_KM: f32 = 6371.0;

// haversine_cv
// 
// See ref. https://en.wikipedia.org/wiki/Great-circle_distance#:~:text=The%20great%2Dcircle%20distance%2C%20orthodromic,line%20through%20the%20sphere's%20interior). [OpenCV version]
// src - source point containing longitude/latitude [x;y]
// dst - target point containing longitude/latitude [x;y]
// 
pub fn haversine_cv(src: Point2f, dst: Point2f) -> f32 {
    haversine(src.x, src.y, dst.x, dst.y)
}

// haversine
// 
// See ref. https://en.wikipedia.org/wiki/Great-circle_distance#:~:text=The%20great%2Dcircle%20distance%2C%20orthodromic,line%20through%20the%20sphere's%20interior).
// src_lon/src_lat - source point containing longitude/latitude [x;y]
// dst_lon/dst_lat - target point containing longitude/latitude [x;y]
// 
pub fn haversine(src_lon: f32, src_lat: f32, dst_lon: f32, dst_lat: f32) -> f32 {
    let lat1 = src_lat.to_radians();
	let lat2 = dst_lat.to_radians();
    let diff_lat = (dst_lat - src_lat).to_radians();
	let diff_lon = (dst_lon - src_lon).to_radians();
    let a = f32::powi(f32::sin(diff_lat / 2.0), 2) + f32::cos(lat1)*f32::cos(lat2)*f32::powi(f32::sin(diff_lon/2.0), 2);
    let c = 2.0 * f32::atan2(f32::sqrt(a), f32::sqrt(1.0 - a));
	let km = c * EARTH_RADIUS_KM;
    return km;
}

// compute_center
//
// Computes center between two points on a sphere [OpenCV version]
//
pub fn compute_center_cv(a: Point2f, b: Point2f) -> Point2f {
    let center = compute_center(a.x, a.y, b.x, b.y);
    Point2f::new(center.0, center.1)
}

// compute_center
//
// Computes center between two points on a sphere
//
pub fn compute_center(lon1: f32, lat1: f32, lon2: f32, lat2: f32) -> (f32, f32) {
    // Convert longitude and latitude to radians
    let lon1_rad = lon1.to_radians();
    let lat1_rad = lat1.to_radians();
    let lon2_rad = lon2.to_radians();
    let lat2_rad = lat2.to_radians();

    // Convert spherical coordinates to Cartesian coordinates
    let x1 = lat1_rad.cos() * lon1_rad.cos();
    let y1 = lat1_rad.cos() * lon1_rad.sin();
    let z1 = lat1_rad.sin();

    let x2 = lat2_rad.cos() * lon2_rad.cos();
    let y2 = lat2_rad.cos() * lon2_rad.sin();
    let z2 = lat2_rad.sin();

    // Compute the average Cartesian coordinates
    let center_x = (x1 + x2) / 2.0;
    let center_y = (y1 + y2) / 2.0;
    let center_z = (z1 + z2) / 2.0;

    // Convert the Cartesian coordinates back to spherical coordinates
    let center_lon_rad = center_y.atan2(center_x);
    let center_lat_rad = center_z.atan2((center_x.powi(2) + center_y.powi(2)).sqrt());

    // Convert the center coordinates to degrees
    let center_lon = center_lon_rad.to_degrees();
    let center_lat = center_lat_rad.to_degrees();

    (center_lon, center_lat)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_haversine_cv() {
        let src = Point2f::new(6.602018, 52.036769);
        let dst = Point2f::new(6.603560, 52.036730);
        let dist = haversine_cv(src, dst);
        let correct_great_circle_distance = 0.10556793 as f32;
        let eps = 0.000001;
        assert!((dist - correct_great_circle_distance).abs() < eps);
    }
    #[test]
    fn test_haversine() {
        let src: (f32, f32) = (6.602018, 52.036769);
        let dst: (f32, f32) = (6.603560, 52.036730);
        let dist = haversine(src.0, src.1, dst.0, dst.1);
        let correct_great_circle_distance = 0.10556793 as f32;
        let eps = 0.000001;
        assert!((dist - correct_great_circle_distance).abs() < eps);
    }
    #[test]
    fn test_compute_center() {
        let src: (f32, f32) = (37.6190602357743, 54.205634366333044);
        let dst: (f32, f32) = (37.619014168449894, 54.205640353834866);
        let center = compute_center(src.0, src.1, dst.0, dst.1);
        let correct_center: (f32, f32) = (37.6190372021121, 54.205637360083955);
        let eps = 0.00001;
        assert!((center.0 - correct_center.0).abs() < eps);
        assert!((center.1 - correct_center.1).abs() < eps);
    }
}