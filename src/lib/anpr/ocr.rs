use crate::lib::anpr::types::{BoundingBox, OcrResult, OcrSymbol};
use crate::lib::cv::RawFrame;
use crate::lib::detection::Detector;
use crate::settings::InferenceModelSettings;

use crate::lib::anpr::crops::bbox_in_frame;

pub struct OcrRecognizer {
    detector: Detector,
    settings: InferenceModelSettings,
}

impl OcrRecognizer {
    pub fn new(detector: Detector, settings: InferenceModelSettings) -> Self {
        Self { detector, settings }
    }

    pub fn recognize(
        &mut self,
        crop: &RawFrame,
        crop_bbox: &BoundingBox,
    ) -> Result<Option<OcrResult>, String> {
        let (boxes, classes, confidences) = self.detector.detect_frame(
            crop,
            self.settings.conf_threshold,
            self.settings.nms_threshold,
        )?;
        let mut symbols = Vec::new();
        for ((bbox, class_id), confidence) in boxes.into_iter().zip(classes).zip(confidences) {
            if !confidence.is_finite() {
                continue;
            }
            let Some(bbox) = bbox_in_frame(bbox, crop_bbox) else {
                continue;
            };
            let class = self.settings.net_classes.get(class_id).ok_or_else(|| {
                format!("OCR class ID {class_id} is missing from plates.ocr.net_classes")
            })?;
            symbols.push(OcrSymbol {
                class: class.clone(),
                confidence,
                bbox,
            });
        }
        if symbols.is_empty() {
            return Ok(None);
        }
        let symbols = reading_order(symbols);
        let number = symbols.iter().map(|symbol| symbol.class.as_str()).collect();
        let confidence =
            symbols.iter().map(|symbol| symbol.confidence).sum::<f32>() / symbols.len() as f32;
        Ok(Some(OcrResult {
            number,
            confidence,
            symbols,
        }))
    }
}

fn center_y(symbol: &OcrSymbol) -> f64 {
    symbol.bbox.y as f64 + symbol.bbox.height as f64 / 2.0
}

fn reading_order(mut symbols: Vec<OcrSymbol>) -> Vec<OcrSymbol> {
    symbols.sort_by(|a, b| center_y(a).total_cmp(&center_y(b)));
    let mut rows: Vec<Vec<OcrSymbol>> = Vec::new();
    for symbol in symbols {
        if let Some(row) = rows.last_mut() {
            let count = row.len() as f64;
            let row_center = row.iter().map(center_y).sum::<f64>() / count;
            let row_height = row.iter().map(|s| s.bbox.height as f64).sum::<f64>() / count;
            // Allow vertical variation within a row, including smaller region digits.
            let tolerance = row_height.max(symbol.bbox.height as f64) * 0.5;
            if center_y(&symbol) - row_center <= tolerance {
                row.push(symbol);
                continue;
            }
        }
        rows.push(vec![symbol]);
    }
    for row in &mut rows {
        row.sort_by_key(|symbol| {
            (
                symbol.bbox.x as u64 * 2 + symbol.bbox.width as u64,
                symbol.bbox.y,
            )
        });
    }
    rows.into_iter().flatten().collect()
}
