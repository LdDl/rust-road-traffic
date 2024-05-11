use opencv::core::Point2f;

use std::f32::consts::E;
use std::f32::consts::PI;
const EARTH_RADIUS_M: f32 = 6378137.0;
const EARTH_RADIUS_KM: f32 = 6378.137;
const f: f32 = 298.257223563;

pub fn lonlat_to_meters_cv(lon_lat: &Point2f) -> Point2f {
    let lon = lon_lat.x;
    let lat = lon_lat.y;
    let x = lon2x(lon);
    let y = lat2y(lat);
    Point2f::new(x, y)
}

pub fn lon2x(lon: f32) -> f32 {
    EARTH_RADIUS_KM * 1000. * lon.to_radians()
}

pub fn x2lon(x: f32) -> f32 {
    (x / (EARTH_RADIUS_KM * 1000.)).to_degrees()
}

pub fn lat2y(lat: f32) -> f32 {
    ((lat.to_radians() / 2. + PI / 4.).tan()).log(E) * EARTH_RADIUS_KM * 1000.
}

pub fn y2lat(y: f32) -> f32 {
    (2. * ((y / (EARTH_RADIUS_KM * 1000.)).exp()).atan() - PI / 2.).to_degrees()
}

pub fn lonlat_to_meters(lon: f32, lat: f32) -> (f32, f32) {
    let x = lon2x(lon);
    let y = lat2y(lat);
    (x, y)
}

pub fn meters_to_lonlat(x: f32, y: f32) -> (f32, f32) {
    let lon = x2lon(x);
    let lat = y2lat(y);
    (lon, lat)
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

