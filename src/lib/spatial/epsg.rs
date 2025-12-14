use opencv::core::Point2f;

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

// WGS84 semi-major axis (equatorial radius)
const EARTH_RADIUS_M: f32 = 6378137.0;

#[inline]
pub fn lonlat_to_meters_cv(lon_lat: &Point2f) -> Point2f {
    Point2f::new(lon2x(lon_lat.x), lat2y(lon_lat.y))
}

#[inline]
pub fn lon2x(lon: f32) -> f32 {
    EARTH_RADIUS_M * lon.to_radians()
}

#[inline]
pub fn x2lon(x: f32) -> f32 {
    (x / EARTH_RADIUS_M).to_degrees()
}

#[inline]
pub fn lat2y(lat: f32) -> f32 {
    // Web Mercator: y = R * ln(tan(π/4 + lat/2))
    (lat.to_radians() * 0.5 + FRAC_PI_4).tan().ln() * EARTH_RADIUS_M
}

#[inline]
pub fn y2lat(y: f32) -> f32 {
    // Inverse: lat = 2 * atan(exp(y/R)) - π/2
    (2.0 * (y / EARTH_RADIUS_M).exp().atan() - FRAC_PI_2).to_degrees()
}

#[inline]
pub fn lonlat_to_meters(lon: f32, lat: f32) -> (f32, f32) {
    (lon2x(lon), lat2y(lat))
}

#[inline]
pub fn meters_to_lonlat(x: f32, y: f32) -> (f32, f32) {
    (x2lon(x), y2lat(y))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_epsg_conversion() {
        let test_lon: f32 = 37.6202637616082;
        let test_lat: f32 = 54.208100345367;

        let eps_xy: f32 = 3.0;
        let eps_lonlat: f32 = 0.0001;
        let correct_x: f32 = 4187868.6054508663;
        let correct_y: f32 = 7209666.936059543;
        let (x, y) = lonlat_to_meters(test_lon, test_lat);
        println!("x: {}, y: {}", x, y);
        println!("correct_x: {}, correct_y: {}", correct_x, correct_y);
        assert!((x - correct_x).abs() < eps_xy);
        assert!((y - correct_y).abs() < eps_xy);

        let (lon, lat) = meters_to_lonlat(x, y);
        assert!((lon - test_lon).abs() < eps_lonlat);
        assert!((lat - test_lat).abs() < eps_lonlat);
    }
}

