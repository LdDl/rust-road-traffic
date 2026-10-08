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
    pub fn relative_to(self, parent: Self) -> RelativeBoundingBox {
        RelativeBoundingBox {
            // OCR padding can place a symbol before the detected plate origin.
            x: i64::from(self.x) - i64::from(parent.x),
            y: i64::from(self.y) - i64::from(parent.y),
            width: self.width,
            height: self.height,
            parent_width: parent.width,
            parent_height: parent.height,
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

#[derive(Serialize)]
pub struct RelativeBoundingBox {
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
    pub parent_width: u32,
    pub parent_height: u32,
}

#[derive(Clone, Serialize)]
pub struct VehicleDetection {
    pub class: String,
    pub confidence: f32,
    pub bbox: BoundingBox,
}

#[derive(Serialize)]
pub struct VehicleResult {
    pub class: String,
    pub confidence: f32,
    pub bbox: RelativeBoundingBox,
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
    pub bbox: RelativeBoundingBox,
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
    pub bbox: Option<RelativeBoundingBox>,
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
