use std::fmt;
use tracing::info;

use crate::lib::anpr::crops::{bbox_in_frame, crop_frame, ocr_crop_bbox};
use crate::lib::anpr::ocr::OcrRecognizer;
use crate::lib::anpr::types::{BoundingBox, PlateDetection};
use crate::lib::cv::RawFrame;
use crate::lib::detection::{Detector, DetectorError};
use crate::lib::logging;
use crate::settings::{InferenceModelSettings, PlatesSettings};

pub struct PlateModels {
    detection: Detector,
    ocr: OcrRecognizer,
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
        let ocr = OcrRecognizer::new(
            Self::load_model(&settings.ocr, "plates.ocr")?,
            settings.ocr.clone(),
        );

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
        let crop_bbox = BoundingBox {
            x,
            y,
            width,
            height,
        };
        let Some(crop) = crop_frame(frame, &crop_bbox) else {
            return Ok(None);
        };

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
            let Some(bbox) = bbox_in_frame(bbox, &crop_bbox) else {
                continue;
            };
            if best
                .as_ref()
                .is_some_and(|plate| plate.confidence >= confidence)
            {
                continue;
            }
            best = Some(PlateDetection {
                class: class.clone(),
                confidence,
                bbox,
                ocr: None,
            });
        }
        Ok(best)
    }

    pub fn recognize_plate(
        &mut self,
        frame: &RawFrame,
        plate: &mut PlateDetection,
    ) -> Result<(), String> {
        let crop_bbox = ocr_crop_bbox(frame, &plate.bbox);
        let crop = crop_frame(frame, &crop_bbox)
            .ok_or_else(|| "Plate bbox does not define a valid frame crop".to_string())?;
        plate.ocr = self.ocr.recognize(&crop, &crop_bbox)?;
        Ok(())
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
