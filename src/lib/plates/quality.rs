use serde::Serialize;

use crate::lib::cv::RawFrame;
use crate::lib::vehicle_events::BoundingBox;

const SAMPLE_SIZE: usize = 64;
// Use a vehicle-relative border margin, independent of frame resolution.
const EDGE_MARGIN: f64 = 0.1;

#[derive(Serialize)]
pub(super) struct PlateQuality {
    area: u64,
    laplacian_variance: f64,
    score: f64,
}

impl PlateQuality {
    pub fn measure(frame: &RawFrame, bbox: &BoundingBox) -> Option<Self> {
        if bbox.width == 0
            || bbox.height == 0
            || bbox.x.checked_add(bbox.width)? > frame.width
            || bbox.y.checked_add(bbox.height)? > frame.height
        {
            return None;
        }
        let area = bbox.width as u64 * bbox.height as u64;
        // Measure the detected plate itself, excluding the surrounding OCR padding.
        let laplacian_variance = laplacian_variance(frame, bbox);
        let score = (area as f64).sqrt() * (1.0 + laplacian_variance.ln_1p()).sqrt();
        Some(Self {
            area,
            laplacian_variance,
            score,
        })
    }
}

#[derive(Serialize)]
pub(super) struct CandidateQuality {
    area: u64,
    touching_edges: u32,
    edge_penalty: f64,
    estimated_plate_visibility: Option<f64>,
    laplacian_variance: f64,
    score: f64,
}

impl CandidateQuality {
    pub fn measure(frame: &RawFrame, bbox: &BoundingBox) -> Option<Self> {
        let right = bbox.x.checked_add(bbox.width)?;
        let bottom = bbox.y.checked_add(bbox.height)?;
        if bbox.width == 0 || bbox.height == 0 || right > frame.width || bottom > frame.height {
            return None;
        }
        // A clamped bbox cannot tell how much was cut off; touching an edge is only a proxy.
        let touching_edges = u32::from(bbox.x == 0)
            + u32::from(bbox.y == 0)
            + u32::from(right == frame.width)
            + u32::from(bottom == frame.height);
        let area = bbox.width as u64 * bbox.height as u64;
        let sharpness = laplacian_variance(frame, bbox);
        let mut quality = Self {
            area,
            touching_edges,
            edge_penalty: 1.0,
            estimated_plate_visibility: None,
            laplacian_variance: sharpness,
            score: 0.0,
        };
        quality.update_visibility(bbox, (frame.width, frame.height), None);
        Some(quality)
    }

    pub fn update_visibility(
        &mut self,
        bbox: &BoundingBox,
        frame_size: (u32, u32),
        plate_reference: Option<(&BoundingBox, &BoundingBox)>,
    ) {
        let gaps = [
            (bbox.x as f64, bbox.width as f64),
            (
                (frame_size.0 - bbox.x - bbox.width) as f64,
                bbox.width as f64,
            ),
            (bbox.y as f64, bbox.height as f64),
            (
                (frame_size.1 - bbox.y - bbox.height) as f64,
                bbox.height as f64,
            ),
        ];
        // A detector can stop its bbox short of the image edge even when the car is cut off.
        self.edge_penalty = 1.0
            + gaps
                .iter()
                .map(|(gap, size)| (1.0 - gap / (size * EDGE_MARGIN)).clamp(0.0, 1.0))
                .sum::<f64>();
        self.estimated_plate_visibility = plate_reference
            .map(|(vehicle, plate)| estimated_plate_visibility(bbox, frame_size, vehicle, plate));
        // Logarithmic weighting limits the advantage from texture and noise. No blur cutoff.
        self.score = (self.area as f64).sqrt() * (1.0 + self.laplacian_variance.ln_1p()).sqrt()
            / self.edge_penalty
            * self.estimated_plate_visibility.unwrap_or(1.0).powi(2);
    }

    pub fn is_better_than(&self, other: &Self) -> bool {
        self.score > other.score
    }
}

