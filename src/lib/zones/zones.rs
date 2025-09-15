// Export submodules
pub(crate) mod geojson;
pub(crate) mod geometry;

use chrono::{DateTime, Utc};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use geometry::PointsOrientation;
use geometry::{get_orientation, is_intersects, is_on_segment};

use geojson::{GeoPolygon, VirtualLineFeature, ZoneFeature, ZonePropertiesGeoJSON};

use crate::{lib::{spatial::compute_center}};
use crate::lib::spatial::epsg::lonlat_to_meters;
use crate::lib::spatial::haversine;
use crate::lib::spatial::SpatialConverter;
use crate::lib::zones::{
    Skeleton, Statistics, VehicleTypeParameters, TrafficFlowParameters, VirtualLine, VirtualLineDirection,
};
use opencv::{
    core::Mat, core::Point2f, core::Point2i, core::Scalar, imgproc::line, imgproc::put_text,
    imgproc::FONT_HERSHEY_SIMPLEX, imgproc::LINE_8,
};

#[derive(Debug, Clone)]
struct ObjectInfo {
    classname: String,
    speed: f32,
    crossed_virtual_line: bool,
    timestamp_registration: f32
}

type Registered = HashMap<Uuid, ObjectInfo>;

#[derive(Debug)]
pub struct Zone {
    pub id: String,
    pixel_coordinates: Vec<Point2f>,
    spatial_coordinates_epsg4326: Vec<Point2f>,
    spatial_coordinates_epsg3857: Vec<Point2f>,
    pub color: Scalar,
    pub road_lane_num: u16,
    pub road_lane_direction: u8,
    spatial_converter: SpatialConverter,
    pub statistics: Statistics,
    objects_registered: Registered,
    objects_crossed: HashSet<Uuid>,
    pub current_statistics: RealTimeStatistics,
    skeleton: Skeleton,
    virtual_line: Option<VirtualLine>,
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
            spatial_converter: SpatialConverter::default(),
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
        let converter = if spatial_coordinates_epsg4326.len() > 0 {
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
            skeleton.pixels_per_meter = skeleton.length_pixels / skeleton.length_meters;
            SpatialConverter::new_from(coordinates.clone(), spatial_coordinates_epsg3857.clone())
        } else {
            SpatialConverter::default()
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
        skeleton.pixels_per_meter = skeleton.length_pixels / skeleton.length_meters;
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
        self.spatial_converter = SpatialConverter::new_from(
            self.pixel_coordinates.clone(),
            self.spatial_coordinates_epsg3857.clone(),
        );
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
        self.spatial_converter = SpatialConverter::new_from(
            self.pixel_coordinates.clone(),
            self.spatial_coordinates_epsg3857.clone(),
        );
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
    pub fn register_or_update_object(
        &mut self,
        object_id: Uuid,
        _timestamp: f32,
        _relative_time: f32,
        _speed: f32,
        _classname: String,
        _crossed_virtual_line: bool,
        _zone_id_from: Option<String>,
    ) {
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
                    timestamp_registration: _timestamp
                });
            }
        }
        if !register_as_crossed {
            return
        }
        // Check if this object has crossed the virtual line before
        if !self.objects_crossed.contains(&object_id) {
            // First time crossing - add to crossed objects and update OD matrix
            self.objects_crossed.insert(object_id);
            if let Some(zone_id_from) = _zone_id_from {
                *self.current_statistics.income
                    .entry(zone_id_from)
                    .or_insert(0) += 1;
            }
        } else {
            // Object has crossed before - this could be a U-turn
            // For now, let's count every crossing (you can add time-based logic later)
            if let Some(zone_id_from) = _zone_id_from {
                *self.current_statistics.income
                    .entry(zone_id_from)
                    .or_insert(0) += 1;
            }
        }
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
            class_stats.avg_speed = -1.0;
        }
        self.statistics.traffic_flow_parameters = TrafficFlowParameters::default();
        // Clear real-time statistics for incoming vehicles from other zones
        self.current_statistics.income.clear();
    }
    pub fn update_statistics(&mut self, _period_start: DateTime<Utc>, _period_end: DateTime<Utc>) {
        self.reset_statistics(_period_start, _period_end);
        let register_via_virtual_line = self.virtual_line.is_some();
        // Are there better ways to sort hashmap (or btreemap) and extract just timestamps? 
        let headway_avg = if self.objects_registered.len() > 1 { // For headway calculation two vehicles are needed at least
            let mut sorted_by_time = self.objects_registered.values().map(|object_info| object_info.timestamp_registration).collect::<Vec<f32>>();
            sorted_by_time.sort_by(|a, b| a.partial_cmp(b).unwrap());
            sorted_by_time.windows(2).map(|w| w[1] - w[0]).sum::<f32>() / (sorted_by_time.len() as f32 - 1.0)
        } else {
            0.0
        };
        let mut total_avg_speed = 0.0;
        let mut total_sum_intensity = 0;
        let mut total_defined_sum_intensity: u32 = 0;
        for (_, object_info) in self.objects_registered.iter() {
            let classname = object_info.classname.to_owned();
            let speed = object_info.speed;
            let vehicle_type_parameters = match self.statistics.vehicles_data.entry(classname.clone()) {
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
            if speed < 0.0 {
                continue
            }
            vehicle_type_parameters.defined_sum_intensity += 1;
            total_defined_sum_intensity += 1;
            // Iterative average calculation
            // https://math.stackexchange.com/questions/106700/incremental-averageing
            // Start calculate average speed calculation only when there are two vehicles atleast
            if total_defined_sum_intensity < 2 {
                vehicle_type_parameters.avg_speed = speed;
                total_avg_speed = speed;
                continue;
            }
            vehicle_type_parameters.avg_speed = vehicle_type_parameters.avg_speed + (speed - vehicle_type_parameters.avg_speed) / (vehicle_type_parameters.defined_sum_intensity as f32);
            total_avg_speed = total_avg_speed + (speed - total_avg_speed) / (total_defined_sum_intensity as f32);
        }
        self.statistics.traffic_flow_parameters.avg_speed = if total_sum_intensity > 0 {
            // Could have non-estimated speed for some vehicle classes. Therefore it is needed to filter those
            let speeds = self.statistics.vehicles_data.iter().filter(|vt_param| vt_param.1.avg_speed > 0.0).map(|v| v.1.avg_speed).collect::<Vec<f32>>();
            if speeds.is_empty() {
                -1.0
            } else {
                speeds.iter().sum::<f32>() / (speeds.len() as f32)
            }
        } else {
            -1.0
        };
        self.statistics.traffic_flow_parameters.sum_intensity = total_sum_intensity;
        self.statistics.traffic_flow_parameters.defined_sum_intensity = total_defined_sum_intensity;
        self.statistics.traffic_flow_parameters.avg_headway = headway_avg;
        // self.statistics.traffic_flow_parameters.avg_speed = self.statistics.vehicles_data.values().map(|vt_param| vt_param.sum_intensity).sum::<u32>();
        self.reset_objects_registered();
    }
    // Checks if given polygon contains a point
    // Code has been taken from: https://github.com/LdDl/odam/blob/master/virtual_polygons.go#L180
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        let n = self.pixel_coordinates.len();
        // @todo: math.maxInt could lead to overflow obviously. Need good workaround. PRs are welcome
        let extreme_point = vec![99999.0, y as f32];
        let mut intersections_cnt = 0;
        let mut previous = 0;
        loop {
            let current = (previous + 1) % n;
            // Check if the segment from given point P to extreme point intersects with the segment from polygon point on previous interation to  polygon point on current interation
            if is_intersects(
                self.pixel_coordinates[previous].x as f32,
                self.pixel_coordinates[previous].y as f32,
                self.pixel_coordinates[current].x as f32,
                self.pixel_coordinates[current].y as f32,
                x,
                y,
                extreme_point[0],
                extreme_point[1],
            ) {
                let orientation = get_orientation(
                    self.pixel_coordinates[previous].x as f32,
                    self.pixel_coordinates[previous].y as f32,
                    x,
                    y,
                    self.pixel_coordinates[current].x as f32,
                    self.pixel_coordinates[current].y as f32,
                );
                // If given point P is collinear with segment from polygon point on previous interation to  polygon point on current interation
                if orientation == PointsOrientation::Collinear {
                    // then check if it is on segment
                    // 'True' will be returns if it lies on segment. Otherwise 'False' will be returned
                    return is_on_segment(
                        self.pixel_coordinates[previous].x as f32,
                        self.pixel_coordinates[previous].y as f32,
                        x,
                        y,
                        self.pixel_coordinates[current].x as f32,
                        self.pixel_coordinates[current].y as f32,
                    );
                }
                intersections_cnt += 1;
            }
            previous = current;
            if previous == 0 {
                break;
            }
        }
        // If ray intersects even number of times then return true
        // Otherwise return false
        if intersections_cnt % 2 == 1 {
            return true;
        }
        false
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
                if is_left_before && !is_left_after {
                    return true;
                }
                if !is_left_before && is_left_after {
                    return true;
                }
                // if vl.direction == VirtualLineDirection::LeftToRightTopToBottom {
                //     if is_left_before && !is_left_after {
                //         return true;
                //     }
                // } else {
                //     if !is_left_before && is_left_after {
                //         return true;
                //     }
                // }
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
    pub fn draw_geom(&self, img: &mut Mat) {
        // @todo: proper error handling
        for i in 1..self.pixel_coordinates.len() {
            let prev_pt = Point2i::new(
                self.pixel_coordinates[i - 1].x as i32,
                self.pixel_coordinates[i - 1].y as i32,
            );
            let current_pt = Point2i::new(
                self.pixel_coordinates[i].x as i32,
                self.pixel_coordinates[i].y as i32,
            );
            match line(img, prev_pt, current_pt, self.color, 2, LINE_8, 0) {
                Ok(_) => {}
                Err(err) => {
                    panic!("Can't draw line for polygon due the error: {:?}", err)
                }
            };
        }
        let last_pt = Point2i::new(
            self.pixel_coordinates[self.pixel_coordinates.len() - 1].x as i32,
            self.pixel_coordinates[self.pixel_coordinates.len() - 1].y as i32,
        );
        let first_pt = Point2i::new(
            self.pixel_coordinates[0].x as i32,
            self.pixel_coordinates[0].y as i32,
        );
        match line(img, last_pt, first_pt, self.color, 2, LINE_8, 0) {
            Ok(_) => {}
            Err(err) => {
                panic!("Can't draw line for polygon due the error: {:?}", err)
            }
        };
    }
    pub fn draw_skeleton(&self, img: &mut Mat) {
        self.skeleton.draw_on_mat(img);
    }
    pub fn draw_virtual_line(&self, img: &mut Mat) {
        match &self.virtual_line {
            Some(vl) => {
                vl.draw_on_mat(img);
            }
            None => {}
        }
    }
    pub fn draw_current_intensity(&self, img: &mut Mat) {
        let register_via_virtual_line = match &self.virtual_line {
            Some(_) => true,
            None => false,
        };
        let current_intensity = match register_via_virtual_line {
            true => self.objects_crossed.len(),
            false => self.objects_registered.len(),
        };
        let anchor = Point2i::new(
            self.pixel_coordinates[0].x as i32 + 20,
            self.pixel_coordinates[0].y as i32 - 10,
        );
        match put_text(
            img,
            &current_intensity.to_string(),
            anchor,
            FONT_HERSHEY_SIMPLEX,
            0.5,
            Scalar::from((0.0, 0.0, 0.0)),
            2,
            LINE_8,
            false,
        ) {
            Ok(_) => {}
            Err(err) => {
                println!("Can't display velocity of object due the error {:?}", err);
            }
        };
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
        poly_element.push(vec![
            self.spatial_coordinates_epsg4326[0].x,
            self.spatial_coordinates_epsg4326[0].y,
        ]);
        geojson_poly.push(poly_element);
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
            },
            geometry: GeoPolygon {
                geometry_type: "Polygon".to_string(),
                coordinates: geojson_poly,
            },
        }
    }
}

fn find_skeleton_line(
    coordinates: &Vec<Point2f>,
    first_line_idx: usize,
    second_line_id: usize,
) -> [Point2f; 2] {
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
    use super::*;
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
}
