use tracing::{debug, warn};
use uuid::Uuid;

use crate::lib::cv::RawFrame;
use crate::lib::logging;
use crate::lib::vehicle_events::{BoundingBox, PlateDetection, VehicleDetection};

use super::quality::CandidateQuality;
use super::{PlateModels, crop_frame, ocr_crop_bbox, save_plate_crop};

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

pub(crate) struct PlateObservation {
    pub vehicle: VehicleDetection,
    pub plate: PlateDetection,
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
        self.plate.ocr.as_ref().map_or(0.0, |ocr| {
            let count = ocr.symbols.len().max(1) as f32;
            ocr.symbols
                .iter()
                .map(|symbol| symbol.confidence)
                .sum::<f32>()
                / count
        })
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
        let Some(quality) = CandidateQuality::measure(frame, &vehicle.bbox) else {
            return;
        };

        // Reserve the last attempt for track completion.
        let should_attempt = self.attempts == 0
            || (self.attempts == 1
                && self
                    .last_attempt_bbox
                    .as_ref()
                    .is_some_and(|previous| moved_enough(previous, &vehicle.bbox)));
        if should_attempt {
            if let Some(candidate) = Candidate::new(frame, vehicle, quality) {
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
    ) -> Option<PlateObservation> {
        if let (Some(models), Some(candidate)) = (models, self.pending.take()) {
            self.attempt(models, track_id, candidate);
        }
        let best = (0..self.observations.len()).max_by(|&a, &b| {
            let a = &self.observations[a];
            let b = &self.observations[b];
            self.votes(a)
                .cmp(&self.votes(b))
                .then_with(|| a.confidence().total_cmp(&b.confidence()))
                .then_with(|| a.plate.confidence.total_cmp(&b.plate.confidence))
        })?;
        Some(self.observations.swap_remove(best))
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

        let local_vehicle = BoundingBox {
            x: candidate.vehicle.bbox.x - candidate.crop_bbox.x,
            y: candidate.vehicle.bbox.y - candidate.crop_bbox.y,
            width: candidate.vehicle.bbox.width,
            height: candidate.vehicle.bbox.height,
        };
        let mut plate = match models.detect_plate(&candidate.crop, &local_vehicle) {
            Ok(Some(plate)) => plate,
            Ok(None) => return,
            Err(error) => {
                warn!(scope = logging::SCOPE_PROCESSING, %track_id, %error, "Plate detection failed");
                return;
            }
        };
        if let Err(error) = save_plate_crop(&candidate.crop, &plate.bbox, track_id, self.attempts) {
            warn!(scope = logging::SCOPE_PROCESSING, %track_id, %error, "Can't save temporary plate crop");
        }
        if let Err(error) = models.recognize_plate(&candidate.crop, &mut plate) {
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
        self.observations.push(PlateObservation {
            vehicle: candidate.vehicle,
            plate,
            frame_size: candidate.frame_size,
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
