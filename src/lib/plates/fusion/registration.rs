use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{Group, NormalizedBox, Symbol, mean, median, representative};

const MIN_ANCHORS: usize = 3;
const ANCHOR_TOLERANCE: f64 = 0.5;

#[derive(Serialize)]
struct Transform {
    scale_x: f64,
    shift_x: f64,
    scale_y: f64,
    shear_y: f64,
    shift_y: f64,
}

#[derive(Serialize)]
pub(super) struct Registration {
    pub applied: bool,
    available_anchors: usize,
    reason: &'static str,
    anchors: Vec<String>,
    transform: Option<Transform>,
    max_anchor_error: Option<f64>,
}

struct Anchor {
    class: String,
    source: NormalizedBox,
    target: NormalizedBox,
}

struct Candidate {
    scale: f64,
    shift: f64,
    error: f64,
    inliers: Vec<usize>,
}

fn fit_line(points: &[(f64, f64)]) -> (f64, f64) {
    let cx = mean(points.iter().map(|p| p.0));
    let cy = mean(points.iter().map(|p| p.1));
    let spread: f64 = points.iter().map(|p| (p.0 - cx).powi(2)).sum();
    if spread <= 1e-12 {
        return (0.0, cy);
    }
    let slope = points.iter().map(|p| (p.0 - cx) * (p.1 - cy)).sum::<f64>() / spread;
    (slope, cy - slope * cx)
}

pub(super) fn register(groups: &[Group], mut symbols: Vec<Symbol>) -> (Vec<Symbol>, Registration) {
    let mut target_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for group in groups {
        for class in group
            .iter()
            .map(|s| s.class.as_str())
            .collect::<BTreeSet<_>>()
        {
            *target_counts.entry(class).or_default() += 1;
        }
    }
    let mut source_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for symbol in &symbols {
        *source_counts.entry(&symbol.class).or_default() += 1;
    }
    // Only unique, non-conflicting character identities can anchor the transformation.
    let unique: BTreeMap<&str, NormalizedBox> = groups
        .iter()
        .filter(|group| {
            let class = group[0].class.as_str();
            target_counts[class] == 1 && group.iter().all(|s| s.class == class)
        })
        .map(|group| (group[0].class.as_str(), representative(group)))
        .collect();
    let anchors: Vec<_> = symbols
        .iter()
        .filter_map(|s| {
            if source_counts[s.class.as_str()] != 1 {
                return None;
            }
            unique.get(s.class.as_str()).map(|target| Anchor {
                class: s.class.clone(),
                source: s.bbox,
                target: *target,
            })
        })
        .collect();
    let unavailable = |reason| Registration {
        applied: false,
        available_anchors: anchors.len(),
        reason,
        anchors: Vec::new(),
        transform: None,
        max_anchor_error: None,
    };
    if anchors.len() < MIN_ANCHORS {
        return (symbols, unavailable("insufficient_unique_anchors"));
    }
    let min_span = 2.0 * median(anchors.iter().map(|a| a.source.width));
    let mut candidates = Vec::new();
    for i in 0..anchors.len() {
        for j in i + 1..anchors.len() {
            let (a, b) = (&anchors[i], &anchors[j]);
            let dx = b.source.cx() - a.source.cx();
            if dx.abs() < min_span {
                continue;
            }
            let scale = (b.target.cx() - a.target.cx()) / dx;
            if !(0.5..=2.0).contains(&scale) {
                continue;
            }
            let shift = a.target.cx() - scale * a.source.cx();
            let residuals: Vec<_> = anchors
                .iter()
                .map(|a| {
                    (scale * a.source.cx() + shift - a.target.cx()).abs()
                        / a.target.width.max(a.source.width * scale)
                })
                .collect();
            let inliers: Vec<_> = residuals
                .iter()
                .enumerate()
                .filter(|(_, r)| **r <= ANCHOR_TOLERANCE)
                .map(|(i, _)| i)
                .collect();
            if inliers.len() >= MIN_ANCHORS {
                let error = mean(inliers.iter().map(|i| residuals[*i]));
                candidates.push(Candidate {
                    scale,
                    shift,
                    error,
                    inliers,
                });
            }
        }
    }
    candidates.sort_by(|a, b| {
        b.inliers
            .len()
            .cmp(&a.inliers.len())
            .then_with(|| a.error.total_cmp(&b.error))
    });
    let Some(best) = candidates.first() else {
        return (symbols, unavailable("no_anchor_consensus"));
    };
    for other in candidates
        .iter()
        .skip(1)
        .take_while(|c| c.inliers.len() == best.inliers.len())
    {
        if anchors.iter().any(|a| {
            ((best.scale - other.scale) * a.source.cx() + best.shift - other.shift).abs()
                / a.target.width
                > ANCHOR_TOLERANCE
        }) {
            return (symbols, unavailable("competing_transforms"));
        }
    }
    let inliers: Vec<_> = best.inliers.iter().map(|i| &anchors[*i]).collect();
    let (scale_x, shift_x) = fit_line(
        &inliers
            .iter()
            .map(|a| (a.source.cx(), a.target.cx()))
            .collect::<Vec<_>>(),
    );
    if !(0.5..=2.0).contains(&scale_x) {
        return (symbols, unavailable("unstable_scale"));
    }
    let scale_y = median(inliers.iter().map(|a| a.target.height / a.source.height));
    // Correct moderate baseline tilt; this is not a projective rectification of the image.
    let (shear_y, shift_y) = fit_line(
        &inliers
            .iter()
            .map(|a| (a.source.cx(), a.target.cy() - scale_y * a.source.cy()))
            .collect::<Vec<_>>(),
    );
    let max_error = inliers
        .iter()
        .map(|a| {
            let dx = (scale_x * a.source.cx() + shift_x - a.target.cx()).abs()
                / a.target.width.max(scale_x * a.source.width);
            let dy = (scale_y * a.source.cy() + shear_y * a.source.cx() + shift_y - a.target.cy())
                .abs()
                / a.target.height;
            dx.max(dy)
        })
        .fold(0.0, f64::max);
    if max_error > ANCHOR_TOLERANCE {
        return (symbols, unavailable("poor_fit"));
    }
    for symbol in &mut symbols {
        let b = symbol.bbox;
        let cx = scale_x * b.cx() + shift_x;
        let cy = scale_y * b.cy() + shear_y * b.cx() + shift_y;
        let width = scale_x * b.width;
        let height = scale_y * b.height + shear_y.abs() * b.width;
        symbol.original_bbox = Some(b);
        symbol.bbox = NormalizedBox {
            x: cx - width / 2.0,
            y: cy - height / 2.0,
            width,
            height,
        };
    }
    (
        symbols,
        Registration {
            applied: true,
            available_anchors: anchors.len(),
            reason: "anchor_consensus",
            anchors: inliers.iter().map(|a| a.class.clone()).collect(),
            transform: Some(Transform {
                scale_x,
                shift_x,
                scale_y,
                shear_y,
                shift_y,
            }),
            max_anchor_error: Some(max_error),
        },
    )
}
