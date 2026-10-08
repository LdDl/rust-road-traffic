use serde::Serialize;
use std::cmp::Ordering;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::lib::cv::RawFrame;
use crate::lib::logging;
use crate::lib::vehicle_events::{BoundingBox, PlateDetection, PlateResult, VehicleDetection};

use super::fusion;
use super::quality::{CandidateQuality, PlateQuality};
use super::{PlateModels, crop_frame, ocr_crop_bbox, save_debug_crop, save_plate_crop};

const MAX_ATTEMPTS: u8 = 3;

struct Candidate {
    crop: RawFrame,
    crop_bbox: BoundingBox,
    vehicle: VehicleDetection,
    frame_size: (u32, u32),
    quality: CandidateQuality,
}

impl Candidate {
    fn new(
        frame: &RawFrame,
        vehicle: &VehicleDetection,
        quality: CandidateQuality,
    ) -> Option<Self> {
        if vehicle.bbox.width == 0 || vehicle.bbox.height == 0 {
            return None;
        }
        // Preserve surrounding pixels for the OCR padding, without retaining a full frame.
        let crop_bbox = ocr_crop_bbox(frame, &vehicle.bbox);
        Some(Self {
            crop: crop_frame(frame, &crop_bbox)?,
            crop_bbox,
            vehicle: vehicle.clone(),
            frame_size: (frame.width, frame.height),
            quality,
        })
    }
}

struct PlateObservation {
    vehicle: VehicleDetection,
    plate: PlateDetection,
    frame_size: (u32, u32),
    attempt: u8,
}

pub(crate) struct RecognitionResult {
    pub vehicle: VehicleDetection,
    pub plate: PlateResult,
    pub frame_size: (u32, u32),
}

impl PlateObservation {
    fn number(&self) -> Option<&str> {
        self.plate
            .ocr
            .as_ref()
            .map(|ocr| ocr.number.as_str())
            .filter(|number| !number.is_empty())
    }

    fn confidence(&self) -> f32 {
        self.plate.ocr.as_ref().map_or(0.0, |ocr| ocr.confidence)
    }
}

#[derive(Default)]
pub(crate) struct TrackRecognition {
    attempts: u8,
    last_attempt_bbox: Option<BoundingBox>,
    pending: Option<Candidate>,
    observations: Vec<PlateObservation>,
    confirmed: bool,
}

impl TrackRecognition {
    pub fn observe(
        &mut self,
        models: &mut PlateModels,
        frame: &RawFrame,
        vehicle: &VehicleDetection,
        track_id: Uuid,
    ) {
        if self.confirmed || self.attempts >= MAX_ATTEMPTS {
            return;
        }
        let Some(mut quality) = CandidateQuality::measure(frame, &vehicle.bbox) else {
            return;
        };
        let plate_reference = self
            .observations
            .last()
            .map(|observation| (&observation.vehicle.bbox, &observation.plate.bbox));
        quality.update_visibility(&vehicle.bbox, (frame.width, frame.height), plate_reference);
        if let Some(pending) = &mut self.pending {
            pending.quality.update_visibility(
                &pending.vehicle.bbox,
                pending.frame_size,
                plate_reference,
            );
        }

        // Reserve the last attempt for track completion.
        let should_attempt = self.attempts == 0
            || (self.attempts == 1
                && self
                    .last_attempt_bbox
                    .as_ref()
                    .is_some_and(|previous| moved_enough(previous, &vehicle.bbox)));
        if should_attempt {
            if let Some(mut candidate) = Candidate::new(frame, vehicle, quality) {
                if let Some(pending) = &mut self.pending {
                    if pending.quality.is_better_than(&candidate.quality) {
                        // Consume the better unused frame and keep the current one as a fallback.
                        std::mem::swap(pending, &mut candidate);
                    }
                }
                self.attempt(models, track_id, candidate);
            }
        } else if self
            .pending
            .as_ref()
            .is_none_or(|pending| quality.is_better_than(&pending.quality))
        {
            if let Some(candidate) = Candidate::new(frame, vehicle, quality) {
                self.pending = Some(candidate);
            }
        }
    }

