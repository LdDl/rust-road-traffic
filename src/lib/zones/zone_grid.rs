use std::collections::HashMap;
use std::sync::Mutex;
use opencv::core::Point2f;

use super::zones::Zone;

/// Spatial index for O(1) zone lookup
/// Divides frame into grid cells, each cell stores which zones overlap it
pub struct ZoneGrid {
    /// 2D grid stored as flat Vec for cache efficiency
    /// Index = row * width_cells + col
    /// Each cell contains zone IDs that overlap that cell
    cells: Vec<Vec<String>>,
    cell_size: f32,
    width_cells: usize,
    height_cells: usize,
    frame_width: f32,
    frame_height: f32,
}

impl ZoneGrid {
    /// Create uninitialized grid (for when frame dimensions unknown)
    pub fn uninitialized() -> Self {
        Self {
            cells: Vec::new(),
            cell_size: 32.0,
            width_cells: 0,
            height_cells: 0,
            frame_width: 0.0,
            frame_height: 0.0,
        }
    }

    /// Check if grid has been initialized with frame dimensions
    pub fn is_initialized(&self) -> bool {
        self.width_cells > 0 && self.height_cells > 0
    }

    /// Initialize grid with frame dimensions (call once when frame size known)
    pub fn initialize(&mut self, frame_width: f32, frame_height: f32, cell_size: f32) {
        self.frame_width = frame_width;
        self.frame_height = frame_height;
        self.cell_size = cell_size;
        self.width_cells = (frame_width / cell_size).ceil() as usize;
        self.height_cells = (frame_height / cell_size).ceil() as usize;
        let total_cells = self.width_cells * self.height_cells;
        self.cells = vec![Vec::new(); total_cells];
    }

    /// Rebuild grid from current zones
    /// Call this after any zone mutation (create, update, delete)
    pub fn rebuild(&mut self, zones: &HashMap<String, Mutex<Zone>>) {
        // Clear all cells
        for cell in &mut self.cells {
            cell.clear();
        }

        if !self.is_initialized() {
            return;
        }

        // For each zone, find which cells it overlaps
        for (zone_id, zone_mutex) in zones.iter() {
            let zone = match zone_mutex.lock() {
                Ok(z) => z,
                Err(_) => continue, // Skip poisoned mutex
            };

            // Get zone bounding box from pixel_coordinates
            let bbox = Self::compute_bbox(&zone.pixel_coordinates);
            if bbox.is_none() {
                continue;
            }
            let (min_x, min_y, max_x, max_y) = bbox.unwrap();

            // Convert to cell indices (clamped to grid bounds)
            let col_start = ((min_x / self.cell_size).floor() as usize).min(self.width_cells.saturating_sub(1));
            let col_end = ((max_x / self.cell_size).ceil() as usize).min(self.width_cells);
            let row_start = ((min_y / self.cell_size).floor() as usize).min(self.height_cells.saturating_sub(1));
            let row_end = ((max_y / self.cell_size).ceil() as usize).min(self.height_cells);

            // Mark all cells in bounding box as containing this zone
            for row in row_start..row_end {
                for col in col_start..col_end {
                    let idx = row * self.width_cells + col;
                    if idx < self.cells.len() {
                        self.cells[idx].push(zone_id.clone());
                    }
                }
            }
        }
    }

    /// Get candidate zone IDs for a point
    /// Returns empty slice if point is outside frame bounds
    #[inline]
    pub fn get_candidate_zones(&self, x: f32, y: f32) -> &[String] {
        if !self.is_initialized() || x < 0.0 || y < 0.0 || x >= self.frame_width || y >= self.frame_height {
            return &[];
        }

        let col = (x / self.cell_size) as usize;
        let row = (y / self.cell_size) as usize;

        // Bounds check (shouldn't fail if frame bounds check passed, but be safe)
        if col >= self.width_cells || row >= self.height_cells {
            return &[];
        }

        let idx = row * self.width_cells + col;
        &self.cells[idx]
    }

    /// Compute axis-aligned bounding box of polygon
    fn compute_bbox(points: &[Point2f]) -> Option<(f32, f32, f32, f32)> {
        if points.is_empty() {
            return None;
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for pt in points {
            min_x = min_x.min(pt.x);
            min_y = min_y.min(pt.y);
            max_x = max_x.max(pt.x);
            max_y = max_y.max(pt.y);
        }

        Some((min_x, min_y, max_x, max_y))
    }

    /// Get grid statistics for debugging
    #[allow(dead_code)]
    pub fn stats(&self) -> ZoneGridStats {
        let mut non_empty = 0;
        let mut max_zones_per_cell = 0;
        let mut total_zone_refs = 0;

        for cell in &self.cells {
            if !cell.is_empty() {
                non_empty += 1;
                max_zones_per_cell = max_zones_per_cell.max(cell.len());
                total_zone_refs += cell.len();
            }
        }

        ZoneGridStats {
            total_cells: self.cells.len(),
            non_empty_cells: non_empty,
            max_zones_per_cell,
            avg_zones_per_non_empty: if non_empty > 0 { total_zone_refs as f32 / non_empty as f32 } else { 0.0 },
            cell_size: self.cell_size,
            grid_dimensions: (self.width_cells, self.height_cells),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ZoneGridStats {
    pub total_cells: usize,
    pub non_empty_cells: usize,
    pub max_zones_per_cell: usize,
    pub avg_zones_per_non_empty: f32,
    pub cell_size: f32,
    pub grid_dimensions: (usize, usize),
}
