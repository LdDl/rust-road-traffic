// Export submodules
pub mod geojson;
pub mod geometry;

use chrono::{DateTime, Utc};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use geometry::{is_intersects, polygon_contains_point};

use geojson::{
    GeoPolygon, SolidLineFeature, VirtualLineFeature, ZoneFeature, ZonePropertiesGeoJSON,
};

use crate::lib::constants::EPSILON;
use crate::lib::cv::RawFrame;
use crate::lib::cv::Scalar;
use crate::lib::draw::primitives::{draw_line_thick, draw_text, scalar_to_bgr};
use crate::lib::spatial::PerspectiveTransform;
use crate::lib::spatial::Point2f;
use crate::lib::spatial::compute_center;
use crate::lib::spatial::epsg::lonlat_to_meters;
use crate::lib::spatial::haversine;
use crate::lib::zones::{
    SOLID_LINE_COLOR_RGB, Skeleton, SolidLine, Statistics, TrafficFlowParameters,
    VehicleTypeParameters, VirtualLine, VirtualLineDirection,
};
use crate::settings::LaneRulesSettings;

/// A track must be away from the virtual line for this long before another
/// crossing is counted. This suppresses bbox jitter around the line without
/// hiding a real later crossing.
const VIRTUAL_LINE_CROSSING_COOLDOWN_SEC: f32 = 0.5;

#[derive(Debug, Clone)]
struct ObjectInfo {
    classname: String,
    speed: f32,
    crossed_virtual_line: bool,
    timestamp_registration: f32,
    last_crossed_at: f32,
}

type Registered = HashMap<Uuid, ObjectInfo>;

#[derive(Debug)]
pub struct Zone {
    pub id: String,
    pub pixel_coordinates: Vec<Point2f>,
    spatial_coordinates_epsg4326: Vec<Point2f>,
    spatial_coordinates_epsg3857: Vec<Point2f>,
    pub color: Scalar,
    pub road_lane_num: u16,
    pub road_lane_direction: u8,
    spatial_converter: Option<PerspectiveTransform>,
    pub statistics: Statistics,
    objects_registered: Registered,
    objects_crossed: HashSet<Uuid>,
    pub current_statistics: RealTimeStatistics,
    skeleton: Skeleton,
    virtual_line: Option<VirtualLine>,
    /// Violation rules configured for this zone. They are not used by counting,
    /// but they are part of the zone: saving the configuration writes them back
    /// and the API shows the ones that can be edited
    rules: LaneRulesSettings,
}

#[derive(Debug)]
pub struct RealTimeStatistics {
    pub last_time: u64,
    pub last_time_relative: f32,
    pub last_time_registered: f32,
    pub occupancy: u16,
    /// Information about how many vehicles traveled from other zones to the current one.
    /// Key: zone_id_from
    /// Value: number of vehicles arrived from that zone
    pub income: HashMap<String, u32>,
}

