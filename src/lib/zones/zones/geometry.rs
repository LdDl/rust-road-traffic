use crate::lib::constants::EPSILON;
use crate::lib::spatial::{Aabb, Point2f};

#[derive(Copy, Clone, PartialEq)]
pub enum PointsOrientation {
    Collinear,
    Clockwise,
    CounterClockwise,
}

// get_orientation Gets orientations of points P -> Q -> R.
// Possible output values: Collinear / Clockwise or CounterClockwise
// Input: points P, Q and R in provided order
pub fn get_orientation(px: f32, py: f32, qx: f32, qy: f32, rx: f32, ry: f32) -> PointsOrientation {
    let val = (qy - py) * (rx - qx) - (qx - px) * (ry - qy);
    if val.abs() < EPSILON {
        return PointsOrientation::Collinear;
    }
    if val > 0.0 {
        return PointsOrientation::Clockwise;
    }
    PointsOrientation::CounterClockwise
}

// is_on_segment Checks if point Q lies on segment PR
// Input: three colinear points Q, Q and R
pub fn is_on_segment(px: f32, py: f32, qx: f32, qy: f32, rx: f32, ry: f32) -> bool {
    if qx <= f32::max(px, rx)
        && qx >= f32::min(px, rx)
        && qy <= f32::max(py, ry)
        && qy >= f32::min(py, ry)
    {
        return true;
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
pub fn is_intersects(
    first_px: f32,
    first_py: f32,
    first_qx: f32,
    first_qy: f32,
    second_px: f32,
    second_py: f32,
    second_qx: f32,
    second_qy: f32,
) -> bool {
    // Find the four orientations needed for general case and special ones
    let o1 = get_orientation(first_px, first_py, first_qx, first_qy, second_px, second_py);
    let o2 = get_orientation(first_px, first_py, first_qx, first_qy, second_qx, second_qy);
    let o3 = get_orientation(
        second_px, second_py, second_qx, second_qy, first_px, first_py,
    );
    let o4 = get_orientation(
        second_px, second_py, second_qx, second_qy, first_qx, first_qy,
    );

    // General case
    if o1 != o2 && o3 != o4 {
        return true;
    }

    /* Special cases */
    // P1, Q1, P2 are colinear and P2 lies on segment P1-Q1
    if o1 == PointsOrientation::Collinear
        && is_on_segment(first_px, first_py, second_px, second_py, first_qx, first_qy)
    {
        return true;
    }
    // P1, Q1 and Q2 are colinear and Q2 lies on segment P1-Q1
    if o2 == PointsOrientation::Collinear
        && is_on_segment(first_px, first_py, second_qx, second_qy, first_qx, first_qy)
    {
        return true;
    }
    // P2, Q2 and P1 are colinear and P1 lies on segment P2-Q2
    if o3 == PointsOrientation::Collinear
        && is_on_segment(
            second_px, second_py, first_px, first_py, second_qx, second_qy,
        )
    {
        return true;
    }
    // P2, Q2 and Q1 are colinear and Q1 lies on segment P2-Q2
    if o4 == PointsOrientation::Collinear
        && is_on_segment(
            second_px, second_py, first_qx, first_qy, second_qx, second_qy,
        )
    {
        return true;
    }
    // Segments do not intersect
    return false;
}

// ---------------------------------------------------------------------------
// Point-based helpers layered on the primitives above. These are the
// canonical implementations for every consumer in the repository: zone
// matching, virtual-line crossings and the violation rules.
// ---------------------------------------------------------------------------

/// Signed perpendicular distance from the infinite line through `a` and `b`
/// to `point`. Positive and negative values are opposite sides of the line;
/// a degenerate line yields zero.
pub fn line_signed_distance(a: Point2f, b: Point2f, point: Point2f) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length = (dx * dx + dy * dy).sqrt();
    if !length.is_finite() || length <= EPSILON {
        return 0.0;
    }
    let area2 = (b.x - a.x) * (point.y - a.y) - (b.y - a.y) * (point.x - a.x);
    area2 / length
}

/// Whether `point` lies inside the axis-aligned rectangle, inclusively
/// within `EPSILON`.
pub fn point_in_rect(point: Point2f, rect: &Aabb) -> bool {
    point.x >= rect.min_x - EPSILON
        && point.x <= rect.max_x + EPSILON
        && point.y >= rect.min_y - EPSILON
        && point.y <= rect.max_y + EPSILON
}

/// Whether the segment `start`-`end` intersects the axis-aligned rectangle.
pub fn segment_intersects_rect(start: Point2f, end: Point2f, rect: &Aabb) -> bool {
    if point_in_rect(start, rect) || point_in_rect(end, rect) {
        return true;
    }

    let corners = [
        Point2f::new(rect.min_x, rect.min_y),
        Point2f::new(rect.max_x, rect.min_y),
        Point2f::new(rect.max_x, rect.max_y),
        Point2f::new(rect.min_x, rect.max_y),
    ];

    for index in 0..4 {
        let next = (index + 1) % 4;
        if is_intersects(
            start.x,
            start.y,
            end.x,
            end.y,
            corners[index].x,
            corners[index].y,
            corners[next].x,
            corners[next].y,
        ) {
            return true;
        }
    }
    false
}

/// Point where two finite segments cross, when it is defined. Collinear
/// overlap returns `None`; the caller decides on a fallback.
pub fn segment_intersection_point(
    a: Point2f,
    b: Point2f,
    c: Point2f,
    d: Point2f,
) -> Option<Point2f> {
    let rx = b.x - a.x;
    let ry = b.y - a.y;
    let sx = d.x - c.x;
    let sy = d.y - c.y;
    let denominator = rx * sy - ry * sx;
    if denominator.abs() <= EPSILON {
        return None;
    }

    let cax = c.x - a.x;
    let cay = c.y - a.y;
    let t = (cax * sy - cay * sx) / denominator;
    let u = (cax * ry - cay * rx) / denominator;
    if !(-EPSILON..=1.0 + EPSILON).contains(&t) || !(-EPSILON..=1.0 + EPSILON).contains(&u) {
        return None;
    }

    Some(Point2f::new(a.x + t * rx, a.y + t * ry))
}

/// Ray-casting point-in-polygon test. A point exactly on an edge counts as
/// inside. Taken from `Zone::contains_point`, which used to be the only
/// copy: rules and zones must classify a point the same way.
pub fn polygon_contains_point(polygon: &[Point2f], x: f32, y: f32) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    // @todo: math.maxInt could lead to overflow obviously. Need good workaround. PRs are welcome
    let extreme_point: [f32; 2] = [99999.0, y];
    let mut intersections_cnt = 0;
    let mut previous = 0;
    loop {
        let current = (previous + 1) % n;
        // Check if the segment from given point P to extreme point intersects with the segment from polygon point on previous iteration to polygon point on current iteration
        if is_intersects(
            polygon[previous].x,
            polygon[previous].y,
            polygon[current].x,
            polygon[current].y,
            x,
            y,
            extreme_point[0],
            extreme_point[1],
        ) {
            let orientation = get_orientation(
                polygon[previous].x,
                polygon[previous].y,
                x,
                y,
                polygon[current].x,
                polygon[current].y,
            );
            // If given point P is collinear with segment from polygon point on previous iteration to polygon point on current iteration
            if orientation == PointsOrientation::Collinear {
                // then check if it is on segment
                // 'True' will be returns if it lies on segment. Otherwise 'False' will be returned
                return is_on_segment(
                    polygon[previous].x,
                    polygon[previous].y,
                    x,
                    y,
                    polygon[current].x,
                    polygon[current].y,
                );
            }
            intersections_cnt += 1;
        }
        previous = current;
        if previous == 0 {
            break;
        }
    }
    // If ray intersects even number of times then return true
    // Otherwise return false
    intersections_cnt % 2 == 1
}

