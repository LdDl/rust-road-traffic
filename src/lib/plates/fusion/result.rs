use serde::Serialize;

use crate::lib::vehicle_events::{BoundingBox, PlateDetection};

use super::{Alternative, FusionResult, Position, PositionStatus, mean};

#[derive(Serialize)]
pub(in crate::lib::plates) struct OcrSummary {
    number: String,
    mean_confidence: f64,
    has_conflicts: bool,
    reference_attempt: u8,
    positions: Vec<OcrPosition>,
}

#[derive(Serialize)]
struct OcrPosition {
    position: usize,
    row: usize,
    class: String,
    mean_confidence: f64,
    status: PositionStatus,
    bbox: Option<BoundingBox>,
    observations: Vec<Observation>,
    alternatives: Vec<OcrAlternative>,
}

#[derive(Serialize)]
struct OcrAlternative {
    class: String,
    mean_confidence: f64,
    observations: Vec<Observation>,
}

#[derive(Serialize)]
struct Observation {
    attempt: u8,
    confidence: f64,
}

impl FusionResult {
    pub(in crate::lib::plates) fn ocr_summary(
        &self,
        reference_attempt: u8,
        reference: &PlateDetection,
    ) -> Option<OcrSummary> {
        let number = self.hypothesis.as_ref()?;
        if self.rows.iter().all(Vec::is_empty)
            || !self
                .inputs
                .iter()
                .any(|input| input.attempt == reference_attempt)
        {
            return None;
        }
        let mut positions = Vec::new();
        for (row, groups) in self.rows.iter().enumerate() {
            for group in groups {
                let selected = &group.alternatives[0];
                // Use the real detection of this position in the reference frame, even if its class disagrees.
                let bbox = group
                    .observations
                    .iter()
                    .find(|symbol| symbol.attempt == reference_attempt)
                    .and_then(|symbol| reference.ocr.as_ref()?.symbols.get(symbol.index))
                    .map(|symbol| symbol.bbox);
                positions.push(OcrPosition {
                    // Positions are global reading-order indices; rows are also zero-based.
                    position: positions.len(),
                    row,
                    class: selected.class.clone(),
                    mean_confidence: selected.confidence_sum / selected.votes as f64,
                    status: group.status,
                    bbox,
                    observations: observations(group, selected),
                    alternatives: group
                        .alternatives
                        .iter()
                        .skip(1)
                        .map(|alternative| OcrAlternative {
                            class: alternative.class.clone(),
                            mean_confidence: alternative.confidence_sum / alternative.votes as f64,
                            observations: observations(group, alternative),
                        })
                        .collect(),
                });
            }
        }
        Some(OcrSummary {
            number: number.clone(),
            // Give each position equal weight, regardless of its number of observations.
            mean_confidence: mean(positions.iter().map(|position| position.mean_confidence)),
            has_conflicts: positions
                .iter()
                .any(|position| position.status == PositionStatus::Conflict),
            reference_attempt,
            positions,
        })
    }
}

fn observations(group: &Position, alternative: &Alternative) -> Vec<Observation> {
    group
        .observations
        .iter()
        .filter(|symbol| symbol.class == alternative.class)
        .map(|symbol| Observation {
            attempt: symbol.attempt,
            confidence: symbol.confidence,
        })
        .collect()
}
