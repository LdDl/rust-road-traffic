use opencv::{
    prelude::*,
    core::Mat,
    core::Point2f,
    core::Vector,
    core::DECOMP_LU,
    core::mul_mat_mat,
    core::CV_32F,
    imgproc::get_perspective_transform,
};

// Spatial converter around transform matrix.
// It helps to transform coordinates from Euclidean space to WGS84 projection
pub struct SpatialConverter {
    transform_mat: Mat
}

impl SpatialConverter {
    // Constructor for SpatialConverter
    //
    // src_points - OpenCV vector of source points in Euclidean space
    // dest_points - OpenCV vector of destination points (for further transformation) in WGS84 projection
    //
    fn new(src_points: &Vector<Point2f>, dest_points: &Vector<Point2f>) -> Self {
        let transform_mat_f64 = get_perspective_transform(&src_points, &dest_points, DECOMP_LU).unwrap();
        let mut transform_mat_f32 = Mat::default();
        match transform_mat_f64.convert_to(&mut transform_mat_f32, CV_32F, 1.0, 0.0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't cast tranform matrix into float32 due the error: {:?}", err);
            }
        };
        return SpatialConverter{
            transform_mat: transform_mat_f32
        };
    }
    // Constructor for SpatialConverter
    //
    // src_points - built-in vector of source points in Euclidean space
    // dest_points - built-in vector of destination points (for further transformation) in WGS84 projection
    //
    fn new_from(src_points: Vec<Point2f>, dest_points: Vec<Point2f>) -> Self {
        let src = Vector::<Point2f>::from(src_points);
        let trgt = Vector::<Point2f>::from(dest_points);
        let transform_mat_f64 = get_perspective_transform(&src, &trgt, DECOMP_LU).unwrap();
        let mut transform_mat_f32 = Mat::default();
        match transform_mat_f64.convert_to(&mut transform_mat_f32, CV_32F, 1.0, 0.0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't cast tranform matrix into float32 due the error: {:?}", err);
            }
        };
        return SpatialConverter{
            transform_mat: transform_mat_f32
        };
    }
    // Spatial conversion function
    //
    // src - point in Euclidean space
    //
    fn transform_to_wgs84(&self, src: &Point2f) -> Point2f {
        let pmat_data = vec![
            vec![src.x as f32],
            vec![src.y as f32],
            vec![1.0 as f32],
        ];
        let pmat = Mat::from_slice_2d(&pmat_data).unwrap();
        let answ = mul_mat_mat(&self.transform_mat, &pmat).unwrap().to_mat().unwrap();
        let answ_ptr = answ.data_typed::<f32>().unwrap();
        let scale = answ_ptr[2];
        let xattr = answ_ptr[0];
        let yattr = answ_ptr[1];
        return Point2f::new(xattr / scale, yattr / scale);
    }
}

use std::f32::consts::PI;
const earth_raidus_km: f32 = 6371.0;

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
	let km = c * earth_raidus_km;
    return km;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_spatial_converter() {
        let mut src = Vector::<Point2f>::new();
        src.push(Point2f::new(1200.0, 278.0));
        src.push(Point2f::new(87.0, 328.0),);
        src.push(Point2f::new(36.0, 583.0));
        src.push(Point2f::new(1205.0, 698.0));

        let mut dst = Vector::<Point2f>::new();
        dst.push(Point2f::new(6.602018, 52.036769));
        dst.push(Point2f::new(6.603227, 52.036181));
        dst.push(Point2f::new(6.603638, 52.036558));
        dst.push(Point2f::new(6.603560, 52.036730));

        let converter = SpatialConverter::new(&src, &dst);
        for (i, p) in src.iter().enumerate() {
            let result = converter.transform_to_wgs84(&p);
            /* Round to 3 decimal places as we may lose precision due f32->f64 and f64->f32 casting */
            let result_x = (result.x * 10e3).round() / 10e3;
            let result_y = (result.y * 10e3).round() / 10e3;
            let correct_x = (dst.get(i).unwrap().x * 10e3).round() / 10e3;
            let correct_y = (dst.get(i).unwrap().y * 10e3).round() / 10e3;
            assert_eq!(result_x, correct_x);
            assert_eq!(result_y, correct_y);
        }

        let src_from = vec![
            Point2f::new(1200.0, 278.0),
            Point2f::new(87.0, 328.0),
            Point2f::new(36.0, 583.0),
            Point2f::new(1205.0, 698.0)
        ];
        let dst_from = vec![
            Point2f::new(6.602018, 52.036769),
            Point2f::new(6.603227, 52.036181),
            Point2f::new(6.603638, 52.036558),
            Point2f::new(6.603560, 52.036730)
        ];
        
        let converter_from = SpatialConverter::new_from(src_from.clone(), dst_from);
        for (i, p) in src_from.iter().enumerate() {
            let result = converter_from.transform_to_wgs84(&p);
            /* Round to 3 decimal places as we may lose precision due f32->f64 and f64->f32 casting */
            let result_x = (result.x * 10e3).round() / 10e3;
            let result_y = (result.y * 10e3).round() / 10e3;
            let correct_x = (dst.get(i).unwrap().x * 10e3).round() / 10e3;
            let correct_y = (dst.get(i).unwrap().y * 10e3).round() / 10e3;
            assert_eq!(result_x, correct_x);
            assert_eq!(result_y, correct_y);
        }
    }
    #[test]
    fn test_haversine() {
        let src = Point2f::new(6.602018, 52.036769);
        let dst = Point2f::new(6.603560, 52.036730);
        let dist = haversine(src, dst);
        let correct_great_circle_distance = 0.10556793 as f32;
        assert_eq!(dist, correct_great_circle_distance);
    }
}