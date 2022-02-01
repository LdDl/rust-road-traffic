#[derive(Copy, Clone, PartialEq)]
pub enum PointsOrientation {
    Collinear,
    Clockwise,
    CounterClockwise
}

// get_orientation Gets orientations of points P -> Q -> R.
// Possible output values: Collinear / Clockwise or CounterClockwise
// Input: points P, Q and R in provided order
pub fn get_orientation(px: f32, py: f32, qx: f32, qy: f32, rx: f32, ry: f32) -> PointsOrientation {
    let val = (qy-py)*(rx-qx) - (qx-px)*(ry-qy);
	if val == 0.0 {
		return PointsOrientation::Collinear;
	}
	if val > 0.0 {
		return PointsOrientation::Clockwise;
	}
    return PointsOrientation::CounterClockwise; // if it's neither collinear nor clockwise
}

// is_on_segment Checks if point Q lies on segment PR
// Input: three colinear points Q, Q and R
pub fn is_on_segment(px: f32, py: f32, qx: f32, qy: f32, rx: f32, ry: f32) -> bool {
    if qx <= f32::max(px, rx) && qx >= f32::min(px, rx) && qy <= f32::max(py, ry) && qy >= f32::min(py, ry) {
		return true
	}
    return false;
}

// is_intersects Checks if segments intersect each other
// Input:
// first_px, first_py, first_qx, first_qy === first segment
// second_px, second_py, second_qx, second_qy === second segment
/*
Notation
	P1 = (first_px, first_py)
	Q1 = (first_qx, first_qy)
	P2 = (second_px, second_py)
	Q2 = (second_qx, second_qy)
*/
pub fn is_intersects(first_px: f32, first_py: f32, first_qx: f32, first_qy: f32, second_px: f32, second_py: f32, second_qx: f32, second_qy: f32) -> bool {
    // Find the four orientations needed for general case and special ones
    let o1 = get_orientation(first_px, first_py, first_qx, first_qy, second_px, second_py);
    let o2 = get_orientation(first_px, first_py, first_qx, first_qy, second_qx, second_qy);
    let o3 = get_orientation(second_px, second_py, second_qx, second_qy, first_px, first_py);
    let o4 = get_orientation(second_px, second_py, second_qx, second_qy, first_qx, first_qy);

    // General case
    if o1 != o2 && o3 != o4 {
        return true;
    }

    /* Special cases */
    // P1, Q1, P2 are colinear and P2 lies on segment P1-Q1
    if o1 == PointsOrientation::Collinear && is_on_segment(first_px, first_py, second_px, second_py, first_qx, first_qy) {
        return true;
    }
    // P1, Q1 and Q2 are colinear and Q2 lies on segment P1-Q1
    if o2 == PointsOrientation::Collinear && is_on_segment(first_px, first_py, second_qx, second_qy, first_qx, first_qy) {
        return true;
    }
    // P2, Q2 and P1 are colinear and P1 lies on segment P2-Q2
    if o3 == PointsOrientation::Collinear && is_on_segment(second_px, second_py, first_px, first_py, second_qx, second_qy) {
        return true;
    }
    // P2, Q2 and Q1 are colinear and Q1 lies on segment P2-Q2
    if o4 == PointsOrientation::Collinear && is_on_segment(second_px, second_py, first_qx, first_qy, second_qx, second_qy) {
        return true;
    }
    // Segments do not intersect
    return false;
}