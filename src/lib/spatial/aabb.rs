//! Axis-aligned bounding box in floating-point coordinates.
//!
//! The one AABB type of the repository: zone grids, geometry primitives and
//! violation rules all use it instead of local tuples or integer rectangles.

use crate::lib::cv::Rect;
use crate::lib::spatial::Point2f;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Aabb {
    /// Bounding box of the two given points, given as the opposite corners.
    pub fn new(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Aabb {
            min_x: min_x.min(max_x),
            min_y: min_y.min(max_y),
            max_x: min_x.max(max_x),
            max_y: min_y.max(max_y),
        }
    }

    /// Bounding box of an integer `Rect` in the same pixel coordinates.
    pub fn from_rect(rect: &Rect) -> Self {
        let x = rect.x as f32;
        let y = rect.y as f32;
        Aabb::new(x, y, x + rect.width as f32, y + rect.height as f32)
    }

    /// Degenerate box consisting of a single point.
    pub fn point(point: Point2f) -> Self {
        Aabb {
            min_x: point.x,
            min_y: point.y,
            max_x: point.x,
            max_y: point.y,
        }
    }

    /// Bounding box of the given points, or `None` when there are none.
    pub fn from_points(points: &[Point2f]) -> Option<Self> {
        let mut points = points.iter();
        let first = *points.next()?;
        let mut bbox = Aabb::point(first);
        for point in points {
            bbox.min_x = bbox.min_x.min(point.x);
            bbox.min_y = bbox.min_y.min(point.y);
            bbox.max_x = bbox.max_x.max(point.x);
            bbox.max_y = bbox.max_y.max(point.y);
        }
        Some(bbox)
    }

    pub fn center(&self) -> Point2f {
        Point2f::new(
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
        )
    }

    /// Smallest box that contains both boxes.
    pub fn union(&self, other: &Aabb) -> Self {
        Aabb {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        !(self.max_x < other.min_x
            || other.max_x < self.min_x
            || self.max_y < other.min_y
            || other.max_y < self.min_y)
    }

    /// The box grown by `margin` on every side. A non-finite or negative
    /// margin is treated as zero.
    pub fn expanded(&self, margin: f32) -> Self {
        let margin = if margin.is_finite() {
            margin.max(0.0)
        } else {
            0.0
        };
        Aabb {
            min_x: self.min_x - margin,
            min_y: self.min_y - margin,
            max_x: self.max_x + margin,
            max_y: self.max_y + margin,
        }
    }

    pub fn is_finite(&self) -> bool {
        self.min_x.is_finite()
            && self.min_y.is_finite()
            && self.max_x.is_finite()
            && self.max_y.is_finite()
    }
}

impl From<&Rect> for Aabb {
    fn from(rect: &Rect) -> Self {
        Aabb::from_rect(rect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_are_normalized() {
        let aabb = Aabb::new(10.0, 30.0, 0.0, 5.0);
        assert_eq!(
            aabb,
            Aabb {
                min_x: 0.0,
                min_y: 5.0,
                max_x: 10.0,
                max_y: 30.0
            }
        );
    }

    #[test]
    fn from_points_handles_empty_and_single() {
        assert_eq!(Aabb::from_points(&[]), None);
        let single = Aabb::from_points(&[Point2f::new(3.0, 4.0)]).unwrap();
        assert_eq!(single, Aabb::point(Point2f::new(3.0, 4.0)));
    }

    #[test]
    fn union_and_intersection() {
        let a = Aabb::new(0.0, 0.0, 10.0, 10.0);
        let b = Aabb::new(5.0, 5.0, 15.0, 15.0);
        assert!(a.intersects(&b));
        assert_eq!(a.union(&b), Aabb::new(0.0, 0.0, 15.0, 15.0));
        let far = Aabb::new(100.0, 100.0, 110.0, 110.0);
        assert!(!a.intersects(&far));
    }

    #[test]
    fn expansion_clamps_bad_margins() {
        let a = Aabb::new(0.0, 0.0, 10.0, 10.0);
        assert_eq!(a.expanded(5.0), Aabb::new(-5.0, -5.0, 15.0, 15.0));
        assert_eq!(a.expanded(-1.0), a);
        assert_eq!(a.expanded(f32::NAN), a);
    }
}