    pub fn complete(
        mut self,
        models: Option<&mut PlateModels>,
        track_id: Uuid,
    ) -> Option<RecognitionResult> {
        if let (Some(models), Some(mut candidate)) = (models, self.pending.take()) {
            candidate.quality.update_visibility(
                &candidate.vehicle.bbox,
                candidate.frame_size,
                self.observations
                    .last()
                    .map(|observation| (&observation.vehicle.bbox, &observation.plate.bbox)),
            );
            self.attempt(models, track_id, candidate);
        }
        let fusion = fusion::analyze(self.observations.iter().map(|o| (o.attempt, &o.plate)));
        let Some(best) = (0..self.observations.len())
            .max_by(|&a, &b| self.compare(&self.observations[a], &self.observations[b]))
        else {
            if self.attempts > 0 {
                info!(
                    scope = logging::SCOPE_PROCESSING,
                    %track_id,
                    attempts = self.attempts,
                    reason = "no_plate_found",
                    "Plate recognition finished without a result"
                );
                self.save_comparison(track_id, None, &fusion, None);
            }
            return None;
        };
        let selected = &self.observations[best];
        let runner_up = self
            .observations
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != best)
            .map(|(_, result)| result)
            .max_by(|a, b| self.compare(a, b));
        let reason = match runner_up {
            None => "only_result",
            Some(other) if self.votes(selected) > self.votes(other) => "number_votes",
            Some(other) if selected.confidence() > other.confidence() => "ocr_confidence",
            Some(other) if selected.plate.confidence > other.plate.confidence => "plate_confidence",
            Some(_) => "equal_scores_latest_attempt",
        };
        info!(
            scope = logging::SCOPE_PROCESSING,
            %track_id,
            attempts = self.attempts,
            selected_attempt = selected.attempt,
            number = selected.number().unwrap_or(""),
            ocr_confidence = selected.confidence(),
            matching_observations = self.votes(selected),
            confirmed = self.confirmed,
            reason,
            "Plate recognition reference frame selected"
        );
        let ocr = fusion
            .ocr_summary(selected.attempt, &selected.plate)
            .or_else(|| {
                // Preserve the selected reading when observations cannot be aligned into one result.
                fusion::analyze([(selected.attempt, &selected.plate)])
                    .ocr_summary(selected.attempt, &selected.plate)
            });
        self.save_comparison(track_id, Some(selected), &fusion, ocr.as_ref());
        let selected = self.observations.swap_remove(best);
        Some(RecognitionResult {
            vehicle: selected.vehicle,
            plate: PlateResult {
                class: selected.plate.class,
                confidence: selected.plate.confidence,
                bbox: selected.plate.bbox,
                ocr,
            },
            frame_size: selected.frame_size,
        })
    }

    fn compare(&self, a: &PlateObservation, b: &PlateObservation) -> Ordering {
        self.votes(a)
            .cmp(&self.votes(b))
            .then_with(|| a.confidence().total_cmp(&b.confidence()))
            .then_with(|| a.plate.confidence.total_cmp(&b.plate.confidence))
    }

    fn votes(&self, observation: &PlateObservation) -> usize {
        let Some(number) = observation.number() else {
            return 0;
        };
        self.observations
            .iter()
            .filter(|other| other.number() == Some(number))
            .count()
    }

    fn save_comparison(
        &self,
        track_id: Uuid,
        selected: Option<&PlateObservation>,
        fusion: &fusion::FusionResult,
        ocr: Option<&fusion::OcrSummary>,
    ) {
        let current = selected.map(|s| SelectedReading {
            attempt: s.attempt,
            number: s.number(),
            mean_confidence: s.plate.ocr.as_ref().map(|ocr| ocr.confidence),
            matching_observations: self.votes(s),
            confirmed: self.confirmed,
        });
        let differs = fusion
            .hypothesis
            .as_deref()
            .zip(selected.and_then(|s| s.number()))
            .map(|(merged, current)| merged != current);
        let details = TrackComparison {
            track_id,
            attempts: self.attempts,
            current,
            ocr,
            fusion,
            differs,
        };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            std::fs::create_dir_all("plate_crops")?;
            std::fs::write(
                format!("plate_crops/{track_id}-summary.json"),
                serde_json::to_vec_pretty(&details)?,
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            warn!(scope = logging::SCOPE_PROCESSING, %track_id, %error, "Can't save temporary OCR comparison JSON");
        }
    }

    fn attempt(&mut self, models: &mut PlateModels, track_id: Uuid, candidate: Candidate) {
        if self.confirmed || self.attempts >= MAX_ATTEMPTS {
            return;
        }
        self.attempts += 1;
        self.last_attempt_bbox = Some(candidate.vehicle.bbox);
        debug!(
            scope = logging::SCOPE_PROCESSING,
            %track_id,
            attempt = self.attempts,
            "Plate recognition attempt"
        );

        let local_vehicle = BoundingBox {
            x: candidate.vehicle.bbox.x - candidate.crop_bbox.x,
            y: candidate.vehicle.bbox.y - candidate.crop_bbox.y,
            width: candidate.vehicle.bbox.width,
            height: candidate.vehicle.bbox.height,
        };
        if let Err(error) = save_debug_crop(
            &candidate.crop,
            &local_vehicle,
            &format!("plate_crops/{track_id}-{}-vehicle.png", self.attempts),
        ) {
            warn!(scope = logging::SCOPE_PROCESSING, %track_id, %error, "Can't save temporary vehicle crop");
        }
        let mut plate = match models.detect_plate(&candidate.crop, &local_vehicle) {
            Ok(Some(plate)) => plate,
            Ok(None) => {
                save_attempt_details(
                    track_id,
                    self.attempts,
                    &candidate,
                    None,
                    None,
                    OcrDecision::NoPlate,
                    None,
                );
                return;
            }
            Err(error) => {
                warn!(scope = logging::SCOPE_PROCESSING, %track_id, %error, "Plate detection failed");
                save_attempt_details(
                    track_id,
                    self.attempts,
                    &candidate,
                    None,
                    None,
                    OcrDecision::DetectionFailed,
                    Some(&error),
                );
                return;
            }
        };
        if let Err(error) = save_plate_crop(&candidate.crop, &plate.bbox, track_id, self.attempts) {
            warn!(scope = logging::SCOPE_PROCESSING, %track_id, %error, "Can't save temporary plate crop");
        }
        let plate_quality = PlateQuality::measure(&candidate.crop, &plate.bbox);
        let ocr_decision = if models.ocr.is_some() {
            OcrDecision::PlateFound
        } else {
            OcrDecision::Disabled
        };
        let ocr_error = if ocr_decision.should_recognize() {
            models.recognize_plate(&candidate.crop, &mut plate).err()
        } else {
            None
        };
        if let Some(error) = &ocr_error {
            warn!(scope = logging::SCOPE_PROCESSING, %track_id, %error, "Plate OCR failed");
        }

        plate.bbox.x += candidate.crop_bbox.x;
        plate.bbox.y += candidate.crop_bbox.y;
        if let Some(ocr) = &mut plate.ocr {
            for symbol in &mut ocr.symbols {
                symbol.bbox.x += candidate.crop_bbox.x;
                symbol.bbox.y += candidate.crop_bbox.y;
            }
        }
        save_attempt_details(
            track_id,
            self.attempts,
            &candidate,
            Some(&plate),
            plate_quality.as_ref(),
            ocr_decision,
            ocr_error.as_deref(),
        );
        self.observations.push(PlateObservation {
            vehicle: candidate.vehicle,
            plate,
            frame_size: candidate.frame_size,
            attempt: self.attempts,
        });
        self.confirmed = self
            .observations
            .iter()
            .any(|result| self.votes(result) >= 2);
        if self.confirmed {
            self.pending = None;
        }
    }
}

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OcrDecision {
    PlateFound,
    Disabled,
    NoPlate,
    DetectionFailed,
}

