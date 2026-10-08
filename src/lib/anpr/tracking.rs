use std::cmp::Ordering;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::lib::anpr::types::{BoundingBox, PlateDetection, PlateResult, VehicleDetection};
use crate::lib::cv::RawFrame;
use crate::lib::logging;

use crate::lib::anpr::crops::crop_frame;
use crate::lib::anpr::fusion;
use crate::lib::anpr::plate_detector::PlateModels;
use crate::lib::anpr::quality::CandidateQuality;

const MAX_ATTEMPTS: u8 = 3;

struct Candidate {
    crop: RawFrame,
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
        Some(Self {
            crop: crop_frame(frame, &vehicle.bbox)?,
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

pub struct RecognitionResult {
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
pub struct TrackRecognition {
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
        let fusion = fusion::analyze(self.observations.iter().map(|o| (o.attempt, &o.plate)));
        let ocr = fusion::summarize(&fusion, selected.attempt, &selected.plate).or_else(|| {
            // Preserve the selected reading when observations cannot be aligned into one result.
            let fallback = fusion::analyze([(selected.attempt, &selected.plate)]);
            fusion::summarize(&fallback, selected.attempt, &selected.plate)
        });
        let selected = self.observations.swap_remove(best);
        let plate_bbox = selected.plate.bbox.relative_to(selected.vehicle.bbox);
        Some(RecognitionResult {
            vehicle: selected.vehicle,
            plate: PlateResult {
                class: selected.plate.class,
                confidence: selected.plate.confidence,
                bbox: plate_bbox,
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

        let mut plate = match models.detect_plate(&candidate.crop) {
            Ok(Some(plate)) => plate,
            Ok(None) => return,
            Err(error) => {
                warn!(scope = logging::SCOPE_PROCESSING, %track_id, %error, "Plate detection failed");
                return;
            }
        };
        if let Err(error) = models.recognize_plate(&candidate.crop, &mut plate) {
            warn!(scope = logging::SCOPE_PROCESSING, %track_id, %error, "Plate OCR failed");
        }

        plate.bbox.x += candidate.vehicle.bbox.x;
        plate.bbox.y += candidate.vehicle.bbox.y;
        if let Some(ocr) = &mut plate.ocr {
            for symbol in &mut ocr.symbols {
                symbol.bbox.x += candidate.vehicle.bbox.x;
                symbol.bbox.y += candidate.vehicle.bbox.y;
            }
        }
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