#[cfg(test)]
mod point_helpers_tests {
    use super::*;

    #[test]
    fn signed_distance_is_perpendicular_and_signed() {
        let a = Point2f::new(0.0, 0.0);
        let b = Point2f::new(100.0, 0.0);
        assert_eq!(line_signed_distance(a, b, Point2f::new(50.0, 30.0)), 30.0);
        assert_eq!(line_signed_distance(a, b, Point2f::new(50.0, -30.0)), -30.0);
        // Degenerate line has no sides.
        assert_eq!(line_signed_distance(a, a, Point2f::new(50.0, 30.0)), 0.0);
    }

    #[test]
    fn segment_intersects_rectangle() {
        let rect = Aabb::new(10.0, 10.0, 30.0, 30.0);
        let pt = |x: f32, y: f32| Point2f::new(x, y);
        assert!(segment_intersects_rect(
            pt(0.0, 20.0),
            pt(40.0, 20.0),
            &rect
        ));
        // Diagonally through opposite corners.
        assert!(segment_intersects_rect(pt(0.0, 0.0), pt(40.0, 40.0), &rect));
        // One endpoint inside.
        assert!(segment_intersects_rect(
            pt(20.0, 20.0),
            pt(50.0, 50.0),
            &rect
        ));
        // Fully outside.
        assert!(!segment_intersects_rect(pt(0.0, 5.0), pt(40.0, 5.0), &rect));
    }

    #[test]
    fn segment_intersection_point_is_defined_for_crossings() {
        let a = Point2f::new(0.0, 50.0);
        let b = Point2f::new(100.0, 50.0);
        let c = Point2f::new(50.0, 0.0);
        let d = Point2f::new(50.0, 100.0);
        let point = segment_intersection_point(a, b, c, d).expect("crossing");
        assert!((point.x - 50.0).abs() < EPSILON && (point.y - 50.0).abs() < EPSILON);
        // Parallel segments have no crossing point.
        assert_eq!(
            segment_intersection_point(a, b, Point2f::new(0.0, 60.0), Point2f::new(100.0, 60.0)),
            None
        );
    }

    #[test]
    fn polygon_contains_point_counts_an_edge_point_as_inside() {
        let polygon = [
            Point2f::new(0.0, 0.0),
            Point2f::new(100.0, 0.0),
            Point2f::new(100.0, 100.0),
            Point2f::new(0.0, 100.0),
        ];
        assert!(polygon_contains_point(&polygon, 50.0, 50.0));
        assert!(!polygon_contains_point(&polygon, 150.0, 50.0));
        // Exactly on an edge: the canonical semantics say "inside".
        assert!(polygon_contains_point(&polygon, 50.0, 0.0));
        assert!(polygon_contains_point(&polygon, 100.0, 25.0));
        // Degenerate polygons contain nothing.
        assert!(!polygon_contains_point(&[], 0.0, 0.0));
        assert!(!polygon_contains_point(&[Point2f::new(0.0, 0.0)], 0.0, 0.0));
    }
}
