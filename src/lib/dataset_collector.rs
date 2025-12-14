use opencv::{
    prelude::*,
    core::Mat,
    core::Rect as RectCV,
    imgcodecs::imwrite,
    core::Vector,
};

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use uuid::Uuid;

use crate::settings::DatasetCollectorSettings;

/// Tracks capture state for each object track
#[derive(Debug, Clone)]
struct TrackCaptureState {
    /// Number of captures already done for this track
    capture_count: u32,
    /// Frame number when last capture was done
    last_capture_frame: u64,
}

/// Dataset collector for saving images and YOLO annotations
pub struct DatasetCollector {
    settings: DatasetCollectorSettings,
    images_dir: String,
    labels_dir: String,
    /// Maps track UUID to its capture state
    track_states: HashMap<Uuid, TrackCaptureState>,
    /// Global frame counter
    frame_counter: u64,
    /// Class name to class ID mapping
    class_to_id: HashMap<String, usize>,
}

impl DatasetCollector {
    pub fn new(settings: DatasetCollectorSettings, net_classes: &[String]) -> Result<Self, Box<dyn std::error::Error>> {
        let images_dir = format!("{}/images", settings.output_dir);
        let labels_dir = format!("{}/labels", settings.output_dir);

        // Create directories if they don't exist
        fs::create_dir_all(&images_dir)?;
        fs::create_dir_all(&labels_dir)?;

        // Build class name to ID mapping
        let class_to_id: HashMap<String, usize> = net_classes
            .iter()
            .enumerate()
            .map(|(i, name)| (name.clone(), i))
            .collect();

        println!("[DatasetCollector] Initialized with output_dir: {}", settings.output_dir);
        println!("[DatasetCollector] min_track_age: {}, max_captures_per_track: {}, capture_interval: {}",
            settings.min_track_age, settings.max_captures_per_track, settings.capture_interval);

        Ok(Self {
            settings,
            images_dir,
            labels_dir,
            track_states: HashMap::new(),
            frame_counter: 0,
            class_to_id,
        })
    }

    /// Check if a bounding box touches or is near the frame edges
    fn is_near_edge(&self, bbox: &RectCV, frame_width: i32, frame_height: i32) -> bool {
        let margin = self.settings.edge_margin_pixels as i32;

        // Check if bbox is near any edge
        bbox.x <= margin
            || bbox.y <= margin
            || (bbox.x + bbox.width) >= (frame_width - margin)
            || (bbox.y + bbox.height) >= (frame_height - margin)
    }

    /// Convert pixel bounding box to YOLO normalized format
    /// Returns: (center_x, center_y, width, height) all normalized to [0, 1]
    fn bbox_to_yolo(&self, bbox: &RectCV, frame_width: i32, frame_height: i32) -> (f64, f64, f64, f64) {
        let center_x = (bbox.x as f64 + bbox.width as f64 / 2.0) / frame_width as f64;
        let center_y = (bbox.y as f64 + bbox.height as f64 / 2.0) / frame_height as f64;
        let width = bbox.width as f64 / frame_width as f64;
        let height = bbox.height as f64 / frame_height as f64;

        (center_x, center_y, width, height)
    }

