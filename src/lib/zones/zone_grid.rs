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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_zone(id: &str, points: Vec<(f32, f32)>) -> Zone {
        let mut zone = Zone::default();
        zone.id = id.to_string();
        zone.pixel_coordinates = points.into_iter()
            .map(|(x, y)| Point2f::new(x, y))
            .collect();
        zone
    }

    #[test]
    fn test_uninitialized_grid() {
        let grid = ZoneGrid::uninitialized();
        assert!(!grid.is_initialized());
        assert_eq!(grid.get_candidate_zones(100.0, 100.0), &[] as &[String]);
    }

    #[test]
    fn test_initialize_grid() {
        let mut grid = ZoneGrid::uninitialized();
        grid.initialize(640.0, 480.0, 32.0);

        assert!(grid.is_initialized());
        // 640/32 = 20
        assert_eq!(grid.width_cells, 20);
        // 480/32 = 15
        assert_eq!(grid.height_cells, 15);
        // 20 * 15
        assert_eq!(grid.cells.len(), 300);
    }

    #[test]
    fn test_get_candidate_zones_out_of_bounds() {
        let mut grid = ZoneGrid::uninitialized();
        grid.initialize(640.0, 480.0, 32.0);

        // Negative coordinates
        assert_eq!(grid.get_candidate_zones(-1.0, 100.0), &[] as &[String]);
        assert_eq!(grid.get_candidate_zones(100.0, -1.0), &[] as &[String]);

        // Beyond frame bounds
        assert_eq!(grid.get_candidate_zones(640.0, 100.0), &[] as &[String]);
        assert_eq!(grid.get_candidate_zones(100.0, 480.0), &[] as &[String]);
        assert_eq!(grid.get_candidate_zones(1000.0, 1000.0), &[] as &[String]);
    }

    #[test]
    fn test_get_candidate_zones_empty_grid() {
        let mut grid = ZoneGrid::uninitialized();
        grid.initialize(640.0, 480.0, 32.0);

        // Valid point but no zones registered
        assert_eq!(grid.get_candidate_zones(100.0, 100.0), &[] as &[String]);
    }

    #[test]
    fn test_rebuild_single_zone() {
        let mut grid = ZoneGrid::uninitialized();
        grid.initialize(640.0, 480.0, 32.0);

        // Create zone covering cells (0,0) to (2,2) - roughly 0-64px in both dimensions
        let zone = create_test_zone("zone1", vec![
            (10.0, 10.0), (60.0, 10.0), (60.0, 60.0), (10.0, 60.0)
        ]);

        let mut zones = HashMap::new();
        zones.insert("zone1".to_string(), Mutex::new(zone));

        grid.rebuild(&zones);

        // Point inside zone's bounding box
        let candidates = grid.get_candidate_zones(30.0, 30.0);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], "zone1");

        // Point outside zone's bounding box
        let candidates = grid.get_candidate_zones(200.0, 200.0);
        assert_eq!(candidates.len(), 0);
    }

    #[test]
    fn test_rebuild_multiple_zones() {
        let mut grid = ZoneGrid::uninitialized();
        grid.initialize(640.0, 480.0, 32.0);

        // Zone 1: top-left area
        let zone1 = create_test_zone("zone1", vec![
            (0.0, 0.0), (64.0, 0.0), (64.0, 64.0), (0.0, 64.0)
        ]);

        // Zone 2: overlapping with zone1
        let zone2 = create_test_zone("zone2", vec![
            (32.0, 32.0), (128.0, 32.0), (128.0, 128.0), (32.0, 128.0)
        ]);

        // Zone 3: separate area
        let zone3 = create_test_zone("zone3", vec![
            (400.0, 300.0), (500.0, 300.0), (500.0, 400.0), (400.0, 400.0)
        ]);

        let mut zones = HashMap::new();
        zones.insert("zone1".to_string(), Mutex::new(zone1));
        zones.insert("zone2".to_string(), Mutex::new(zone2));
        zones.insert("zone3".to_string(), Mutex::new(zone3));

        grid.rebuild(&zones);

        // Point only in zone1
        let candidates = grid.get_candidate_zones(16.0, 16.0);
        assert_eq!(candidates.len(), 1);
        assert!(candidates.contains(&"zone1".to_string()));

        // Point in overlap area (both zone1 and zone2)
        let candidates = grid.get_candidate_zones(48.0, 48.0);
        assert_eq!(candidates.len(), 2);
        assert!(candidates.contains(&"zone1".to_string()));
        assert!(candidates.contains(&"zone2".to_string()));

        // Point only in zone3
        let candidates = grid.get_candidate_zones(450.0, 350.0);
        assert_eq!(candidates.len(), 1);
        assert!(candidates.contains(&"zone3".to_string()));
    }

    #[test]
    fn test_rebuild_clears_previous_data() {
        let mut grid = ZoneGrid::uninitialized();
        grid.initialize(640.0, 480.0, 32.0);

        // First rebuild with zone1
        let zone1 = create_test_zone("zone1", vec![
            (0.0, 0.0), (64.0, 0.0), (64.0, 64.0), (0.0, 64.0)
        ]);
        let mut zones = HashMap::new();
        zones.insert("zone1".to_string(), Mutex::new(zone1));
        grid.rebuild(&zones);

        assert_eq!(grid.get_candidate_zones(16.0, 16.0).len(), 1);

        // Second rebuild with different zone
        let zone2 = create_test_zone("zone2", vec![
            (200.0, 200.0), (300.0, 200.0), (300.0, 300.0), (200.0, 300.0)
        ]);
        let mut zones = HashMap::new();
        zones.insert("zone2".to_string(), Mutex::new(zone2));
        grid.rebuild(&zones);

        // Old zone should be gone
        assert_eq!(grid.get_candidate_zones(16.0, 16.0).len(), 0);
        // New zone should be present
        assert_eq!(grid.get_candidate_zones(250.0, 250.0).len(), 1);
    }

    #[test]
    fn test_rebuild_uninitialized_grid_no_panic() {
        let mut grid = ZoneGrid::uninitialized();

        let zone = create_test_zone("zone1", vec![
            (10.0, 10.0), (60.0, 10.0), (60.0, 60.0), (10.0, 60.0)
        ]);
        let mut zones = HashMap::new();
        zones.insert("zone1".to_string(), Mutex::new(zone));

        // Should not panic
        grid.rebuild(&zones);
        assert!(!grid.is_initialized());
    }

    #[test]
    fn test_stats() {
        let mut grid = ZoneGrid::uninitialized();
        // 4x4 grid = 16 cells
        grid.initialize(128.0, 128.0, 32.0);

        let zone = create_test_zone("zone1", vec![
            (0.0, 0.0), (64.0, 0.0), (64.0, 64.0), (0.0, 64.0)
        ]);
        let mut zones = HashMap::new();
        zones.insert("zone1".to_string(), Mutex::new(zone));
        grid.rebuild(&zones);

        let stats = grid.stats();
        assert_eq!(stats.total_cells, 16);
        // 2x2 cells covered
        assert_eq!(stats.non_empty_cells, 4);
        assert_eq!(stats.max_zones_per_cell, 1);
        assert_eq!(stats.grid_dimensions, (4, 4));
    }

    #[test]
    fn test_edge_of_frame() {
        let mut grid = ZoneGrid::uninitialized();
        grid.initialize(640.0, 480.0, 32.0);

        // Zone at the edge of frame
        let zone = create_test_zone("edge_zone", vec![
            (600.0, 440.0), (640.0, 440.0), (640.0, 480.0), (600.0, 480.0)
        ]);
        let mut zones = HashMap::new();
        zones.insert("edge_zone".to_string(), Mutex::new(zone));
        grid.rebuild(&zones);

        // Point just inside the edge
        let candidates = grid.get_candidate_zones(620.0, 460.0);
        assert_eq!(candidates.len(), 1);

        // Point at exact edge (should be out of bounds)
        let candidates = grid.get_candidate_zones(640.0, 480.0);
        assert_eq!(candidates.len(), 0);
    }

    #[test]
    fn test_compute_bbox_empty() {
        let result = ZoneGrid::compute_bbox(&[]);
        assert!(result.is_none());
    }

    #[test]
    fn test_compute_bbox_single_point() {
        let points = vec![Point2f::new(100.0, 200.0)];
        let result = ZoneGrid::compute_bbox(&points);
        assert_eq!(result, Some((100.0, 200.0, 100.0, 200.0)));
    }

    #[test]
    fn test_compute_bbox_multiple_points() {
        let points = vec![
            Point2f::new(10.0, 20.0),
            Point2f::new(50.0, 30.0),
            Point2f::new(30.0, 60.0),
        ];
        let result = ZoneGrid::compute_bbox(&points);
        assert_eq!(result, Some((10.0, 20.0, 50.0, 60.0)));
    }
}
