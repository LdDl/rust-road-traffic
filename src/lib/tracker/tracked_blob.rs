use uuid::Uuid;
use mot_rs::mot::{SimpleBlob, BlobBBox};
use mot_rs::utils::{Point, Rect};

/// Enum wrapper for tracked objects - allows runtime choice between centroid and bbox Kalman
#[derive(Debug, Clone)]
pub enum TrackedBlob {
    Simple(SimpleBlob),
    BBox(BlobBBox),
}

impl TrackedBlob {
    pub fn get_id(&self) -> Uuid {
        match self {
            TrackedBlob::Simple(b) => b.get_id(),
            TrackedBlob::BBox(b) => b.get_id(),
        }
    }
    pub fn get_track(&self) -> &Vec<Point> {
        match self {
            TrackedBlob::Simple(b) => b.get_track(),
            TrackedBlob::BBox(b) => b.get_track(),
        }
    }
    pub fn get_bbox(&self) -> Rect {
        match self {
            TrackedBlob::Simple(b) => b.get_bbox(),
            TrackedBlob::BBox(b) => b.get_bbox(),
        }
    }
    pub fn get_no_match_times(&self) -> usize {
        match self {
            TrackedBlob::Simple(b) => b.get_no_match_times(),
            TrackedBlob::BBox(b) => b.get_no_match_times(),
        }
    }
}
