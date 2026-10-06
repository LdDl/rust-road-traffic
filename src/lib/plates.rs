use std::fmt;
use tracing::info;

use crate::lib::cv::RawFrame;
use crate::lib::detection::{Detector, DetectorError};
use crate::lib::logging;
use crate::lib::vehicle_events::{BoundingBox, PlateDetection};
use crate::settings::{InferenceModelSettings, PlatesSettings};

pub struct PlateModels {
    pub detection: Detector,
    pub ocr: Option<Detector>,
    detection_settings: InferenceModelSettings,
}

#[derive(Debug)]
pub struct PlateModelLoadError {
    model: &'static str,
    source: DetectorError,
}

impl fmt::Display for PlateModelLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.model, self.source)
    }
}

impl std::error::Error for PlateModelLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl PlateModels {
    pub fn from_settings(
        settings: Option<&PlatesSettings>,
    ) -> Result<Option<Self>, PlateModelLoadError> {
        let Some(settings) = settings.filter(|settings| settings.enable) else {
            return Ok(None);
        };

        let detection = Self::load_model(&settings.detection, "plates.detection")?;
        let ocr = if settings.ocr.enable {
            Some(Self::load_model(&settings.ocr.model, "plates.ocr")?)
        } else {
            None
        };

        Ok(Some(Self {
            detection,
            ocr,
            detection_settings: settings.detection.clone(),
        }))
    }

    pub fn detect_plate(
        &mut self,
        frame: &RawFrame,
        vehicle_bbox: &BoundingBox,
    ) -> Result<Option<PlateDetection>, String> {
        let x = vehicle_bbox.x.min(frame.width);
        let y = vehicle_bbox.y.min(frame.height);
        let width = vehicle_bbox.width.min(frame.width - x);
        let height = vehicle_bbox.height.min(frame.height - y);
        if width == 0 || height == 0 {
            return Ok(None);
        }

        let mut crop = RawFrame::new(width, height);
        let row_bytes = crop.step();
        for row in 0..height as usize {
            let source = (y as usize + row) * frame.step() + x as usize * 3;
            let target = row * row_bytes;
            crop.data[target..target + row_bytes]
                .copy_from_slice(&frame.data[source..source + row_bytes]);
        }

        let settings = &self.detection_settings;
        let (boxes, classes, confidences) =
            self.detection
                .detect_frame(&crop, settings.conf_threshold, settings.nms_threshold)?;
        let mut best: Option<PlateDetection> = None;
        for ((bbox, class_id), confidence) in boxes.into_iter().zip(classes).zip(confidences) {
            if !confidence.is_finite() || bbox.width <= 0 || bbox.height <= 0 {
                continue;
            }
            let class = settings.net_classes.get(class_id).ok_or_else(|| {
                format!("Plate class ID {class_id} is missing from plates.detection.net_classes")
            })?;
            let left = (bbox.x as i64).clamp(0, width as i64) as u32;
            let top = (bbox.y as i64).clamp(0, height as i64) as u32;
            let right = (bbox.x as i64 + bbox.width as i64).clamp(0, width as i64) as u32;
            let bottom = (bbox.y as i64 + bbox.height as i64).clamp(0, height as i64) as u32;
            if right <= left || bottom <= top {
                continue;
            }
            if best
                .as_ref()
                .is_some_and(|plate| plate.confidence >= confidence)
            {
                continue;
            }
            best = Some(PlateDetection {
                class: class.clone(),
                confidence,
                bbox: BoundingBox {
                    x: x + left,
                    y: y + top,
                    width: right - left,
                    height: bottom - top,
                },
                ocr: None,
            });
        }
        Ok(best)
    }

    fn load_model(
        settings: &InferenceModelSettings,
        model: &'static str,
    ) -> Result<Detector, PlateModelLoadError> {
        info!(
            scope = logging::SCOPE_STARTUP,
            model,
            weights = %settings.network_weights,
            "Loading plate model"
        );
        let detector = Detector::new(
            &settings.network_weights,
            settings.net_width.zip(settings.net_height),
            None,
        )
        .map_err(|source| PlateModelLoadError { model, source })?;
        info!(scope = logging::SCOPE_STARTUP, model, "Plate model loaded");
        Ok(detector)
    }
}