impl Zone {
    pub fn default() -> Self {
        Zone {
            id: Uuid::new_v4().to_string(),
            pixel_coordinates: vec![],
            spatial_coordinates_epsg4326: vec![],
            spatial_coordinates_epsg3857: vec![],
            color: Scalar::from((255.0, 255.0, 255.0)),
            road_lane_num: 0,
            road_lane_direction: 0,
            spatial_converter: None,
            statistics: Statistics::default(),
            objects_registered: HashMap::new(),
            objects_crossed: HashSet::new(),
            current_statistics: RealTimeStatistics {
                last_time: 0,
                last_time_relative: 0.0,
                last_time_registered: 0.0,
                occupancy: 0,
                income: HashMap::new(),
            },
            skeleton: Skeleton::default(),
            virtual_line: None,
            rules: LaneRulesSettings::default(),
        }
    }
    pub fn new(
        id: String,
        coordinates: Vec<Point2f>,
        spatial_coordinates_epsg4326: Vec<Point2f>,
        spatial_coordinates_epsg3857: Vec<Point2f>,
        color: Scalar,
        road_lane_num: u16,
        road_lane_direction: u8,
        _virtual_line: Option<VirtualLine>,
    ) -> Self {
        /* Init skeleton */
        let skeleton_line = find_skeleton_line(&coordinates, 0, 2); // 0-1 is first segment of polygon, 2-3 is second segment
        let mut skeleton = Skeleton::new(skeleton_line[0], skeleton_line[1]);
        let converter = if spatial_coordinates_epsg4326.len() >= 4 {
            /* Eval distance between sides if spatial data is provided */
            let a = spatial_coordinates_epsg4326[0];
            let b = spatial_coordinates_epsg4326[1];
            let c = spatial_coordinates_epsg4326[2];
            let d = spatial_coordinates_epsg4326[3];
            let ab_center = compute_center(a.x, a.y, b.x, b.y);
            let cd_center = compute_center(c.x, c.y, d.x, d.y);
            let length_meters =
                haversine(ab_center.0, ab_center.1, cd_center.0, cd_center.1) * 1000.0;
            skeleton.length_meters = length_meters;
            // Guard against division by zero when skeleton endpoints are at same location
            skeleton.pixels_per_meter = if length_meters > EPSILON {
                skeleton.length_pixels / skeleton.length_meters
            } else {
                0.0 // Invalid - will be filtered in speed calculations
            };
            make_perspective_transform(&coordinates, &spatial_coordinates_epsg3857)
        } else {
            None
        };
        Zone {
            id: id,
            pixel_coordinates: coordinates,
            spatial_coordinates_epsg4326: spatial_coordinates_epsg4326,
            spatial_coordinates_epsg3857: spatial_coordinates_epsg3857,
            color: color,
            road_lane_num: road_lane_num,
            road_lane_direction: road_lane_direction,
            spatial_converter: converter,
            statistics: Statistics::default(),
            objects_registered: HashMap::new(),
            objects_crossed: HashSet::new(),
            current_statistics: RealTimeStatistics {
                last_time: 0,
                last_time_relative: 0.0,
                last_time_registered: 0.0,
                occupancy: 0,
                income: HashMap::new(),
            },
            skeleton: skeleton,
            virtual_line: _virtual_line,
            rules: LaneRulesSettings::default(),
        }
    }
    pub fn default_from_cv(points: Vec<Point2f>) -> Self {
        Zone::new(
            "dir_0_lane_0".to_owned(),
            points,
            vec![],
            vec![],
            Scalar::from((255.0, 255.0, 255.0)),
            0,
            0,
            None,
        )
    }
    pub fn get_id(&self) -> String {
        self.id.clone()
    }
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }
    pub fn set_road_lane_num(&mut self, new_value: u16) {
        self.road_lane_num = new_value;
    }
    pub fn set_road_lane_direction(&mut self, new_value: u8) {
        self.road_lane_direction = new_value;
    }
    pub fn get_pixel_coordinates(&self) -> Vec<Point2f> {
        self.pixel_coordinates.clone()
    }
    pub fn get_spatial_coordinates_epsg4326(&self) -> Vec<Point2f> {
        self.spatial_coordinates_epsg4326.clone()
    }
    pub fn set_color(&mut self, rgb: [i16; 3]) {
        // RGB to BGR
        let (b, g, r) = (rgb[2] as f64, rgb[1] as f64, rgb[0] as f64);
        self.color = Scalar::from((b, g, r));
    }
    pub fn get_color(&self) -> [i16; 3] {
        let (b, g, r) = (
            self.color[0] as i16,
            self.color[1] as i16,
            self.color[2] as i16,
        );
        [r, g, b]
    }
    pub fn set_line_color(&mut self, rgb: [i16; 3]) {
        if let Some(vline) = self.virtual_line.as_mut() {
            let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
            vline.set_color_rgb(r, g, b);
        };
    }
    pub fn update_skeleton(&mut self) {
        if self.pixel_coordinates.len() < 4 || self.spatial_coordinates_epsg4326.len() < 4 {
            return;
        }
        /* Eval distance between sides */
        let a = self.spatial_coordinates_epsg4326[0];
        let b = self.spatial_coordinates_epsg4326[1];
        let c = self.spatial_coordinates_epsg4326[2];
        let d = self.spatial_coordinates_epsg4326[3];
        let ab_center = compute_center(a.x, a.y, b.x, b.y);
        let cd_center = compute_center(c.x, c.y, d.x, d.y);
        let length_meters = haversine(ab_center.0, ab_center.1, cd_center.0, cd_center.1) * 1000.0;
        /* Init skeleton */
        let skeleton_line = find_skeleton_line(&self.pixel_coordinates, 0, 2); // 0-1 is first segment of polygon, 2-3 is second segment
        let mut skeleton = Skeleton::new(skeleton_line[0], skeleton_line[1]);
        skeleton.length_meters = length_meters;
        // Guard against division by zero when skeleton endpoints are at same location
        skeleton.pixels_per_meter = if length_meters > EPSILON {
            skeleton.length_pixels / skeleton.length_meters
        } else {
            0.0 // Invalid - will be filtered in speed calculations
        };
        self.skeleton = skeleton;
    }
    pub fn update_pixel_map_cv(&mut self, pixel_src_points: Vec<Point2f>) {
        self.pixel_coordinates = pixel_src_points;
        if self.spatial_coordinates_epsg4326.len() == 0 {
            self.spatial_coordinates_epsg4326 = self
                .pixel_coordinates
                .iter()
                .map(|pt| Point2f::new(pt.x as f32, pt.y as f32))
                .collect();
            self.spatial_coordinates_epsg3857 = self
                .spatial_coordinates_epsg4326
                .iter()
                .map(|pt| {
                    let lonlat = lonlat_to_meters(pt.x, pt.y);
                    Point2f::new(lonlat.0, lonlat.1)
                })
                .collect();
        }
        self.spatial_converter =
            make_perspective_transform(&self.pixel_coordinates, &self.spatial_coordinates_epsg3857);
        self.update_skeleton();
    }
    pub fn update_spatial_map_cv(&mut self, spatial_dest_points: Vec<Point2f>) {
        self.spatial_coordinates_epsg4326 = spatial_dest_points;
        self.spatial_coordinates_epsg3857 = self
            .spatial_coordinates_epsg4326
            .iter()
            .map(|pt| {
                let lonlat = lonlat_to_meters(pt.x, pt.y);
                Point2f::new(lonlat.0, lonlat.1)
            })
            .collect();
        if self.pixel_coordinates.len() == 0 {
            self.pixel_coordinates = self
                .spatial_coordinates_epsg3857
                .iter()
                .map(|pt| Point2f::new(pt.x as f32, pt.y as f32))
                .collect();
        }
        self.spatial_converter =
            make_perspective_transform(&self.pixel_coordinates, &self.spatial_coordinates_epsg3857);
        self.update_skeleton();
    }
    pub fn update_pixel_map(&mut self, pixel_src_points: [[u16; 2]; 4]) {
        let val = pixel_src_points
            .iter()
            .map(|pt| Point2f::new(pt[0] as f32, pt[1] as f32))
            .collect();
        self.update_pixel_map_cv(val);
    }
    pub fn update_spatial_map(&mut self, spatial_dest_points: [[f32; 2]; 4]) {
        let val = spatial_dest_points
            .iter()
            .map(|pt| Point2f::new(pt[0], pt[1]))
            .collect();
        self.update_spatial_map_cv(val);
    }
    pub fn set_target_classes(&mut self, vehicle_types: &HashSet<String>) {
        for class in vehicle_types.iter() {
            self.statistics
                .vehicles_data
                .insert(class.clone(), VehicleTypeParameters::default());
        }
    }
    /// Returns whether this observation satisfies the zone's counting rule.
    pub fn register_or_update_object(
        &mut self,
        object_id: Uuid,
        _timestamp: f32,
        _relative_time: f32,
        _speed: f32,
        _classname: String,
        _crossed_virtual_line: bool,
        _zone_id_from: Option<String>,
    ) -> bool {
        let register_as_crossed = match &self.virtual_line {
            Some(_) => _crossed_virtual_line,
            None => false,
        };
        match self.objects_registered.entry(object_id) {
            Occupied(mut entry) => {
                // println!("Object {} is already registered in zone {}", object_id, self.id);
                entry.get_mut().classname = _classname;
                entry.get_mut().speed = _speed;
                // If object crossed virtual line then we should not reset this flag
                let was_previously_crossed = entry.get().crossed_virtual_line;
                if !was_previously_crossed {
                    entry.get_mut().crossed_virtual_line = register_as_crossed;
                }
            }
            Vacant(entry) => {
                self.current_statistics.last_time_registered = _relative_time;
                entry.insert(ObjectInfo {
                    classname: _classname,
                    speed: _speed,
                    crossed_virtual_line: register_as_crossed,
                    timestamp_registration: _timestamp,
                    last_crossed_at: -1.0,
                });
            }
        }
        if !register_as_crossed {
            return self.virtual_line.is_none();
        }

        // Bbox jitter can flip the side of the line several times per second.
        // Count a crossing again only after the track has been away long
        // enough for a real new crossing.
        let duplicate = self
            .objects_registered
            .get(&object_id)
            .map_or(false, |info| {
                let age = _relative_time - info.last_crossed_at;
                age >= 0.0 && age < VIRTUAL_LINE_CROSSING_COOLDOWN_SEC
            });
        if duplicate {
            return true;
        }
        if let Some(info) = self.objects_registered.get_mut(&object_id) {
            info.last_crossed_at = _relative_time;
        }

        // Check if this object has crossed the virtual line before
        if !self.objects_crossed.contains(&object_id) {
            // First time crossing - add to crossed objects and update OD matrix
            self.objects_crossed.insert(object_id);
            if let Some(zone_id_from) = _zone_id_from {
                *self
                    .current_statistics
                    .income
                    .entry(zone_id_from)
                    .or_insert(0) += 1;
            }
        } else {
            // Object has crossed before - this could be a U-turn
            // For now, let's count every crossing (you can add time-based logic later)
            if let Some(zone_id_from) = _zone_id_from {
                *self
                    .current_statistics
                    .income
                    .entry(zone_id_from)
                    .or_insert(0) += 1;
            }
        }
        true
    }
    pub fn reset_objects_registered(&mut self) {
        self.objects_registered.clear();
        self.objects_crossed.clear();
    }
    pub fn reset_statistics(&mut self, _period_start: DateTime<Utc>, _period_end: DateTime<Utc>) {
        self.statistics.period_start = _period_start;
        self.statistics.period_end = _period_end;
        for (_, class_stats) in self.statistics.vehicles_data.iter_mut() {
            class_stats.sum_intensity = 0;
            class_stats.defined_sum_intensity = 0;
            class_stats.avg_speed = -1.0;
        }
        self.statistics.traffic_flow_parameters = TrafficFlowParameters::default();
        // Clear real-time statistics for incoming vehicles from other zones
        self.current_statistics.income.clear();
    }
    pub fn update_statistics(&mut self, _period_start: DateTime<Utc>, _period_end: DateTime<Utc>) {
        // Save OD matrix data before resetting
        self.statistics.income = self.current_statistics.income.clone();
        self.reset_statistics(_period_start, _period_end);
        let register_via_virtual_line = self.virtual_line.is_some();
        let headway_avg = match register_via_virtual_line {
            true => {
                let mut sorted_by_time = self
                    .objects_crossed
                    .iter()
                    .filter_map(|&object_id| self.objects_registered.get(&object_id))
                    .map(|object_info| object_info.timestamp_registration)
                    .collect::<Vec<f32>>();
                // Need at least 2 timestamps to compute headway
                if sorted_by_time.len() >= 2 {
                    sorted_by_time.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let sum: f32 = sorted_by_time.windows(2).map(|w| w[1] - w[0]).sum();
                    sum / (sorted_by_time.len() - 1) as f32
                } else {
                    0.0
                }
            }
            _ => {
                // Need at least 2 objects to compute headway
                if self.objects_registered.len() >= 2 {
                    let mut sorted_by_time: Vec<f32> = self
                        .objects_registered
                        .values()
                        .map(|object_info| object_info.timestamp_registration)
                        .collect();
                    sorted_by_time.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let sum: f32 = sorted_by_time.windows(2).map(|w| w[1] - w[0]).sum();
                    sum / (sorted_by_time.len() - 1) as f32
                } else {
                    0.0
                }
            }
        };
        let mut total_sum_intensity = 0;
        let mut total_defined_sum_intensity: u32 = 0;
        for (_, object_info) in self.objects_registered.iter() {
            let classname = object_info.classname.to_owned();
            let speed = object_info.speed;
            let vehicle_type_parameters =
                match self.statistics.vehicles_data.entry(classname.clone()) {
                    Occupied(o) => o.into_mut(),
                    Vacant(v) => {
                        let new_params = v.insert(VehicleTypeParameters::default());
                        new_params
                    }
                };
            if register_via_virtual_line && !object_info.crossed_virtual_line {
                continue;
            }
            vehicle_type_parameters.sum_intensity += 1;
            total_sum_intensity += 1;
            // Ignore undefined vehicle speed (but keep it as counted in intensity parameter)
            if speed < 0.0 || !speed.is_finite() {
                continue;
            }
            vehicle_type_parameters.defined_sum_intensity += 1;
            total_defined_sum_intensity += 1;
            // Iterative average calculation
            // https://math.stackexchange.com/questions/106700/incremental-averageing
            // Start calculate average speed calculation only when there are two vehicles atleast
            if vehicle_type_parameters.defined_sum_intensity < 2 {
                vehicle_type_parameters.avg_speed = speed;
                continue;
            }
            vehicle_type_parameters.avg_speed = vehicle_type_parameters.avg_speed
                + (speed - vehicle_type_parameters.avg_speed)
                    / (vehicle_type_parameters.defined_sum_intensity as f32);
        }
        self.statistics.traffic_flow_parameters.avg_speed = if total_defined_sum_intensity > 0 {
            // Could have non-estimated speed for some vehicle classes. Therefore it is needed to filter those
            let speeds = self
                .statistics
                .vehicles_data
                .iter()
                .filter(|vt_param| vt_param.1.avg_speed > 0.0 && vt_param.1.avg_speed.is_finite())
                .map(|v| v.1.avg_speed)
                .collect::<Vec<f32>>();
            if speeds.is_empty() {
                -1.0
            } else {
                speeds.iter().sum::<f32>() / (speeds.len() as f32)
            }
        } else {
            -1.0
        };
        self.statistics.traffic_flow_parameters.sum_intensity = total_sum_intensity;
        self.statistics
            .traffic_flow_parameters
            .defined_sum_intensity = total_defined_sum_intensity;
        self.statistics.traffic_flow_parameters.avg_headway = headway_avg;
        // self.statistics.traffic_flow_parameters.avg_speed = self.statistics.vehicles_data.values().map(|vt_param| vt_param.sum_intensity).sum::<u32>();
        self.reset_objects_registered();
    }
    // Checks if given polygon contains a point.
    // Delegates to the canonical `polygon_contains_point` so that zone matching
    // and the violation rules classify a point identically.
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        polygon_contains_point(&self.pixel_coordinates, x, y)
    }
    pub fn contains_point_cv(&self, pt: &Point2f) -> bool {
        self.contains_point(pt.x, pt.y)
    }
    // Checks if an object has entered the polygon
    // Let's clarify for future questions: we are assuming the object is represented by a center, not a bounding box
    // So object has entered polygon when its center had entered polygon too
    pub fn object_entered_cv(&self, from: Point2f, to: Point2f) -> bool {
        // If P(xN-1,yN-1) is not inside of polygon and P(xN,yN) is inside of polygon then object has entered the polygon
        if !self.contains_point_cv(&from) && self.contains_point_cv(&to) {
            return true;
        }
        false
    }
    // Checks if an object has left the polygon
    // Let's clarify for future questions: we are assuming the object is represented by a center, not a bounding box
    // So object has left polygon when its center had left polygon too
    pub fn object_left_cv(&self, from: Point2f, to: Point2f) -> bool {
        // If P(xN-1,yN-1) is not inside of polygon and P(xN,yN) is inside of polygon then object has entered the polygon
        if self.contains_point_cv(&from) && !self.contains_point_cv(&to) {
            return true;
        }
        false
    }
    pub fn project_to_skeleton(&self, x: f32, y: f32) -> (f32, f32) {
        self.skeleton.project(x, y)
    }
    pub fn get_skeleton_ppm(&self) -> f32 {
        self.skeleton.pixels_per_meter
    }
    pub fn crossed_virtual_line(&self, x1: f32, y1: f32, x2: f32, y2: f32) -> bool {
        match &self.virtual_line {
            Some(vl) => {
                let is_left_before = vl.is_left(x1, y1);
                let is_left_after = vl.is_left(x2, y2);

                // A side change alone would also count a track that goes
                // around the end of the line. The movement must actually
                // cross the configured segment.
                if !is_intersects(
                    x1,
                    y1,
                    x2,
                    y2,
                    vl.line[0][0] as f32,
                    vl.line[0][1] as f32,
                    vl.line[1][0] as f32,
                    vl.line[1][1] as f32,
                ) {
                    return false;
                }

                // Check direction-specific crossing
                if vl.direction == VirtualLineDirection::Inbound {
                    // Inbound: crossing from right to left (or bottom to top)
                    if !is_left_before && is_left_after {
                        return true;
                    }
                } else {
                    // Outbound: crossing from left to right (or top to bottom)
                    if is_left_before && !is_left_after {
                        return true;
                    }
                }
                return false;
            }
            None => {
                return false;
            }
        }
    }
    pub fn get_virtual_line(&self) -> Option<VirtualLine> {
        match &self.virtual_line {
            Some(vl) => Some(vl.clone()),
            None => None,
        }
    }
    pub fn set_virtual_line(&mut self, _virtual_line: VirtualLine) {
        self.virtual_line = Some(_virtual_line);
    }
    /// Violation rules of this zone as the configuration holds them
    pub fn get_rules(&self) -> LaneRulesSettings {
        self.rules.clone()
    }
    pub fn set_rules(&mut self, _rules: LaneRulesSettings) {
        self.rules = _rules;
    }
    /// The solid line of this zone, if a rule is configured for it. The line is
    /// kept inside `rules.solid_line`, which is also what the violation engine
    /// reads, so there is a single copy of the geometry
    pub fn get_solid_line(&self) -> Option<SolidLine> {
        self.rules
            .solid_line
            .as_ref()
            .map(|rule| SolidLine::new(rule.line, rule.margin_px))
    }
    pub fn set_solid_line(&mut self, _solid_line: SolidLine) {
        self.rules.solid_line = Some(crate::settings::LaneSolidLineRuleSettings {
            line: _solid_line.line,
            margin_px: _solid_line.margin_px,
        });
    }
    pub fn clear_solid_line(&mut self) {
        self.rules.solid_line = None;
    }
    pub fn draw_geom(&self, img: &mut RawFrame) {
        let w = img.cols() as usize;
        let h = img.rows() as usize;
        let step = img.step();
        let bytes = img.data_bytes_mut();
        let bgr = scalar_to_bgr(&self.color);
        for i in 0..self.pixel_coordinates.len() {
            let j = (i + 1) % self.pixel_coordinates.len();
            draw_line_thick(
                bytes,
                step,
                w,
                h,
                self.pixel_coordinates[i].x as i32,
                self.pixel_coordinates[i].y as i32,
                self.pixel_coordinates[j].x as i32,
                self.pixel_coordinates[j].y as i32,
                bgr,
                2,
            );
        }
    }
    pub fn draw_skeleton(&self, img: &mut RawFrame) {
        self.skeleton.draw_on_frame(img);
    }
    pub fn draw_virtual_line(&self, img: &mut RawFrame) {
        match &self.virtual_line {
            Some(vl) => {
                vl.draw_on_frame(img);
            }
            None => {}
        }
    }
    pub fn draw_solid_line(&self, img: &mut RawFrame) {
        if let Some(solid_line) = self.get_solid_line() {
            solid_line.draw_on_frame(img);
        }
    }
    pub fn draw_current_intensity(&self, img: &mut RawFrame) {
        let register_via_virtual_line = self.virtual_line.is_some();
        let current_intensity = match register_via_virtual_line {
            true => self.objects_crossed.len(),
            false => self.objects_registered.len(),
        };
        let w = img.cols() as usize;
        let h = img.rows() as usize;
        let step = img.step();
        let bytes = img.data_bytes_mut();
        let black: [u8; 3] = [0, 0, 0];
        draw_text(
            bytes,
            step,
            w,
            h,
            self.pixel_coordinates[0].x as i32 + 20,
            self.pixel_coordinates[0].y as i32 - 10,
            &current_intensity.to_string(),
            black,
            1,
        );
    }
    pub fn to_geojson(&self) -> ZoneFeature {
        let mut euclidean: Vec<Vec<i32>> = Vec::new();
        for pt in self.pixel_coordinates.iter() {
            euclidean.push(vec![pt.x as i32, pt.y as i32]);
        }
        let mut geojson_poly = vec![];
        let mut poly_element = vec![];
        for v in self.spatial_coordinates_epsg4326.iter() {
            poly_element.push(vec![v.x, v.y]);
        }
        if let Some(first) = self.spatial_coordinates_epsg4326.first() {
            poly_element.push(vec![first.x, first.y]);
        }
        if !poly_element.is_empty() {
            geojson_poly.push(poly_element);
        }
        ZoneFeature {
            typ: "Feature".to_string(),
            id: self.id.clone(),
            properties: ZonePropertiesGeoJSON {
                road_lane_num: self.road_lane_num,
                road_lane_direction: self.road_lane_direction,
                coordinates: euclidean,
                color_rgb: [
                    self.color[2] as i16,
                    self.color[1] as i16,
                    self.color[0] as i16,
                ],
                virtual_line: match &self.virtual_line {
                    Some(vl) => Some(VirtualLineFeature {
                        geometry: vl.line,
                        color_rgb: vl.color,
                        direction: vl.direction.to_string(),
                    }),
                    None => None,
                },
                solid_line: self.rules.solid_line.as_ref().map(|rule| SolidLineFeature {
                    geometry: rule.line,
                    color_rgb: SOLID_LINE_COLOR_RGB,
                    margin_px: rule.margin_px,
                }),
            },
            geometry: GeoPolygon {
                geometry_type: "Polygon".to_string(),
                coordinates: geojson_poly,
            },
        }
    }
}

