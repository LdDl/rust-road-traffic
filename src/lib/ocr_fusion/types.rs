use serde::Serialize;

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionStatus {
    Agreement,
    Single,
    Conflict,
}

// Bounding boxes use the caller's normalized text-region coordinates.
// Preserve out-of-region symbols when a recognition crop includes padding.
pub struct DetectedSymbol {
    pub class: String,
    pub confidence: f64,
    pub bbox: NormalizedBox,
}

pub struct Observation {
    pub attempt: u8,
    pub number: String,
    pub symbols: Vec<DetectedSymbol>,
}

#[derive(Clone, Copy, Serialize)]
pub struct NormalizedBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl NormalizedBox {
    pub fn cx(self) -> f64 {
        self.x + self.width / 2.0
    }

    pub fn cy(self) -> f64 {
        self.y + self.height / 2.0
    }
}

#[derive(Clone, Serialize)]
pub struct Symbol {
    pub attempt: u8,
    pub index: usize,
    pub class: String,
    pub confidence: f64,
    pub bbox: NormalizedBox,
    pub original_bbox: Option<NormalizedBox>,
}

pub type Group = Vec<Symbol>;

#[derive(Serialize)]
pub struct Input {
    pub attempt: u8,
    pub number: String,
    pub rows: usize,
}

#[derive(Serialize)]
pub struct Alternative {
    pub class: String,
    pub votes: usize,
    pub confidence_sum: f64,
    pub attempts: Vec<u8>,
}

#[derive(Serialize)]
pub struct Position {
    pub proposed_class: String,
    pub status: PositionStatus,
    pub missing_attempts: Vec<u8>,
    pub alternatives: Vec<Alternative>,
    pub observations: Group,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Flag {
    NoOcr,
    InvalidInput,
    InputLimit,
    RowLayoutMismatch,
    SingleReading,
    AmbiguousAlignment,
    ConflictSymbols,
    SingleSymbols,
}

#[derive(Serialize)]
pub struct Alignment {
    pub attempt: u8,
    pub row: usize,
    pub registration: Registration,
    #[serde(flatten)]
    pub metrics: Metrics,
}

#[derive(Serialize)]
pub struct FusionResult {
    pub hypothesis: Option<String>,
    pub experimental: bool,
    pub coordinate_space: &'static str,
    pub inputs: Vec<Input>,
    pub anchor_attempt: Option<u8>,
    pub alignment_order: Vec<u8>,
    pub alignment: Vec<Alignment>,
    pub rows: Vec<Vec<Position>>,
    pub flags: Vec<Flag>,
}

#[derive(Serialize)]
pub struct Transform {
    pub scale_x: f64,
    pub shift_x: f64,
    pub scale_y: f64,
    pub shear_y: f64,
    pub shift_y: f64,
}

#[derive(Serialize)]
pub struct Registration {
    pub applied: bool,
    pub available_anchors: usize,
    pub reason: &'static str,
    pub anchors: Vec<String>,
    pub transform: Option<Transform>,
    pub max_anchor_error: Option<f64>,
}

#[derive(Serialize)]
pub struct Metrics {
    pub cost: f64,
    pub alternative_margin: Option<f64>,
    pub ambiguous: bool,
}
