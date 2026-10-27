use crate::lib::track_observations::PointPx;

pub fn contains_point(polygon: &[PointPx], point: &PointPx) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = n - 1;

    for i in 0..n {
        let pi = &polygon[i];
        let pj = &polygon[j];

        let dy = pj.y - pi.y;
        let denom = if dy.abs() < f32::EPSILON {
            f32::EPSILON
        } else {
            dy
        };

        let intersects = ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / denom + pi.x);

        if intersects {
            inside = !inside;
        }

        j = i;
    }

    inside
}