fn make_perspective_transform(
    pixel_coords: &[Point2f],
    epsg3857_coords: &[Point2f],
) -> Option<PerspectiveTransform> {
    if pixel_coords.len() < 4 || epsg3857_coords.len() < 4 {
        return None;
    }
    let src: [(f32, f32); 4] = core::array::from_fn(|i| (pixel_coords[i].x, pixel_coords[i].y));
    let dst: [(f32, f32); 4] =
        core::array::from_fn(|i| (epsg3857_coords[i].x, epsg3857_coords[i].y));
    PerspectiveTransform::new(&src, &dst)
}

fn find_skeleton_line(
    coordinates: &Vec<Point2f>,
    first_line_idx: usize,
    second_line_id: usize,
) -> [Point2f; 2] {
    if coordinates.len() <= first_line_idx + 1 || coordinates.len() <= second_line_id + 1 {
        // Invalid geometry is rejected by the settings validation; this guard
        // keeps a manually constructed Zone from panicking.
        let fallback = coordinates
            .first()
            .copied()
            .unwrap_or_else(|| Point2f::new(0.0, 0.0));
        return [fallback, fallback];
    }
    let a = coordinates[first_line_idx];
    let b = coordinates[first_line_idx + 1];
    let a_b_center = Point2f::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);

    let c = coordinates[second_line_id];
    let d = coordinates[second_line_id + 1];
    let c_d_center = Point2f::new((c.x + d.x) / 2.0, (c.y + d.y) / 2.0);

    [a_b_center, c_d_center]
}

