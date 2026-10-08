use crate::lib::ocr_fusion::types::PositionStatus;
use mot_rs::utils::Rect;
use serde::Serialize;

#[derive(Clone, Copy, Serialize)]
pub struct BoundingBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl BoundingBox {
    pub fn intersection(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self
            .x
            .checked_add(self.width)?
            .min(other.x.checked_add(other.width)?);
        let bottom = self
            .y
            .checked_add(self.height)?
            .min(other.y.checked_add(other.height)?);
        if right <= x || bottom <= y {
            return None;
        }
        Some(Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }

    pub fn relative_to(self, parent: Self) -> Self {
        Self {
            x: self.x - parent.x,
            y: self.y - parent.y,
            width: self.width,
            height: self.height,
        }
    }

    pub fn from_detection(rect: Rect, frame_width: u32, frame_height: u32) -> Self {
        let x = rect.x.clamp(0.0, frame_width as f32).floor() as u32;
        let y = rect.y.clamp(0.0, frame_height as f32).floor() as u32;
        let right = (rect.x + rect.width).clamp(0.0, frame_width as f32).ceil() as u32;
        let bottom = (rect.y + rect.height)
            .clamp(0.0, frame_height as f32)
            .ceil() as u32;
        Self {
            x,
            y,
            width: right.saturating_sub(x),
            height: bottom.saturating_sub(y),
        }
    }
}

#[derive(Clone, Serialize)]
pub struct VehicleDetection {
    pub class: String,
    pub confidence: f32,
    pub bbox: BoundingBox,
}

#[derive(Serialize)]
pub struct OcrSymbol {
    pub class: String,
    pub confidence: f32,
    pub bbox: BoundingBox,
}

#[derive(Serialize)]
pub struct OcrResult {
    pub number: String,
    pub confidence: f32,
    pub symbols: Vec<OcrSymbol>,
}

#[derive(Serialize)]
pub struct PlateDetection {
    pub class: String,
    pub confidence: f32,
    pub bbox: BoundingBox,
    pub ocr: Option<OcrResult>,
}

#[derive(Serialize)]
pub struct PlateResult {
    pub class: String,
    pub confidence: f32,
    pub bbox: BoundingBox,
    pub ocr: Option<OcrSummary>,
}

#[derive(Serialize)]
pub struct OcrSummary {
    pub number: String,
    pub mean_confidence: f64,
    pub has_conflicts: bool,
    pub reference_attempt: u8,
    pub positions: Vec<OcrPosition>,
}

#[derive(Serialize)]
pub struct OcrPosition {
    pub position: usize,
    pub row: usize,
    pub class: String,
    pub mean_confidence: f64,
    pub status: PositionStatus,
    pub bbox: Option<BoundingBox>,
    pub observations: Vec<OcrObservation>,
    pub alternatives: Vec<OcrAlternative>,
}

#[derive(Serialize)]
pub struct OcrAlternative {
    pub class: String,
    pub mean_confidence: f64,
    pub observations: Vec<OcrObservation>,
}

#[derive(Serialize)]
pub struct OcrObservation {
    pub attempt: u8,
    pub confidence: f64,
}
