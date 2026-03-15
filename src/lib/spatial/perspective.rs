use nalgebra::Matrix3;

use crate::lib::constants::EPSILON_TINY;

/// Perspective transformer using a 3x3 homography matrix.
/// Converts coordinates from pixel space to projected (e.g. EPSG:3857) space.
///
/// Pure nalgebra replacement for OpenCV's `cv::getPerspectiveTransform` + `cv::Mat * vec`.
/// OpenCV C++ source: https://github.com/opencv/opencv/blob/4.x/modules/imgproc/src/imgwarp.cpp#L2921
#[derive(Debug, Clone)]
pub struct PerspectiveTransform {
    matrix: Matrix3<f32>,
}

impl PerspectiveTransform {
    /// Creates a perspective transform from 4 source-destination point pairs.
    ///
    /// Uses the DLT (Direct Linear Transform) algorithm to compute
    /// a 3x3 homography matrix H such that dst = H * src (in homogeneous coordinates).
    ///
    /// # Arguments
    /// * `src` - 4 source points in pixel space [(x, y); 4]
    /// * `dst` - 4 destination points in projected space [(x, y); 4]
    ///
    /// # Returns
    /// `Some(PerspectiveTransform)` if the system is solvable, `None` if degenerate.
    pub fn new(src: &[(f32, f32); 4], dst: &[(f32, f32); 4]) -> Option<Self> {
        let matrix = compute_perspective_matrix(src, dst)?;
        Some(Self { matrix })
    }

    /// Transforms a single point using the perspective matrix.
    ///
    /// Applies homogeneous transformation: result = H * [x, y, 1]^T
    /// then divides by the scale factor (third component).
    pub fn transform(&self, x: f32, y: f32) -> (f32, f32) {
        let w = self.matrix[(2, 0)] * x + self.matrix[(2, 1)] * y + self.matrix[(2, 2)];
        if w.abs() < EPSILON_TINY {
            return (f32::NAN, f32::NAN);
        }
        let out_x = self.matrix[(0, 0)] * x + self.matrix[(0, 1)] * y + self.matrix[(0, 2)];
        let out_y = self.matrix[(1, 0)] * x + self.matrix[(1, 1)] * y + self.matrix[(1, 2)];
        (out_x / w, out_y / w)
    }
}

/// Gaussian elimination with partial pivoting, matching OpenCV's LUImpl.
/// OpenCV C++ source: https://github.com/opencv/opencv/blob/4.x/modules/core/src/matrix_decomp.cpp#L15
///
/// Solves Ax = b in-place. Returns None if matrix is singular.
fn gaussian_elimination_pp(
    a: &mut nalgebra::SMatrix<f64, 8, 8>,
    b: &mut nalgebra::SVector<f64, 8>,
) -> Option<nalgebra::SVector<f64, 8>> {
    // same as OpenCV's DBL_EPSILON*100
    const EPS: f64 = f64::EPSILON * 100.0;

    // Forward elimination with partial pivoting
    for i in 0..8 {
        // Find pivot: row with largest |A[k][i]| for k >= i
        let mut pivot = i;
        for j in (i + 1)..8 {
            if a[(j, i)].abs() > a[(pivot, i)].abs() {
                pivot = j;
            }
        }

        if a[(pivot, i)].abs() < EPS {
            return None;
        }

        // Swap rows if needed
        if pivot != i {
            for j in i..8 {
                let tmp = a[(i, j)];
                a[(i, j)] = a[(pivot, j)];
                a[(pivot, j)] = tmp;
            }
            let tmp = b[i];
            b[i] = b[pivot];
            b[pivot] = tmp;
        }

        // Eliminate column below pivot
        let d = -1.0 / a[(i, i)];
        for j in (i + 1)..8 {
            let alpha = a[(j, i)] * d;
            for k in (i + 1)..8 {
                a[(j, k)] += alpha * a[(i, k)];
            }
            b[j] += alpha * b[i];
        }
    }

    // Back-substitution
    for i in (0..8).rev() {
        let mut s = b[i];
        for k in (i + 1)..8 {
            s -= a[(i, k)] * b[k];
        }
        b[i] = s / a[(i, i)];
    }

    Some(*b)
}