#[cfg(test)]
mod tests {
    use crate::lib::zones::zones::*;
    #[test]
    fn test_contains_point() {
        let convex_polygons = vec![
            Zone::default_from_cv(vec![
                Point2f::new(0.0, 0.0),
                Point2f::new(5.0, 0.0),
                Point2f::new(5.0, 5.0),
                Point2f::new(0.0, 5.0),
            ]),
            Zone::default_from_cv(vec![
                Point2f::new(0.0, 0.0),
                Point2f::new(5.0, 0.0),
                Point2f::new(5.0, 5.0),
                Point2f::new(0.0, 5.0),
            ]),
            Zone::default_from_cv(vec![
                Point2f::new(0.0, 0.0),
                Point2f::new(5.0, 0.0),
                Point2f::new(5.0, 5.0),
                Point2f::new(0.0, 5.0),
            ]),
        ];
        let points = vec![
            Point2f::new(20.0, 20.0),
            Point2f::new(4.0, 4.0),
            Point2f::new(-2.0, 12.0),
        ];
        let correct_answers = vec![false, true, false];
        for (i, convex_polygon) in convex_polygons.iter().enumerate() {
            let answer = convex_polygon.contains_point(points[i].x, points[i].y);
            assert_eq!(answer, correct_answers[i]);
        }
    }
    #[test]
    fn test_object_entered_cv() {
        let polygon = Zone::default_from_cv(vec![
            Point2f::new(23.0, 15.0),
            Point2f::new(67.0, 15.0),
            Point2f::new(67.0, 41.0),
            Point2f::new(23.0, 41.0),
        ]);

        let a_track_must_enter = vec![Point2f::new(34.0, 13.0), Point2f::new(36.0, 21.0)];
        let entered = polygon.object_entered_cv(a_track_must_enter[0], a_track_must_enter[1]);
        assert_eq!(entered, true);

        let b_track_must_not_enter = vec![Point2f::new(46.0, 38.0), Point2f::new(49.0, 46.0)];
        let entered =
            polygon.object_entered_cv(b_track_must_not_enter[0], b_track_must_not_enter[1]);
        assert_eq!(entered, false);

        let c_track_must_not_enter = vec![Point2f::new(55.0, 23.0), Point2f::new(55.0, 29.0)];
        let entered =
            polygon.object_entered_cv(c_track_must_not_enter[0], c_track_must_not_enter[1]);
        assert_eq!(entered, false);

        let d_track_must_not_enter = vec![Point2f::new(19.0, 20.0), Point2f::new(19.0, 25.0)];
        let entered =
            polygon.object_entered_cv(d_track_must_not_enter[0], d_track_must_not_enter[1]);
        assert_eq!(entered, false);
    }
    #[test]
    fn test_object_left_cv() {
        let polygon = Zone::default_from_cv(vec![
            Point2f::new(23.0, 15.0),
            Point2f::new(67.0, 15.0),
            Point2f::new(67.0, 41.0),
            Point2f::new(23.0, 41.0),
        ]);

        let a_track_must_enter = vec![Point2f::new(34.0, 13.0), Point2f::new(36.0, 21.0)];
        let left = polygon.object_left_cv(a_track_must_enter[0], a_track_must_enter[1]);
        assert_eq!(left, false);

        let b_track_must_not_enter = vec![Point2f::new(46.0, 38.0), Point2f::new(49.0, 46.0)];
        let left = polygon.object_left_cv(b_track_must_not_enter[0], b_track_must_not_enter[1]);
        assert_eq!(left, true);

        let c_track_must_not_enter = vec![Point2f::new(55.0, 23.0), Point2f::new(55.0, 29.0)];
        let left = polygon.object_left_cv(c_track_must_not_enter[0], c_track_must_not_enter[1]);
        assert_eq!(left, false);

        let d_track_must_not_enter = vec![Point2f::new(19.0, 20.0), Point2f::new(19.0, 25.0)];
        let left = polygon.object_left_cv(d_track_must_not_enter[0], d_track_must_not_enter[1]);
        assert_eq!(left, false);
    }

