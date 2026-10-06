use crate::lib::cv::RawFrame;
use crate::lib::vehicle_events::BoundingBox;

const SAMPLE_SIZE: usize = 64;

pub(super) struct CandidateQuality {
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
        let area = bbox.width as f64 * bbox.height as f64;
        let sharpness = laplacian_variance(frame, bbox);
        // Logarithmic weighting limits the advantage from texture and noise. No blur cutoff.
        let score = area.sqrt() * (1.0 + sharpness.ln_1p()).sqrt() / (1.0 + touching_edges as f64);
        Some(Self { score })
    }

    pub fn is_better_than(&self, other: &Self) -> bool {
        self.score > other.score
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
