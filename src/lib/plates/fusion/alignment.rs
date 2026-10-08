use serde::Serialize;

use super::{Group, NormalizedBox, Symbol, median, representative};

// Costs are experimental alignment scores, not probabilities.
const GAP_COST: f64 = 1.0;
const AMBIGUOUS_MARGIN: f64 = 0.25;

#[derive(Serialize)]
pub(super) struct Metrics {
    cost: f64,
    alternative_margin: Option<f64>,
    pub ambiguous: bool,
}

#[derive(Clone, Copy)]
enum Step {
    Match,
    Missing,
    Insert,
}

#[derive(Clone)]
struct Cell {
    costs: [f64; 2],
    step: Step,
}

fn match_cost(b: NormalizedBox, group: &Group, symbol: &Symbol, max_shift: f64) -> f64 {
    let s = symbol.bbox;
    let shift = (b.cx() - s.cx()).abs();
    let dx = shift / b.width.max(s.width);
    let dy = (b.cy() - s.cy()).abs() / b.height.max(s.height);
    if shift > max_shift || dx > 2.5 || dy > 1.5 {
        return f64::INFINITY;
    }
    let intersection = ((b.x + b.width).min(s.x + s.width) - b.x.max(s.x)).max(0.0)
        * ((b.y + b.height).min(s.y + s.height) - b.y.max(s.y)).max(0.0);
    let iou = intersection / (b.width * b.height + s.width * s.height - intersection);
    let class_cost = if group.iter().any(|v| v.class == symbol.class) {
        0.0
    } else {
        0.15
    };
    0.7 * dx + 0.2 * dy + 0.2 * (1.0 - iou) + class_cost
}

pub(super) fn align(
    groups: &[Group],
    symbols: &[Symbol],
    registered: bool,
) -> (Vec<Group>, Metrics) {
    let (n, m) = (groups.len(), symbols.len());
    let stride = m + 1;
    let mut cells = vec![
        Cell {
            costs: [f64::INFINITY; 2],
            step: Step::Match
        };
        (n + 1) * stride
    ];
    let boxes: Vec<_> = groups.iter().map(representative).collect();
    // After registration, half the observed character spacing separates positions.
    let spacings: Vec<_> = boxes
        .windows(2)
        .map(|pair| pair[1].cx() - pair[0].cx())
        .chain(
            symbols
                .windows(2)
                .map(|pair| pair[1].bbox.cx() - pair[0].bbox.cx()),
        )
        .filter(|spacing| *spacing > 0.0)
        .collect();
    let max_shift = if registered && !spacings.is_empty() {
        median(spacings.into_iter()) / 2.0
    } else {
        f64::INFINITY
    };
    cells[0].costs[0] = 0.0;
    for i in 0..=n {
        for j in 0..=m {
            if i == 0 && j == 0 {
                continue;
            }
            let mut choices = Vec::with_capacity(6);
            if i > 0 && j > 0 {
                let cost = match_cost(boxes[i - 1], &groups[i - 1], &symbols[j - 1], max_shift);
                choices.extend(
                    cells[(i - 1) * stride + j - 1]
                        .costs
                        .map(|v| (v + cost, Step::Match)),
                );
            }
            // Order unmatched symbols spatially, including gaps at the end of a row.
            if i > 0 && (j == 0 || boxes[i - 1].cx() >= symbols[j - 1].bbox.cx()) {
                choices.extend(
                    cells[(i - 1) * stride + j]
                        .costs
                        .map(|v| (v + GAP_COST, Step::Missing)),
                );
            }
            if j > 0 && (i == 0 || symbols[j - 1].bbox.cx() > boxes[i - 1].cx()) {
                choices.extend(
                    cells[i * stride + j - 1]
                        .costs
                        .map(|v| (v + GAP_COST, Step::Insert)),
                );
            }
            choices.sort_by(|a, b| a.0.total_cmp(&b.0));
            cells[i * stride + j] = Cell {
                costs: [choices[0].0, choices[1].0],
                step: choices[0].1,
            };
        }
    }
    let (mut i, mut j) = (n, m);
    let mut result = Vec::new();
    while i > 0 || j > 0 {
        match cells[i * stride + j].step {
            Step::Match => {
                let mut group = groups[i - 1].clone();
                group.push(symbols[j - 1].clone());
                result.push(group);
                i -= 1;
                j -= 1;
            }
            Step::Missing => {
                result.push(groups[i - 1].clone());
                i -= 1;
            }
            Step::Insert => {
                result.push(vec![symbols[j - 1].clone()]);
                j -= 1;
            }
        }
    }
    result.reverse();
    let [best, second] = cells[n * stride + m].costs;
    let margin = second - best;
    (
        result,
        Metrics {
            cost: best,
            alternative_margin: margin.is_finite().then_some(margin),
            ambiguous: margin < AMBIGUOUS_MARGIN,
        },
    )
}
