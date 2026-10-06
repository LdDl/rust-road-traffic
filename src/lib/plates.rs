use std::fmt;
use tracing::info;
use uuid::Uuid;

use crate::lib::cv::{RawFrame, Rect};
use crate::lib::detection::{Detector, DetectorError};
use crate::lib::logging;
use crate::lib::vehicle_events::{BoundingBox, PlateDetection};
use crate::settings::{InferenceModelSettings, PlatesSettings};

mod ocr;
mod quality;
mod tracking;
use ocr::OcrRecognizer;
pub(crate) use tracking::TrackRecognition;

pub struct PlateModels {
    detection: Detector,
    ocr: Option<OcrRecognizer>,
    detection_settings: InferenceModelSettings,
}

// Temporary visual check of the detected plate crop.
pub fn save_plate_crop(
    frame: &RawFrame,
    bbox: &BoundingBox,
    track_id: Uuid,
    attempt: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let bbox = ocr_crop_bbox(frame, bbox);
    let mut rgb = Vec::with_capacity(bbox.width as usize * bbox.height as usize * 3);
    for row in bbox.y..bbox.y + bbox.height {
        let start = row as usize * frame.step() + bbox.x as usize * 3;
        let end = start + bbox.width as usize * 3;
        for bgr in frame.data[start..end].chunks_exact(3) {
            rgb.extend_from_slice(&[bgr[2], bgr[1], bgr[0]]);
        }
    }

    std::fs::create_dir_all("plate_crops")?;
    let file = std::fs::File::create(format!("plate_crops/{track_id}-{attempt}.png"))?;
    let mut encoder = png::Encoder::new(file, bbox.width, bbox.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&rgb)?;
    writer.finish()?;
    Ok(())
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
            Some(OcrRecognizer::new(
                Self::load_model(&settings.ocr.model, "plates.ocr")?,
                settings.ocr.model.clone(),
            ))
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
        let Some(ocr) = self.ocr.as_mut() else {
            return Ok(());
        };
        let crop_bbox = ocr_crop_bbox(frame, &plate.bbox);
        let crop = crop_frame(frame, &crop_bbox)
            .ok_or_else(|| "Plate bbox does not define a valid frame crop".to_string())?;
        plate.ocr = ocr.recognize(&crop, &crop_bbox)?;
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

fn ocr_crop_bbox(frame: &RawFrame, bbox: &BoundingBox) -> BoundingBox {
    // Expand each side by 10%, with at least two pixels for small plates.
    let pad_x = bbox.width.div_ceil(10).max(2);
    let pad_y = bbox.height.div_ceil(10).max(2);
    let x = bbox.x.saturating_sub(pad_x).min(frame.width);
    let y = bbox.y.saturating_sub(pad_y).min(frame.height);
    let right = bbox
        .x
        .saturating_add(bbox.width)
        .saturating_add(pad_x)
        .min(frame.width);
    let bottom = bbox
        .y
        .saturating_add(bbox.height)
        .saturating_add(pad_y)
        .min(frame.height);
    BoundingBox {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}

fn crop_frame(frame: &RawFrame, bbox: &BoundingBox) -> Option<RawFrame> {
    if bbox.width == 0
        || bbox.height == 0
        || bbox.x.checked_add(bbox.width)? > frame.width
        || bbox.y.checked_add(bbox.height)? > frame.height
    {
        return None;
    }
    let mut crop = RawFrame::new(bbox.width, bbox.height);
    let row_bytes = crop.step();
    for row in 0..bbox.height as usize {
        let source = (bbox.y as usize + row) * frame.step() + bbox.x as usize * 3;
        let target = row * row_bytes;
        crop.data[target..target + row_bytes]
            .copy_from_slice(&frame.data[source..source + row_bytes]);
    }
    Some(crop)
}

fn bbox_in_frame(bbox: Rect, crop: &BoundingBox) -> Option<BoundingBox> {
    if bbox.width <= 0 || bbox.height <= 0 {
        return None;
    }
    let left = (bbox.x as i64).clamp(0, crop.width as i64) as u32;
    let top = (bbox.y as i64).clamp(0, crop.height as i64) as u32;
    let right = (bbox.x as i64 + bbox.width as i64).clamp(0, crop.width as i64) as u32;
    let bottom = (bbox.y as i64 + bbox.height as i64).clamp(0, crop.height as i64) as u32;
    if right <= left || bottom <= top {
        return None;
    }
    Some(BoundingBox {
        x: crop.x + left,
        y: crop.y + top,
        width: right - left,
        height: bottom - top,
    })
}