    #[test]
    fn test_solid_line_is_in_geojson() {
        use crate::lib::cv::Scalar;
        use crate::lib::spatial::Point2f;
        use crate::lib::zones::SolidLine;

        let mut zone = Zone::new(
            "dir_0_lane_0".to_string(),
            vec![
                Point2f::new(0.0, 0.0),
                Point2f::new(10.0, 0.0),
                Point2f::new(10.0, 10.0),
                Point2f::new(0.0, 10.0),
            ],
            vec![
                Point2f::new(1.0, 2.0),
                Point2f::new(1.1, 2.0),
                Point2f::new(1.1, 2.1),
                Point2f::new(1.0, 2.1),
            ],
            vec![
                Point2f::new(1.0, 2.0),
                Point2f::new(1.1, 2.0),
                Point2f::new(1.1, 2.1),
                Point2f::new(1.0, 2.1),
            ],
            Scalar::from((255.0, 255.0, 255.0)),
            0,
            0,
            None,
        );
        zone.set_solid_line(SolidLine::new([[10.0, 20.0], [30.0, 40.0]], 6.0));

        let feature = zone.to_geojson();
        let solid_line = feature
            .properties
            .solid_line
            .expect("solid line must be exposed through GeoJSON");
        assert_eq!(solid_line.geometry, [[10.0, 20.0], [30.0, 40.0]]);
        assert_eq!(solid_line.margin_px, 6.0);
        assert_eq!(solid_line.color_rgb, [255, 255, 0]);
    }

