use opencv::{
    prelude::*,
    core::Mat,
    core::Point2f,
    core::Vector,
    core::DECOMP_LU,
    core::mul_mat_mat,
    core::CV_32F,
    imgproc::get_perspective_transform
};

use crate::lib::constants::EPSILON_TINY;

// Spatial converter around transform matrix.
// It helps to transform coordinates from Euclidean space to WGS84 projection
#[derive(Debug)]
pub struct SpatialConverter {
    transform_mat: Mat
}

impl SpatialConverter {
    // Just empty initialization
    pub fn default() -> Self {
        return SpatialConverter{
            transform_mat: Mat::default(),
        }
    }
    // Constructor for SpatialConverter
    //
    // src_points - OpenCV vector of source OpenCV points in Euclidean space
    // dest_points - OpenCV vector of destination OpenCV points (for further transformation) in WGS84 projection
    //
    pub fn new(src_points: &Vector<Point2f>, dest_points: &Vector<Point2f>) -> Self {
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
    // src_points - built-in vector of source OpenCV points in Euclidean space
    // dest_points - built-in vector of destination OpenCV points (for further transformation) in WGS84 projection
    //
    pub fn new_from(src_points: Vec<Point2f>, dest_points: Vec<Point2f>) -> Self {
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
    pub fn transform_to_epsg_cv(&self, src: &Point2f) -> Point2f {
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
        // Guard against degenerate perspective transform (scale ≈ 0)
        if scale.abs() < EPSILON_TINY {
            return Point2f::new(f32::NAN, f32::NAN);
        }
        Point2f::new(xattr / scale, yattr / scale)
    }
    pub fn transform_to_epsg(&self, src_x: f32, src_y: f32) -> (f32, f32) {
        let pmat_data = vec![
            vec![src_x],
            vec![src_y],
            vec![1.0 as f32],
        ];
        let pmat = Mat::from_slice_2d(&pmat_data).unwrap();
        let answ = mul_mat_mat(&self.transform_mat, &pmat).unwrap().to_mat().unwrap();
        let answ_ptr = answ.data_typed::<f32>().unwrap();
        let scale = answ_ptr[2];
        let xattr = answ_ptr[0];
        let yattr = answ_ptr[1];
        // Guard against degenerate perspective transform (scale ≈ 0)
        if scale.abs() < EPSILON_TINY {
            return (f32::NAN, f32::NAN);
        }
        (xattr / scale, yattr / scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::spatial::haversine::haversine;
    use crate::lib::spatial::epsg::lonlat_to_meters;
    use crate::lib::spatial::epsg::meters_to_lonlat;
    #[test]

    fn test_spatial_converter() {
        let mut src = Vector::<Point2f>::new();
        src.push(Point2f::new(554.0, 592.0));
        src.push(Point2f::new(959.0, 664.0),);
        src.push(Point2f::new(1098.0, 360.0));
        src.push(Point2f::new(998.0, 359.0));

        let mut dst = Vector::<Point2f>::new();
        dst.push(Point2f::new(37.353610, 55.853085));
        dst.push(Point2f::new(37.353559, 55.853081));
        dst.push(Point2f::new(37.353564, 55.852918));
        dst.push(Point2f::new(37.353618, 55.852930));
        dst = dst.into_iter().map(|pt| {
            let pt = lonlat_to_meters(pt.x, pt.y);
            Point2f::new(pt.0, pt.1)
        }).collect();

        let eps_transform = 10.0;

        let converter = SpatialConverter::new(&src, &dst);
        for (i, p) in src.iter().enumerate() {
            let result = converter.transform_to_epsg_cv(&p);
            let result_x = result.x;
            let result_y = result.y;
            let correct_x = dst.get(i).unwrap().x;
            let correct_y = dst.get(i).unwrap().y;

            let diff_x = (result_x - correct_x).abs();
            let diff_y = (result_y - correct_y).abs();

            assert!(diff_x < eps_transform);
            assert!(diff_y < eps_transform);
        }

        let eps = 0.0001;

        let a = Point2f::new(959.0, 664.0);
        let b = Point2f::new(1098.0, 360.0);

        let a_epsg3857 = converter.transform_to_epsg_cv(&a);
        let b_epsg3857 = converter.transform_to_epsg_cv(&b);

        let a_wgs84 = meters_to_lonlat(a_epsg3857.x, a_epsg3857.y);
        let b_wgs84 = meters_to_lonlat(b_epsg3857.x, b_epsg3857.y);

        println!("a_epsg3857: {:?}", a_epsg3857);
        println!("b_epsg3857: {:?}", b_epsg3857);

        println!("a_wgs84: {:?}", a_wgs84);
        println!("b_wgs84: {:?}", b_wgs84);

        let distance = haversine(a_wgs84.0, a_wgs84.1, b_wgs84.0, b_wgs84.1) * 1000.0;
        println!("distance: {}", distance);
        let correct_dist: f32 = 19.96;
        assert!((distance - correct_dist).abs() < eps);
    }
    #[test]
    fn test_skeleton() {
        let mut src: Vector<opencv::core::Point_<f32>> = Vector::<Point2f>::new();
        src.push(Point2f::new(51.0, 266.0));
        src.push(Point2f::new(281.0, 264.0),);
        src.push(Point2f::new(334.0, 80.0));
        src.push(Point2f::new(179.0, 68.0));

        let mut dst = Vector::<Point2f>::new();
        dst.push(Point2f::new(37.6190602357743, 54.205634366333044));
        dst.push(Point2f::new(37.619014168449894, 54.205640353834866));
        dst.push(Point2f::new(37.61899251287025, 54.205596598993196));
        dst.push(Point2f::new(37.6190330678655, 54.205588538885735));
        dst = dst.into_iter().map(|pt| {
            let pt = lonlat_to_meters(pt.x, pt.y);
            Point2f::new(pt.0, pt.1)
        }).collect();

        let eps_transform = 10.0;

        let converter = SpatialConverter::new(&src, &dst);
        for (i, p) in src.iter().enumerate() {
            let result = converter.transform_to_epsg_cv(&p);
            let result_x = result.x;
            let result_y = result.y;
            let correct_x = dst.get(i).unwrap().x;
            let correct_y = dst.get(i).unwrap().y;

            let diff_x = (result_x - correct_x).abs();
            let diff_y = (result_y - correct_y).abs();

            assert!(diff_x < eps_transform);
            assert!(diff_y < eps_transform);
        }

        let eps = 0.0001;

        let a = Point2f::new(51.0, 266.0);
        let b = Point2f::new(281.0, 264.0);

        let a_epsg3857 = converter.transform_to_epsg_cv(&a);
        let b_epsg3857 = converter.transform_to_epsg_cv(&b);

        let a_wgs84 = meters_to_lonlat(a_epsg3857.x, a_epsg3857.y);
        let b_wgs84 = meters_to_lonlat(b_epsg3857.x, b_epsg3857.y);

        println!("a_epsg3857: {:?}", a_epsg3857);
        println!("b_epsg3857: {:?}", b_epsg3857);

        println!("a_wgs84: {:?}", a_wgs84);
        println!("b_wgs84: {:?}", b_wgs84);
    }
}