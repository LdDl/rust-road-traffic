use opencv::{
    core::Point2f
};
// WGS84 semi-major axis. Same as in epsg.rs
const EARTH_RADIUS_KM: f32 = 6378.137;

// haversine_cv
// 
// See ref. https://en.wikipedia.org/wiki/Great-circle_distance#:~:text=The%20great%2Dcircle%20distance%2C%20orthodromic,line%20through%20the%20sphere's%20interior). [OpenCV version]
// src - source point containing longitude/latitude [x;y]
// dst - target point containing longitude/latitude [x;y]
// 
#[inline]
pub fn haversine_cv(src: Point2f, dst: Point2f) -> f32 {
    haversine(src.x, src.y, dst.x, dst.y)
}

// haversine
//
// See ref. https://en.wikipedia.org/wiki/Great-circle_distance#:~:text=The%20great%2Dcircle%20distance%2C%20orthodromic,line%20through%20the%20sphere's%20interior).
// src_lon/src_lat - source point containing longitude/latitude [x;y]
// dst_lon/dst_lat - target point containing longitude/latitude [x;y]
//
#[inline]
pub fn haversine(src_lon: f32, src_lat: f32, dst_lon: f32, dst_lat: f32) -> f32 {
    let lat1 = src_lat.to_radians();
    let lat2 = dst_lat.to_radians();
    let diff_lat = (dst_lat - src_lat).to_radians();
    let diff_lon = (dst_lon - src_lon).to_radians();

    let sin_dlat_half = (diff_lat * 0.5).sin();
    let sin_dlon_half = (diff_lon * 0.5).sin();

    let a = sin_dlat_half * sin_dlat_half
          + lat1.cos() * lat2.cos() * sin_dlon_half * sin_dlon_half;
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    c * EARTH_RADIUS_KM
}

// compute_center_cv
//
// Computes center between two points on a sphere [OpenCV version]
//
#[inline]
pub fn compute_center_cv(a: Point2f, b: Point2f) -> Point2f {
    let center = compute_center(a.x, a.y, b.x, b.y);
    Point2f::new(center.0, center.1)
}

// compute_center
//
// Computes center between two points on a sphere
//
#[inline]
pub fn compute_center(lon1: f32, lat1: f32, lon2: f32, lat2: f32) -> (f32, f32) {
    // Convert to radians and get sin/cos in single calls
    let (sin_lat1, cos_lat1) = lat1.to_radians().sin_cos();
    let (sin_lon1, cos_lon1) = lon1.to_radians().sin_cos();
    let (sin_lat2, cos_lat2) = lat2.to_radians().sin_cos();
    let (sin_lon2, cos_lon2) = lon2.to_radians().sin_cos();

    // Convert spherical coordinates to Cartesian coordinates
    let x1 = cos_lat1 * cos_lon1;
    let y1 = cos_lat1 * sin_lon1;
    let z1 = sin_lat1;

    let x2 = cos_lat2 * cos_lon2;
    let y2 = cos_lat2 * sin_lon2;
    let z2 = sin_lat2;

    // Compute the average Cartesian coordinates
    let center_x = (x1 + x2) / 2.0;
    let center_y = (y1 + y2) / 2.0;
    let center_z = (z1 + z2) / 2.0;

    // Convert the Cartesian coordinates back to spherical coordinates and then to degrees
    let center_lon = center_y.atan2(center_x).to_degrees();
    let center_lat = center_z.atan2((center_x * center_x + center_y * center_y).sqrt()).to_degrees();

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
        let correct_great_circle_distance = 0.10568606 as f32;
        let eps = 0.000001;
        assert!((dist - correct_great_circle_distance).abs() < eps);
    }
    #[test]
    fn test_haversine() {
        let src: (f32, f32) = (6.602018, 52.036769);
        let dst: (f32, f32) = (6.603560, 52.036730);
        let dist = haversine(src.0, src.1, dst.0, dst.1);
        let correct_great_circle_distance = 0.10568606 as f32;
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