    #[test]
    fn crossed_virtual_line_respects_direction_and_finite_segment() {
        use crate::lib::zones::{VirtualLine, VirtualLineDirection};

        let zone_with_line = |direction| {
            let mut zone = Zone::default_from_cv(vec![
                Point2f::new(0.0, 0.0),
                Point2f::new(200.0, 0.0),
                Point2f::new(200.0, 200.0),
                Point2f::new(0.0, 200.0),
            ]);
            zone.set_virtual_line(VirtualLine::new_from([[100, 0], [100, 100]], direction));
            zone
        };

        let inbound = zone_with_line(VirtualLineDirection::Inbound);
        // Right to left crosses the finite segment.
        assert!(inbound.crossed_virtual_line(150.0, 50.0, 50.0, 50.0));
        // Left to right is the opposite direction.
        assert!(!inbound.crossed_virtual_line(50.0, 50.0, 150.0, 50.0));
        // Both points on the same side.
        assert!(!inbound.crossed_virtual_line(150.0, 40.0, 150.0, 60.0));
        // The infinite line is crossed, but below the end of the segment.
        assert!(!inbound.crossed_virtual_line(150.0, 150.0, 50.0, 150.0));

        let outbound = zone_with_line(VirtualLineDirection::Outbound);
        assert!(outbound.crossed_virtual_line(50.0, 50.0, 150.0, 50.0));
        assert!(!outbound.crossed_virtual_line(150.0, 50.0, 50.0, 50.0));
    }