fn estimated_plate_visibility(
    current: &BoundingBox,
    frame_size: (u32, u32),
    previous: &BoundingBox,
    plate: &BoundingBox,
) -> f64 {
    // Clipping can shrink one dimension. Uniform scaling avoids squeezing the plate back in.
    let scale = (current.width as f64 / previous.width.max(1) as f64)
        .max(current.height as f64 / previous.height.max(1) as f64);
    let projected_x = projected_origin(
        current.x,
        current.width,
        frame_size.0,
        previous.width,
        scale,
    ) + (plate.x as f64 - previous.x as f64) * scale;
    let projected_y = projected_origin(
        current.y,
        current.height,
        frame_size.1,
        previous.height,
        scale,
    ) + (plate.y as f64 - previous.y as f64) * scale;
    let width = plate.width as f64 * scale;
    let height = plate.height as f64 * scale;
    // Plate detection receives the vehicle crop, so visibility within that crop matters too.
    let visible_width = ((projected_x + width).min((current.x + current.width) as f64)
        - projected_x.max(current.x as f64))
    .max(0.0);
    let visible_height = ((projected_y + height).min((current.y + current.height) as f64)
        - projected_y.max(current.y as f64))
    .max(0.0);
    (visible_width * visible_height / (width * height).max(f64::EPSILON)).clamp(0.0, 1.0)
}

fn projected_origin(start: u32, size: u32, limit: u32, previous_size: u32, scale: f64) -> f64 {
    // Anchor at the edge farther from the image border, which is less likely to be clipped.
    if start < limit - start - size {
        (start + size) as f64 - previous_size as f64 * scale
    } else {
        start as f64
    }
}

fn grayscale(frame: &RawFrame, x: u32, y: u32) -> f64 {
    let offset = y as usize * frame.step() + x as usize * 3;
    let b = frame.data[offset] as u32;
    let g = frame.data[offset + 1] as u32;
    let r = frame.data[offset + 2] as u32;
    (29 * b + 150 * g + 77 * r) as f64 / 256.0
}

fn laplacian_variance(frame: &RawFrame, bbox: &BoundingBox) -> f64 {
    if bbox.width < 3 || bbox.height < 3 {
        return 0.0;
    }
    let mut gray = [0.0; SAMPLE_SIZE * SAMPLE_SIZE];
    // Bilinear sampling compares crops at the same resolution without allocating a resized frame.
    for row in 0..SAMPLE_SIZE {
        let source_y = ((row as f64 + 0.5) * bbox.height as f64 / SAMPLE_SIZE as f64 - 0.5)
            .clamp(0.0, (bbox.height - 1) as f64);
        let y0 = source_y.floor() as u32;
        let y1 = (y0 + 1).min(bbox.height - 1);
        let fy = source_y - y0 as f64;
        for column in 0..SAMPLE_SIZE {
            let source_x = ((column as f64 + 0.5) * bbox.width as f64 / SAMPLE_SIZE as f64 - 0.5)
                .clamp(0.0, (bbox.width - 1) as f64);
            let x0 = source_x.floor() as u32;
            let x1 = (x0 + 1).min(bbox.width - 1);
            let fx = source_x - x0 as f64;
            let top = grayscale(frame, bbox.x + x0, bbox.y + y0) * (1.0 - fx)
                + grayscale(frame, bbox.x + x1, bbox.y + y0) * fx;
            let bottom = grayscale(frame, bbox.x + x0, bbox.y + y1) * (1.0 - fx)
                + grayscale(frame, bbox.x + x1, bbox.y + y1) * fx;
            gray[row * SAMPLE_SIZE + column] = top * (1.0 - fy) + bottom * fy;
        }
    }

    let mut sum = 0.0;
    let mut sum_squares = 0.0;
    // Exclude the outer border so artificial crop edges do not increase sharpness.
    for y in 1..SAMPLE_SIZE - 1 {
        for x in 1..SAMPLE_SIZE - 1 {
            let i = y * SAMPLE_SIZE + x;
            let laplacian =
                gray[i - 1] + gray[i + 1] + gray[i - SAMPLE_SIZE] + gray[i + SAMPLE_SIZE]
                    - 4.0 * gray[i];
            sum += laplacian;
            sum_squares += laplacian * laplacian;
        }
    }
    let count = ((SAMPLE_SIZE - 2) * (SAMPLE_SIZE - 2)) as f64;
    (sum_squares / count - (sum / count).powi(2)).max(0.0)
}
