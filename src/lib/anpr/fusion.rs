use crate::lib::anpr::types::{
    OcrAlternative, OcrObservation, OcrPosition, OcrSummary, PlateDetection,
};
use crate::lib::ocr_fusion;
use crate::lib::ocr_fusion::geometry::mean;
use crate::lib::ocr_fusion::types::{
    Alternative, DetectedSymbol, FusionResult, NormalizedBox, Observation, Position, PositionStatus,
};

pub fn analyze<'a>(
    observations: impl IntoIterator<Item = (u8, &'a PlateDetection)>,
) -> FusionResult {
    let inputs = observations.into_iter().filter_map(|(attempt, plate)| {
        let ocr = plate.ocr.as_ref().filter(|ocr| !ocr.symbols.is_empty())?;
        let symbols = ocr
            .symbols
            .iter()
            .map(|symbol| DetectedSymbol {
                class: symbol.class.clone(),
                confidence: symbol.confidence as f64,
                // Padding can place symbols outside the detected plate; do not clamp coordinates.
                // Zero-size plate boxes yield non-finite coordinates, rejected by the fusion input validation.
                bbox: NormalizedBox {
                    x: (symbol.bbox.x as f64 - plate.bbox.x as f64) / plate.bbox.width as f64,
                    y: (symbol.bbox.y as f64 - plate.bbox.y as f64) / plate.bbox.height as f64,
                    width: symbol.bbox.width as f64 / plate.bbox.width as f64,
                    height: symbol.bbox.height as f64 / plate.bbox.height as f64,
                },
            })
            .collect();
        Some(Observation {
            attempt,
            number: ocr.number.clone(),
            symbols,
        })
    });
    let mut result = ocr_fusion::analyze(inputs);
    result.coordinate_space = "anchor_plate_normalized";
    result
}

pub fn summarize(
    fusion: &FusionResult,
    reference_attempt: u8,
    reference: &PlateDetection,
) -> Option<OcrSummary> {
    let number = fusion.hypothesis.as_ref()?;
    if fusion.rows.iter().all(Vec::is_empty)
        || !fusion
            .inputs
            .iter()
            .any(|input| input.attempt == reference_attempt)
    {
        return None;
    }
    let mut positions = Vec::new();
    for (row, groups) in fusion.rows.iter().enumerate() {
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

fn observations(group: &Position, alternative: &Alternative) -> Vec<OcrObservation> {
    group
        .observations
        .iter()
        .filter(|symbol| symbol.class == alternative.class)
        .map(|symbol| OcrObservation {
            attempt: symbol.attempt,
            confidence: symbol.confidence,
        })
        .collect()
}