    #[test]
    fn virtual_line_crossings_are_debounced() {
        use crate::lib::zones::{VirtualLine, VirtualLineDirection};
        use uuid::Uuid;

        let mut zone = Zone::default_from_cv(vec![
            Point2f::new(0.0, 0.0),
            Point2f::new(200.0, 0.0),
            Point2f::new(200.0, 200.0),
            Point2f::new(0.0, 200.0),
        ]);
        zone.set_virtual_line(VirtualLine::new_from(
            [[100, 0], [100, 100]],
            VirtualLineDirection::Inbound,
        ));
        let track = Uuid::new_v4();

        assert!(zone.register_or_update_object(
            track,
            0.0,
            1.0,
            10.0,
            "car".to_string(),
            true,
            Some("from".to_string()),
        ));
        assert_eq!(zone.current_statistics.income.get("from"), Some(&1));

        // A second crossing 0.1 s later is bbox jitter, not a new event.
        assert!(zone.register_or_update_object(
            track,
            0.0,
            1.1,
            10.0,
            "car".to_string(),
            true,
            Some("from".to_string()),
        ));
        assert_eq!(zone.current_statistics.income.get("from"), Some(&1));

        // After the cooldown a real crossing is counted again.
        assert!(zone.register_or_update_object(
            track,
            0.0,
            1.7,
            10.0,
            "car".to_string(),
            true,
            Some("from".to_string()),
        ));
        assert_eq!(zone.current_statistics.income.get("from"), Some(&2));
    }
}