impl OcrDecision {
    fn should_recognize(self) -> bool {
        matches!(self, Self::PlateFound)
    }
}

#[derive(Serialize)]
struct SelectedReading<'a> {
    attempt: u8,
    number: Option<&'a str>,
    mean_confidence: Option<f32>,
    matching_observations: usize,
    confirmed: bool,
}

#[derive(Serialize)]
struct TrackComparison<'a> {
    track_id: Uuid,
    attempts: u8,
    current: Option<SelectedReading<'a>>,
    ocr: Option<&'a fusion::OcrSummary>,
    fusion: &'a fusion::FusionResult,
    differs: Option<bool>,
}

#[derive(Serialize)]
struct AttemptDetails<'a> {
    track_id: Uuid,
    attempt: u8,
    quality: &'a CandidateQuality,
    vehicle: &'a VehicleDetection,
    plate: Option<&'a PlateDetection>,
    plate_quality: Option<&'a PlateQuality>,
    ocr_decision: OcrDecision,
    frame_width: u32,
    frame_height: u32,
    error: Option<&'a str>,
}

// Temporary companion to the PNG; failed attempts also get JSON so none are hidden.
fn save_attempt_details(
    track_id: Uuid,
    attempt: u8,
    candidate: &Candidate,
    plate: Option<&PlateDetection>,
    plate_quality: Option<&PlateQuality>,
    ocr_decision: OcrDecision,
    error: Option<&str>,
) {
    let details = AttemptDetails {
        track_id,
        attempt,
        quality: &candidate.quality,
        vehicle: &candidate.vehicle,
        plate,
        plate_quality,
        ocr_decision,
        frame_width: candidate.frame_size.0,
        frame_height: candidate.frame_size.1,
        error,
    };
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all("plate_crops")?;
        std::fs::write(
            format!("plate_crops/{track_id}-{attempt}.json"),
            serde_json::to_vec_pretty(&details)?,
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        warn!(scope = logging::SCOPE_PROCESSING, %track_id, attempt, %error, "Can't save temporary plate attempt JSON");
    }
}

fn moved_enough(previous: &BoundingBox, current: &BoundingBox) -> bool {
    let dx = (current.x as f64 + current.width as f64 / 2.0
        - previous.x as f64
        - previous.width as f64 / 2.0)
        / previous.width.max(1) as f64;
    let dy = (current.y as f64 + current.height as f64 / 2.0
        - previous.y as f64
        - previous.height as f64 / 2.0)
        / previous.height.max(1) as f64;
    dx * dx + dy * dy >= 1.0
}