    /// Process a frame and potentially save dataset samples
    ///
    /// # Arguments
    /// * frame - Raw frame (without any drawing/annotations/resizing and other processing stuff)
    /// * bboxes - Detected bounding boxes in pixel coordinates
    /// * class_names - Class name for each detection
    /// * track_ids - Track UUID for each detection (must match order with bboxes)
    /// * track_ages - Number of points in track (track age) for each detection
    pub fn process_frame(
        &mut self,
        frame: &Mat,
        bboxes: &[RectCV],
        class_names: &[String],
        track_ids: &[Uuid],
        track_ages: &[usize],
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.frame_counter += 1;

        // Debug: log every 100 frames
        if self.frame_counter % 100 == 0 {
            println!("[DatasetCollector] Frame {}: {} detections, {} tracked objects",
                self.frame_counter, bboxes.len(), self.track_states.len());
        }

        if bboxes.is_empty() {
            return Ok(());
        }

        let frame_width = frame.cols();
        let frame_height = frame.rows();

        // Separate objects into:
        // 1. "mature" objects - old enough and not on edge (should be annotated)
        // 2. "trigger" objects - mature objects that need new captures (trigger save)
        let mut mature_objects: Vec<usize> = Vec::new();
        let mut trigger_track_ids: Vec<&Uuid> = Vec::new();
        let mut skipped_young = 0;
        let mut skipped_edge = 0;

        for (i, (bbox, track_id)) in bboxes.iter().zip(track_ids.iter()).enumerate() {
            let track_age = track_ages.get(i).copied().unwrap_or(0);

            // Check track age
            if track_age < self.settings.min_track_age as usize {
                skipped_young += 1;
                continue;
            }

            // Check edge proximity
            if self.settings.skip_edge_objects && self.is_near_edge(bbox, frame_width, frame_height) {
                skipped_edge += 1;
                continue;
            }

            // This object is mature and not on edge - should be annotated if we save
            mature_objects.push(i);

            // Check if this object should trigger a new capture
            let state = self.track_states.entry(*track_id).or_insert(TrackCaptureState {
                capture_count: 0,
                last_capture_frame: 0,
            });

            // Already reached max captures?
            if state.capture_count >= self.settings.max_captures_per_track {
                continue;
            }

            // Check capture interval (only relevant after first capture)
            if state.capture_count > 0 {
                let frames_since_last = self.frame_counter - state.last_capture_frame;
                if frames_since_last < self.settings.capture_interval as u64 {
                    continue;
                }
            }

            // This object triggers a save
            trigger_track_ids.push(track_id);
        }

        // Debug: log skip reasons every 100 frames
        if self.frame_counter % 100 == 0 && !bboxes.is_empty() {
            println!("[DatasetCollector] Skipped: {} young, {} edge. Mature: {}, Triggers: {}",
                skipped_young, skipped_edge, mature_objects.len(), trigger_track_ids.len());
        }

        // If we have trigger objects, save the frame with ALL mature objects annotated
        if !trigger_track_ids.is_empty() {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%f").to_string();
            let filename_base = format!("{}_{}", timestamp, self.frame_counter);

            // Save image
            let image_path = format!("{}/{}.jpg", self.images_dir, filename_base);
            let params = Vector::<i32>::new();
            imwrite(&image_path, frame, &params)?;

            // Build annotations for ALL mature objects (not just triggers)
            let mut annotations = String::new();
            for &i in &mature_objects {
                let bbox = &bboxes[i];
                let class_name = &class_names[i];

                // Get class ID
                let class_id = match self.class_to_id.get(class_name) {
                    Some(id) => *id,
                    None => continue,
                };

                // Convert to YOLO format
                let (cx, cy, w, h) = self.bbox_to_yolo(bbox, frame_width, frame_height);
                annotations.push_str(&format!("{} {:.6} {:.6} {:.6} {:.6}\n", class_id, cx, cy, w, h));
            }

            // Update capture state only for trigger objects
            for track_id in &trigger_track_ids {
                let state = self.track_states.get_mut(*track_id).unwrap();
                state.capture_count += 1;
                state.last_capture_frame = self.frame_counter;
            }

            // Save labels
            let label_path = format!("{}/{}.txt", self.labels_dir, filename_base);
            let mut file = fs::File::create(&label_path)?;
            file.write_all(annotations.as_bytes())?;

            println!("[DatasetCollector] SAVED: {} with {} objects (triggered by {}) -> {}",
                filename_base, mature_objects.len(), trigger_track_ids.len(), image_path);
        }

        // Cleanup old track states (optional - prevents memory growth)
        // Remove tracks that haven't been seen for a while
        if self.frame_counter % 1000 == 0 {
            self.track_states.retain(|_, state| {
                self.frame_counter - state.last_capture_frame < 500
            });
        }

        Ok(())
    }
}