/// Computes a 3x3 perspective transformation matrix from 4 point correspondences.
///
/// Equivalent to OpenCV's `cv::getPerspectiveTransform`:
/// https://github.com/opencv/opencv/blob/4.x/modules/imgproc/src/imgwarp.cpp#L2921
///
/// Solves the system Ah = b where A is an 8x8 matrix derived from the DLT equations:
///
/// For each point pair (x, y) -> (x', y'):
///   x' = (h0*x + h1*y + h2) / (h6*x + h7*y + 1)
///   y' = (h3*x + h4*y + h5) / (h6*x + h7*y + 1)
///
/// Rearranged to:
///   h0*x + h1*y + h2 - h6*x*x' - h7*y*x' = x'
///   h3*x + h4*y + h5 - h6*x*y' - h7*y*y' = y'
pub fn compute_perspective_matrix(
    src: &[(f32, f32); 4],
    dst: &[(f32, f32); 4],
) -> Option<Matrix3<f32>> {
    // Work in f64 for numerical stability, convert result to f32
    let mut a = nalgebra::SMatrix::<f64, 8, 8>::zeros();
    let mut b = nalgebra::SVector::<f64, 8>::zeros();

    // OpenCV-compatible matrix setup matching getPerspectiveTransform in imgwarp.cpp:
    // - Row layout: rows 0-3 for X equations, rows 4-7 for Y equations
    // - Products (src*dst) computed in f32 first, then promoted to f64,
    //   matching C++ implicit float*float→float→double conversion
    for i in 0..4 {
        let (sx, sy) = (src[i].0, src[i].1);
        let (dx, dy) = (dst[i].0, dst[i].1);

        // Row i: x' equation
        a[(i, 0)] = sx as f64;
        a[(i, 1)] = sy as f64;
        a[(i, 2)] = 1.0;
        // a[(i, 3..5)] = 0  (already zero)
        a[(i, 6)] = -(sx * dx) as f64;
        a[(i, 7)] = -(sy * dx) as f64;
        b[i] = dx as f64;

        // Row i+4: y' equation
        // a[(i+4, 0..2)] = 0  (already zero)
        a[(i + 4, 3)] = sx as f64;
        a[(i + 4, 4)] = sy as f64;
        a[(i + 4, 5)] = 1.0;
        a[(i + 4, 6)] = -(sx * dy) as f64;
        a[(i + 4, 7)] = -(sy * dy) as f64;
        b[i + 4] = dy as f64;
    }

    // Solve Ah = b using Gaussian elimination with partial pivoting.
    // Matches OpenCV's LUImpl in modules/core/src/matrix_decomp.cpp
    let h = gaussian_elimination_pp(&mut a, &mut b)?;

    Some(Matrix3::new(
        h[0] as f32, h[1] as f32, h[2] as f32,
        h[3] as f32, h[4] as f32, h[5] as f32,
        h[6] as f32, h[7] as f32, 1.0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::spatial::epsg::lonlat_to_meters;
    use crate::lib::spatial::haversine::haversine;
    use crate::lib::spatial::epsg::meters_to_lonlat;

    const EPS: f32 = 1e-7;

    #[test]
    fn test_identity_transform() {
        // Square -> same square should give identity-like transform
        let src = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let dst = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let pt = PerspectiveTransform::new(&src, &dst).unwrap();
        let (x, y) = pt.transform(0.5, 0.5);
        assert!((x - 0.5).abs() < EPS);
        assert!((y - 0.5).abs() < EPS);
    }

    #[test]
    fn test_scale_transform() {
        // Scale 2x
        let src = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let dst = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)];
        let pt = PerspectiveTransform::new(&src, &dst).unwrap();
        let (x, y) = pt.transform(0.5, 0.5);
        assert!((x - 1.0).abs() < EPS);
        assert!((y - 1.0).abs() < EPS);
    }

    #[test]
    fn test_translation_transform() {
        let src = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let dst = [(10.0, 20.0), (11.0, 20.0), (11.0, 21.0), (10.0, 21.0)];
        let pt = PerspectiveTransform::new(&src, &dst).unwrap();
        let (x, y) = pt.transform(0.0, 0.0);
        assert!((x - 10.0).abs() < EPS);
        assert!((y - 20.0).abs() < EPS);
    }

    #[test]
    fn test_source_points_map_to_destination() {
        let src = [(554.0, 592.0), (959.0, 664.0), (1098.0, 360.0), (998.0, 359.0)];
        let dst_wgs84 = [
            (37.353610_f32, 55.853085),
            (37.353559, 55.853081),
            (37.353564, 55.852918),
            (37.353618, 55.852930),
        ];
        let dst_meters: [(f32, f32); 4] = core::array::from_fn(|i| {
            let m = lonlat_to_meters(dst_wgs84[i].0, dst_wgs84[i].1);
            (m.0, m.1)
        });

        let pt = PerspectiveTransform::new(&src, &dst_meters).unwrap();

        // meters tolerance
        let eps = 10.0;
        for i in 0..4 {
            let (rx, ry) = pt.transform(src[i].0, src[i].1);
            assert!((rx - dst_meters[i].0).abs() < eps, "x mismatch at point {}: {} vs {}", i, rx, dst_meters[i].0);
            assert!((ry - dst_meters[i].1).abs() < eps, "y mismatch at point {}: {} vs {}", i, ry, dst_meters[i].1);
        }
    }

    #[test]
    fn test_haversine_distance() {
        let src = [(554.0, 592.0), (959.0, 664.0), (1098.0, 360.0), (998.0, 359.0)];
        let dst_wgs84 = [
            (37.353610_f32, 55.853085),
            (37.353559, 55.853081),
            (37.353564, 55.852918),
            (37.353618, 55.852930),
        ];
        let dst_meters: [(f32, f32); 4] = core::array::from_fn(|i| {
            let m = lonlat_to_meters(dst_wgs84[i].0, dst_wgs84[i].1);
            (m.0, m.1)
        });

        let pt = PerspectiveTransform::new(&src, &dst_meters).unwrap();

        let a = pt.transform(959.0, 664.0);
        let b = pt.transform(1098.0, 360.0);

        let a_wgs84 = meters_to_lonlat(a.0, a.1);
        let b_wgs84 = meters_to_lonlat(b.0, b.1);

        let distance = haversine(a_wgs84.0, a_wgs84.1, b_wgs84.0, b_wgs84.1) * 1000.0;
        let correct_dist: f32 = 19.95998;
        assert!((distance - correct_dist).abs() < EPS, "distance mismatch: {} vs {}", distance, correct_dist);
    }

    #[test]
    fn test_skeleton_data() {
        // Same test data as in spatial.rs test_skeleton
        let src = [(51.0, 266.0), (281.0, 264.0), (334.0, 80.0), (179.0, 68.0)];
        let dst_wgs84 = [
            (37.6190602357743_f32, 54.205634366333044),
            (37.619014168449894, 54.205640353834866),
            (37.61899251287025, 54.205596598993196),
            (37.6190330678655, 54.205588538885735),
        ];
        let dst_meters: [(f32, f32); 4] = core::array::from_fn(|i| {
            let m = lonlat_to_meters(dst_wgs84[i].0, dst_wgs84[i].1);
            (m.0, m.1)
        });

        let pt = PerspectiveTransform::new(&src, &dst_meters).unwrap();

        let eps = 10.0;
        for i in 0..4 {
            let (rx, ry) = pt.transform(src[i].0, src[i].1);
            assert!((rx - dst_meters[i].0).abs() < eps, "x mismatch at point {}", i);
            assert!((ry - dst_meters[i].1).abs() < eps, "y mismatch at point {}", i);
        }
    }
}
