use std::collections::BTreeMap;

use serde::Serialize;

use crate::lib::vehicle_events::PlateDetection;

mod alignment;
mod registration;
mod result;

pub(super) use result::OcrSummary;

// Bound the experimental analysis independently of detector output size.
const MAX_SYMBOLS: usize = 128;
const MAX_READINGS: usize = 3;

#[derive(Clone, Copy, Serialize)]
struct NormalizedBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl NormalizedBox {
    fn cx(self) -> f64 {
        self.x + self.width / 2.0
    }

    fn cy(self) -> f64 {
        self.y + self.height / 2.0
    }
}

#[derive(Clone, Serialize)]
struct Symbol {
    attempt: u8,
    index: usize,
    class: String,
    confidence: f64,
    bbox: NormalizedBox,
    original_bbox: Option<NormalizedBox>,
}

type Group = Vec<Symbol>;

struct Reading {
    attempt: u8,
    rows: Vec<Vec<Symbol>>,
    confidence: f64,
    count: usize,
}

#[derive(Serialize)]
struct Input {
    attempt: u8,
    number: String,
    rows: usize,
}

#[derive(Serialize)]
struct Alternative {
    class: String,
    votes: usize,
    confidence_sum: f64,
    attempts: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PositionStatus {
    Agreement,
    Single,
    Conflict,
}

#[derive(Serialize)]
struct Position {
    proposed_class: String,
    status: PositionStatus,
    missing_attempts: Vec<u8>,
    alternatives: Vec<Alternative>,
    observations: Group,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Flag {
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
struct Alignment {
    attempt: u8,
    row: usize,
    registration: registration::Registration,
    #[serde(flatten)]
    metrics: alignment::Metrics,
}

#[derive(Serialize)]
pub(super) struct FusionResult {
    pub hypothesis: Option<String>,
    experimental: bool,
    coordinate_space: &'static str,
    inputs: Vec<Input>,
    anchor_attempt: Option<u8>,
    alignment_order: Vec<u8>,
    alignment: Vec<Alignment>,
    rows: Vec<Vec<Position>>,
    flags: Vec<Flag>,
}

pub(super) fn analyze<'a>(
    observations: impl IntoIterator<Item = (u8, &'a PlateDetection)>,
) -> FusionResult {
    let mut result = FusionResult {
        hypothesis: None,
        experimental: true,
        coordinate_space: "anchor_plate_normalized",
        inputs: Vec::new(),
        anchor_attempt: None,
        alignment_order: Vec::new(),
        alignment: Vec::new(),
        rows: Vec::new(),
        flags: Vec::new(),
    };
    let mut readings = Vec::new();
    for (attempt, plate) in observations {
        let Some(ocr) = plate.ocr.as_ref().filter(|ocr| !ocr.symbols.is_empty()) else {
            continue;
        };
        if ocr.symbols.len() > MAX_SYMBOLS || readings.len() >= MAX_READINGS {
            result.flags.push(Flag::InputLimit);
            return result;
        }
        if plate.bbox.width == 0
            || plate.bbox.height == 0
            || readings.iter().any(|r: &Reading| r.attempt == attempt)
        {
            result.flags.push(Flag::InvalidInput);
            return result;
        }
        let mut symbols = Vec::new();
        for (index, symbol) in ocr.symbols.iter().enumerate() {
            if !symbol.confidence.is_finite()
                || !(0.0..=1.0).contains(&symbol.confidence)
                || symbol.bbox.width == 0
                || symbol.bbox.height == 0
            {
                result.flags.push(Flag::InvalidInput);
                return result;
            }
            // Padding can place symbols outside the detected plate; do not clamp coordinates.
            symbols.push(Symbol {
                attempt,
                index,
                class: symbol.class.clone(),
                confidence: symbol.confidence as f64,
                bbox: NormalizedBox {
                    x: (symbol.bbox.x as f64 - plate.bbox.x as f64) / plate.bbox.width as f64,
                    y: (symbol.bbox.y as f64 - plate.bbox.y as f64) / plate.bbox.height as f64,
                    width: symbol.bbox.width as f64 / plate.bbox.width as f64,
                    height: symbol.bbox.height as f64 / plate.bbox.height as f64,
                },
                original_bbox: None,
            });
        }
        let confidence = mean(symbols.iter().map(|s| s.confidence));
        let count = symbols.len();
        let rows = reading_rows(symbols);
        result.inputs.push(Input {
            attempt,
            number: ocr.number.clone(),
            rows: rows.len(),
        });
        readings.push(Reading {
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

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = values.fold((0.0, 0), |(sum, count), v| (sum + v, count + 1));
    sum / count as f64
}

fn median(values: impl Iterator<Item = f64>) -> f64 {
    let mut values: Vec<_> = values.collect();
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

fn representative(group: &Group) -> NormalizedBox {
    NormalizedBox {
        x: median(group.iter().map(|s| s.bbox.x)),
        y: median(group.iter().map(|s| s.bbox.y)),
        width: median(group.iter().map(|s| s.bbox.width)),
        height: median(group.iter().map(|s| s.bbox.height)),
    }
}
