use std::collections::BTreeMap;

use crate::lib::ocr_fusion::geometry::mean;
use crate::lib::ocr_fusion::types::{
    Alignment, Alternative, Flag, FusionResult, Group, Input, Observation, Position,
    PositionStatus, Symbol,
};

pub mod alignment;
pub mod geometry;
pub mod registration;
pub mod types;

// Bound the experimental analysis independently of detector output size.
const MAX_SYMBOLS: usize = 128;
const MAX_READINGS: usize = 3;

struct PreparedReading {
    attempt: u8,
    rows: Vec<Vec<Symbol>>,
    confidence: f64,
    count: usize,
}

pub fn analyze(observations: impl IntoIterator<Item = Observation>) -> FusionResult {
    let mut result = FusionResult {
        hypothesis: None,
        experimental: true,
        coordinate_space: "anchor_normalized",
        inputs: Vec::new(),
        anchor_attempt: None,
        alignment_order: Vec::new(),
        alignment: Vec::new(),
        rows: Vec::new(),
        flags: Vec::new(),
    };
    let mut readings = Vec::new();
    for observation in observations {
        let attempt = observation.attempt;
        if observation.symbols.is_empty() {
            continue;
        }
        if observation.symbols.len() > MAX_SYMBOLS || readings.len() >= MAX_READINGS {
            result.flags.push(Flag::InputLimit);
            return result;
        }
        if readings
            .iter()
            .any(|r: &PreparedReading| r.attempt == attempt)
        {
            result.flags.push(Flag::InvalidInput);
            return result;
        }
        let mut symbols = Vec::new();
        for (index, symbol) in observation.symbols.into_iter().enumerate() {
            let bbox = symbol.bbox;
            if !symbol.confidence.is_finite()
                || !(0.0..=1.0).contains(&symbol.confidence)
                || ![
                    bbox.x,
                    bbox.y,
                    bbox.width,
                    bbox.height,
                    bbox.cx(),
                    bbox.cy(),
                ]
                .iter()
                .all(|value| value.is_finite())
                || bbox.width <= 0.0
                || bbox.height <= 0.0
            {
                result.flags.push(Flag::InvalidInput);
                return result;
            }
            symbols.push(Symbol {
                attempt,
                index,
                class: symbol.class,
                confidence: symbol.confidence,
                bbox,
                original_bbox: None,
            });
        }
        let confidence = mean(symbols.iter().map(|s| s.confidence));
        let count = symbols.len();
        let rows = reading_rows(symbols);
        result.inputs.push(Input {
            attempt,
            number: observation.number,
            rows: rows.len(),
        });
        readings.push(PreparedReading {
            attempt,
            rows,
            confidence,
            count,
        });
    }
    if readings.is_empty() {
        result.flags.push(Flag::NoOcr);
        return result;
    }
    if readings
        .iter()
        .any(|r| r.rows.len() != readings[0].rows.len())
    {
        result.flags.push(Flag::RowLayoutMismatch);
        return result;
    }
    // Match the offline prototype: the longest reading is an anchor, not a trusted answer.
    readings.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.attempt.cmp(&b.attempt))
    });
    result.anchor_attempt = Some(readings[0].attempt);
    result.alignment_order = readings.iter().map(|r| r.attempt).collect();
    let mut grouped: Vec<Vec<Group>> = readings[0]
        .rows
        .iter()
        .map(|row| row.iter().cloned().map(|s| vec![s]).collect())
        .collect();
    for reading in readings.iter().skip(1) {
        for (index, symbols) in reading.rows.iter().enumerate() {
            let (symbols, registration) = registration::register(&grouped[index], symbols.clone());
            let (groups, metrics) =
                alignment::align(&grouped[index], &symbols, registration.applied);
            grouped[index] = groups;
            result.alignment.push(Alignment {
                attempt: reading.attempt,
                row: index + 1,
                registration,
                metrics,
            });
        }
    }
    let mut attempts = result.alignment_order.clone();
    attempts.sort_unstable();
    result.rows = grouped
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|group| summarize(group, &attempts))
                .collect()
        })
        .collect();
    result.hypothesis = Some(
        result
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|p| p.proposed_class.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );
    if readings.len() == 1 {
        result.flags.push(Flag::SingleReading);
    }
    if result.alignment.iter().any(|a| a.metrics.ambiguous) {
        result.flags.push(Flag::AmbiguousAlignment);
    }
    for (status, flag) in [
        (PositionStatus::Conflict, Flag::ConflictSymbols),
        (PositionStatus::Single, Flag::SingleSymbols),
    ] {
        if result.rows.iter().flatten().any(|p| p.status == status) {
            result.flags.push(flag);
        }
    }
    result
}

fn reading_rows(mut symbols: Vec<Symbol>) -> Vec<Vec<Symbol>> {
    symbols.sort_by(|a, b| a.bbox.cy().total_cmp(&b.bbox.cy()));
    let mut rows: Vec<Vec<Symbol>> = Vec::new();
    for symbol in symbols {
        if let Some(row) = rows.last_mut() {
            let cy = mean(row.iter().map(|s| s.bbox.cy()));
            let height = mean(row.iter().map(|s| s.bbox.height));
            if symbol.bbox.cy() - cy <= 0.5 * height.max(symbol.bbox.height) {
                row.push(symbol);
                continue;
            }
        }
        rows.push(vec![symbol]);
    }
    for row in &mut rows {
        row.sort_by(|a, b| {
            a.bbox
                .cx()
                .total_cmp(&b.bbox.cx())
                .then_with(|| a.bbox.cy().total_cmp(&b.bbox.cy()))
        });
    }
    rows
}

fn summarize(mut group: Group, attempts: &[u8]) -> Position {
    group.sort_by_key(|s| s.attempt);
    let mut classes: BTreeMap<String, Vec<&Symbol>> = BTreeMap::new();
    for symbol in &group {
        classes
            .entry(symbol.class.clone())
            .or_default()
            .push(symbol);
    }
    let mut alternatives: Vec<_> = classes
        .into_iter()
        .map(|(class, symbols)| Alternative {
            class,
            votes: symbols.len(),
            confidence_sum: symbols.iter().map(|s| s.confidence).sum(),
            attempts: symbols.iter().map(|s| s.attempt).collect(),
        })
        .collect();
    alternatives.sort_by(|a, b| {
        b.votes
            .cmp(&a.votes)
            .then_with(|| b.confidence_sum.total_cmp(&a.confidence_sum))
            .then_with(|| a.class.cmp(&b.class))
    });
    Position {
        proposed_class: alternatives[0].class.clone(),
        status: if alternatives.len() > 1 {
            PositionStatus::Conflict
        } else if group.len() == 1 {
            PositionStatus::Single
        } else {
            PositionStatus::Agreement
        },
        missing_attempts: attempts
            .iter()
            .copied()
            .filter(|a| !group.iter().any(|s| s.attempt == *a))
            .collect(),
        alternatives,
        observations: group,
    }
}
