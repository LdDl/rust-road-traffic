// Export submodules
pub(crate) mod geometry;
pub(crate) mod geojson;

use std::collections::{HashMap};
use std::collections::hash_map::Entry::{
    Occupied,
    Vacant
};
use uuid::Uuid;
use chrono::{DateTime, TimeZone, Utc};

use geometry::PointsOrientation;
use geometry::{
    is_intersects,
    get_orientation,
    is_on_segment
};

use geojson::{
    ZoneFeature,
    ZonePropertiesGeoJSON,
    GeoPolygon
};

use opencv::{
    core::Point2i,
    core::Point2f,
    core::Scalar,
    core::Mat,
    imgproc::LINE_8,
    imgproc::line,
    imgproc::FONT_HERSHEY_SIMPLEX,
    imgproc::put_text,
};
use crate::lib::spatial::SpatialConverter;
use crate::lib::spatial::epsg::lonlat_to_meters;
use crate::lib::spatial::compute_center;
use crate::lib::spatial::haversine;

#[derive(Debug)]
pub struct Statistics {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub vehicles_data: HashMap<String, VehicleTypeParameters>
}

impl Statistics {
    pub fn default() -> Self {
        Statistics{
            period_start: TimeZone::with_ymd_and_hms(&Utc, 1970, 1, 1, 0, 0, 0).unwrap(),
            period_end: TimeZone::with_ymd_and_hms(&Utc, 1970, 1, 1, 0, 0, 0).unwrap(),
            vehicles_data: HashMap::new()
        }
    }
}

#[derive(Debug)]
pub struct VehicleTypeParameters {
    pub avg_speed: f32,
    pub sum_intensity: u32
}

impl VehicleTypeParameters {
    pub fn default() -> Self {
        VehicleTypeParameters{
            avg_speed: -1.0,
            sum_intensity: 0,
        }
    }
}

#[derive(Debug)]
struct ObjectInfo {
    classname: String,
    speed: f32,
}

type Registered = HashMap<Uuid, ObjectInfo>;

#[derive(Debug)]
struct Skeleton {
    line: [Point2f; 2],
    color: Scalar,
    length_pixels: f32,
    length_meters: f32,
    pixels_per_meter: f32,
}

impl Skeleton {
    fn new(a: Point2f, b: Point2f) -> Self {
        let length_pixels = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
        Skeleton {
            line: [a, b],
            color: Scalar::from((0.0, 0.0, 0.0)),
            length_pixels: length_pixels,
            length_meters: -1.0,
            pixels_per_meter: -1.0,
        }
    }
    fn default() -> Self {
        Skeleton {
            line: [Point2f::default(), Point2f::default()],
            color: Scalar::from((0.0, 0.0, 0.0)),
            length_pixels: -1.0,
            length_meters: -1.0,
            pixels_per_meter: -1.0,
        }
    }
    pub fn project(&self, x: f32, y: f32) -> (f32, f32) {
        let a = self.line[0];
        let b = self.line[1];
        let (x1, y1) = (a.x, a.y);
        let (x2, y2) = (b.x, b.y);
        let (xP, yP) = (x, y);

        // Calculate vector components of AB
        let ABx = x2 - x1;
        let ABy = y2 - y1;

        // Calculate vector components of AP
        let APx = xP - x1;
        let APy = yP - y1;

        // Calculate the dot product of AB and AP
        let dot_product = APx * ABx + APy * ABy;

        // Calculate the magnitude of AB squared
        let AB_squared = ABx.powi(2) + ABy.powi(2);

        // Calculate the scalar projection of P onto AB
        let scalar_projection = dot_product / AB_squared;
        
        if scalar_projection < 0.0 {
            // P is closest to point A, so use A as the projection point
            (a.x, a.y)
        } else if scalar_projection > 1.0 {
            // P is closest to point B, so use B as the projection point
            (b.x, b.y)
        } else {
            // Calculate the coordinates of the projected point P' on AB
            let xP_prime = x1 + scalar_projection * ABx;
            let yP_prime = y1 + scalar_projection * ABy;
            (xP_prime, yP_prime)
        }
    }
}

#[derive(Debug)]
pub struct VirtualLine {
    line: [Point2f; 2],
    color: Scalar,
}

impl VirtualLine {
    pub fn new(a: Point2f, b: Point2f) -> Self {
        VirtualLine {
            line: [a, b],
            color: Scalar::from((0.0, 0.0, 0.0)),
        }
    }
    pub fn default() -> Self {
        VirtualLine {
            line: [Point2f::default(), Point2f::default()],
            color: Scalar::from((0.0, 0.0, 0.0)),
        }
    }
    // is_left returns true if the given point is to the left side of the vertical AB or if the given point is above of the horizontal AB
    pub fn is_left(&self, cx: f32, cy: f32) -> bool {
        let a = self.line[0];
        let b = self.line[1];
        (b.x - a.x)*(cy - a.y) - (b.y - a.y)*(cx - a.x) > 0.0
    }
}

#[derive(Debug)]
pub struct Zone {
    pub id: String,
    pixel_coordinates: Vec<Point2f>,
    spatial_coordinates_epsg4326:  Vec<Point2f>,
    spatial_coordinates_epsg3857:  Vec<Point2f>,
    pub color: Scalar,
    pub road_lane_num: u16,
    pub road_lane_direction: u8,
    spatial_converter: SpatialConverter,
    pub statistics: Statistics,
    objects_registered: Registered,
    pub current_statistics: RealTimeStatistics,
    skeleton: Skeleton,
    virtual_line: Option<VirtualLine>
}


#[derive(Debug)]
pub struct RealTimeStatistics {
    pub last_time: u64,
    pub occupancy: u16,
}

impl Zone {
    pub fn default() -> Self {
        Zone{
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
            current_statistics: RealTimeStatistics{
                last_time: 0,
                occupancy: 0
            },
            skeleton: Skeleton::default(),
            virtual_line: None,
        }
    }
    pub fn new(id: String, coordinates: Vec<Point2f>, spatial_coordinates_epsg4326: Vec<Point2f>, spatial_coordinates_epsg3857: Vec<Point2f>, color: Scalar, road_lane_num: u16, road_lane_direction: u8, _virtual_line: Option<VirtualLine>) -> Self {
        let converter = SpatialConverter::new_from(coordinates.clone(), spatial_coordinates_epsg3857.clone());
        /* Eval distance between sides */
        let a = spatial_coordinates_epsg4326[0];
        let b = spatial_coordinates_epsg4326[1];
        let c = spatial_coordinates_epsg4326[2];
        let d = spatial_coordinates_epsg4326[3];
        let ab_center = compute_center(a.x, a.y, b.x, b.y);
        let cd_center = compute_center(c.x, c.y, d.x, d.y);
        let length_meters = haversine(ab_center.0, ab_center.1, cd_center.0, cd_center.1) * 1000.0;
  
        /* Init skeleton */
        let skeleton_line = find_skeleton_line(&coordinates, 0, 2); // 0-1 is first segment of polygon, 2-3 is second segment
        let mut skeleton = Skeleton::new(skeleton_line[0], skeleton_line[1]);
        skeleton.length_meters = length_meters;
        skeleton.pixels_per_meter = skeleton.length_pixels / skeleton.length_meters;
        Zone{
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
            current_statistics: RealTimeStatistics{
                last_time: 0,
                occupancy: 0
            },
            skeleton: skeleton,
            virtual_line: _virtual_line
        }
    }
    pub fn new_from_cv_with_id(points: Vec<Point2f>, id: String, _virtual_line: Option<VirtualLine>) -> Self {
        let skeleton_line = find_skeleton_line(&points, 0, 2); // 0-1 is first segment of polygon, 2-3 is second segment
        return Zone{
            id: id,
            pixel_coordinates: points,
            spatial_coordinates_epsg4326: vec![],
            spatial_coordinates_epsg3857: vec![],
            color: Scalar::from((255.0, 255.0, 255.0)),
            road_lane_num: 0,
            road_lane_direction: 0,
            spatial_converter: SpatialConverter::default(),
            statistics: Statistics::default(),
            objects_registered: HashMap::new(),
            current_statistics: RealTimeStatistics{
                last_time: 0,
                occupancy: 0
            },
            skeleton: Skeleton::new(skeleton_line[0], skeleton_line[1]),
            virtual_line: _virtual_line
        }
    }
    pub fn default_from_cv(points: Vec<Point2f>) -> Self{
        Zone::new_from_cv_with_id(points,"dir_0_lane_0".to_owned(), None)
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
        self.color = Scalar::from((rgb[2] as f64, rgb[1] as f64, rgb[0] as f64))
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
            self.spatial_coordinates_epsg4326 = self.pixel_coordinates.iter().map(|pt| Point2f::new(pt.x as f32, pt.y as f32)).collect();
            self.spatial_coordinates_epsg3857 = self.spatial_coordinates_epsg4326.iter().map(
                |pt| {
                let lonlat = lonlat_to_meters(pt.x, pt.y);
                Point2f::new(lonlat.0, lonlat.1)
            }).collect();
        }
        self.spatial_converter = SpatialConverter::new_from(self.pixel_coordinates.clone(), self.spatial_coordinates_epsg3857.clone());
        self.update_skeleton();
    }
    pub fn update_spatial_map_cv(&mut self, spatial_dest_points: Vec<Point2f>) {
        self.spatial_coordinates_epsg4326 = spatial_dest_points;
        self.spatial_coordinates_epsg3857 = self.spatial_coordinates_epsg4326.iter().map(
            |pt| {
            let lonlat = lonlat_to_meters(pt.x, pt.y);
            Point2f::new(lonlat.0, lonlat.1)
        }).collect();
        if self.pixel_coordinates.len() == 0 {
            self.pixel_coordinates = self.spatial_coordinates_epsg3857.iter().map(|pt| Point2f::new(pt.x as f32, pt.y as f32)).collect();
        }
        self.spatial_converter = SpatialConverter::new_from(self.pixel_coordinates.clone(), self.spatial_coordinates_epsg3857.clone());
        self.update_skeleton();
    }
    pub fn update_pixel_map(&mut self, pixel_src_points: [[u16; 2]; 4]) {
        let val = pixel_src_points.iter()
            .map(|pt| Point2f::new(pt[0] as f32, pt[1] as f32))
            .collect();
        self.update_pixel_map_cv(val);
    }
    pub fn update_spatial_map(&mut self, spatial_dest_points: [[f32; 2]; 4]) {
        let val = spatial_dest_points.iter()
            .map(|pt| Point2f::new(pt[0], pt[1]))
            .collect();
        self.update_spatial_map_cv(val);
    }
    pub fn set_target_classes(&mut self, vehicle_types: &'static [&'static str]) {
        for class in vehicle_types.iter() {
            self.statistics.vehicles_data.insert(class.to_string(), VehicleTypeParameters::default());
        }
    }
    pub fn register_or_update_object(&mut self, object_id: Uuid, _speed: f32, _classname: String) {
        match self.objects_registered.entry(object_id) {
            Occupied(mut entry) => {
                entry.get_mut().classname = _classname;
                entry.get_mut().speed = _speed;
            },
            Vacant(entry) => {
                entry.insert(ObjectInfo{classname: _classname, speed: _speed});
            },
        }
    }
    pub fn reset_objects_registered(&mut self) {
        self.objects_registered.clear();
    }
    pub fn reset_statistics(&mut self, _period_start: DateTime<Utc>, _period_end: DateTime<Utc>) {
        self.statistics.period_start = _period_start;
        self.statistics.period_end = _period_end;
        for (_, class_stats) in self.statistics.vehicles_data.iter_mut() {
            class_stats.sum_intensity = 0;
            class_stats.avg_speed = -1.0;
        }
    }
    pub fn update_statistics(&mut self, _period_start: DateTime<Utc>, _period_end: DateTime<Utc>) {
        self.reset_statistics(_period_start, _period_end);
        for (_, object_info) in self.objects_registered.iter() {
            let classname = object_info.classname.to_owned();
            let speed = object_info.speed;
            let mut vehicle_type_parameters = match self.statistics.vehicles_data.entry(classname) {
                Occupied(o) => o.into_mut(),
                Vacant(v) => {
                    v.insert(VehicleTypeParameters{sum_intensity: 1, avg_speed: speed});
                    continue;
                },
            };
            vehicle_type_parameters.sum_intensity += 1;
            // Iterative average calculation
            // https://math.stackexchange.com/questions/106700/incremental-averageing
            vehicle_type_parameters.avg_speed = vehicle_type_parameters.avg_speed * ((vehicle_type_parameters.sum_intensity - 1) as f32 / vehicle_type_parameters.sum_intensity as f32) + speed / vehicle_type_parameters.sum_intensity as f32;
        }
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
                self.pixel_coordinates[previous].x as f32, self.pixel_coordinates[previous].y as f32,
                self.pixel_coordinates[current].x as f32, self.pixel_coordinates[current].y as f32,
                x, y,
                extreme_point[0], extreme_point[1]
            ) 
            {
                let orientation = get_orientation(
                    self.pixel_coordinates[previous].x as f32, self.pixel_coordinates[previous].y as f32,
                    x, y,
                    self.pixel_coordinates[current].x as f32, self.pixel_coordinates[current].y as f32
                );
                // If given point P is collinear with segment from polygon point on previous interation to  polygon point on current interation
                if orientation == PointsOrientation::Collinear {
                    // then check if it is on segment
				    // 'True' will be returns if it lies on segment. Otherwise 'False' will be returned
                    return is_on_segment(
                        self.pixel_coordinates[previous].x as f32, self.pixel_coordinates[previous].y as f32,
                        x, y,
                        self.pixel_coordinates[current].x as f32, self.pixel_coordinates[current].y as f32
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
        if intersections_cnt%2 == 1 {
            return true
        }
        false
    }
    pub fn contains_point_cv(&self, pt: &Point2f) -> bool {
        self.contains_point(pt.x, pt.y)
    }
    pub fn transform_to_epsg_cv(&self, pt: &Point2f) -> Point2f {
        self.spatial_converter.transform_to_epsg_cv(pt)
    }
    pub fn transform_to_epsg(&self, x: f32, y: f32) -> (f32, f32) {
        self.spatial_converter.transform_to_epsg(x, y)
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
    pub fn scale_geom(&mut self, scale_factor_x: f32, scale_factor_y: f32) {
        for pair in self.pixel_coordinates.iter_mut() {
            pair.x = (pair.x * scale_factor_x).floor();
            pair.y = (pair.y * scale_factor_y).floor();
        }
    }
    pub fn project_to_skeleton(&self, x: f32, y: f32) -> (f32, f32) {
        self.skeleton.project(x, y)
    }
    pub fn get_skeleton_ppm(&self) -> f32 {
        self.skeleton.pixels_per_meter
    }
    pub fn project_to_skeleton_cv(&self, pt: &Point2f) -> Point2f {
        let pt = self.project_to_skeleton(pt.x , pt.y);
        Point2f::new(pt.0, pt.1)
    }
    pub fn draw_geom(&self, img: &mut Mat) {
        // @todo: proper error handling
        for i in 1..self.pixel_coordinates.len() {
            let prev_pt = Point2i::new(self.pixel_coordinates[i - 1].x as i32, self.pixel_coordinates[i - 1].y as i32);
            let current_pt = Point2i::new(self.pixel_coordinates[i].x as i32, self.pixel_coordinates[i].y as i32);
            match line(img, prev_pt, current_pt, self.color, 2, LINE_8, 0) {
                Ok(_) => {},
                Err(err) => {
                    panic!("Can't draw line for polygon due the error: {:?}", err)
                }
            };
        }
        let last_pt = Point2i::new(self.pixel_coordinates[self.pixel_coordinates.len() - 1].x as i32, self.pixel_coordinates[self.pixel_coordinates.len() - 1].y as i32);
        let first_pt = Point2i::new(self.pixel_coordinates[0].x as i32, self.pixel_coordinates[0].y as i32);
        match line(img, last_pt, first_pt, self.color, 2, LINE_8, 0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw line for polygon due the error: {:?}", err)
            }
        };
    }
    pub fn draw_skeleton(&self, img: &mut Mat) {
        let a = Point2i::new(self.skeleton.line[0].x as i32, self.skeleton.line[0].y as i32);
        let b = Point2i::new(self.skeleton.line[1].x as i32, self.skeleton.line[1].y as i32);
        match line(img, a, b, self.skeleton.color, 2, LINE_8, 0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw line for polygon due the error: {:?}", err)
            }
        };
    }
    pub fn draw_current_intensity(&self, img: &mut Mat) {
        let current_intensity = self.objects_registered.len();
        let anchor = Point2i::new(self.pixel_coordinates[0].x as i32 + 20, self.pixel_coordinates[0].y as i32 - 10);
        match put_text(img, &current_intensity.to_string(), anchor, FONT_HERSHEY_SIMPLEX, 0.5, Scalar::from((0.0, 0.0, 0.0)), 2, LINE_8, false) {
            Ok(_) => {},
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
        poly_element.push(vec![self.spatial_coordinates_epsg4326[0].x, self.spatial_coordinates_epsg4326[0].y]);
        geojson_poly.push(poly_element);
        ZoneFeature{
            typ: "Feature".to_string(),
            id: self.id.clone(),
            properties: ZonePropertiesGeoJSON{
                road_lane_num: self.road_lane_num,
                road_lane_direction: self.road_lane_direction,
                coordinates: euclidean,
                color_rgb: [self.color[2] as i16, self.color[1] as i16, self.color[0] as i16]
            },
            geometry: GeoPolygon{
                geometry_type: "Polygon".to_string(),
                coordinates: geojson_poly
            },
        }
    }
}

fn find_skeleton_line(coordinates: &Vec<Point2f>, first_line_idx: usize, second_line_id: usize) -> [Point2f; 2] {
    let a = coordinates[first_line_idx];
    let b = coordinates[first_line_idx+1];
    let a_b_center  = Point2f::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);

    let c = coordinates[second_line_id];
    let d = coordinates[second_line_id+1];
    let c_d_center  = Point2f::new((c.x + d.x) / 2.0, (c.y + d.y) / 2.0);

    [a_b_center, c_d_center]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_contains_point() {
        let convex_polygons = vec![
            Zone::default_from_cv(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(0.0, 5.0),
                ]
            ),
            Zone::default_from_cv(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(0.0, 5.0),
                ]
            ),
            Zone::default_from_cv(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(5.0, 0.0),
                ]
            ),
            Zone::default_from_cv(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(5.0, 0.0),
                ]
            ),
            Zone::default_from_cv(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(5.0, 0.0),
                ]
            ),
            Zone::default_from_cv(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(0.0, 5.0),
                ]
            )
        ];
        let points = vec![
            Point2f::new(20.0, 20.0),
            Point2f::new(4.0, 4.0),
            Point2f::new(3.0, 3.0),
            Point2f::new(5.0, 1.0),
            Point2f::new(7.0, 2.0),
            Point2f::new(-2.0, 12.0)
        ];
        let correct_answers = vec![
            false,
            true,
            true,
            true,
            false,
            false
        ];
        for (i, convex_polygon) in convex_polygons.iter().enumerate() {
            let answer = convex_polygon.contains_point(points[i].x, points[i].y);
            assert_eq!(answer, correct_answers[i]);
        }
    }
    
    #[test]
    fn test_vertical_line() {
        let vertical_line = VirtualLine::new(Point2f::new(4.0, 3.0), Point2f::new(5.0, 10.0));
        let c = Point2f::new(3.0, 8.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_left);

        let c = Point2f::new(5.0, 10.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_left);

        let c = Point2f::new(4.0, 3.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_left);

        let c = Point2f::new(3.9, 3.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_left);

        let c = Point2f::new(5.1, 4.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_left);

        let c = Point2f::new(35.1, 19.2);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_left);

        let c = Point2f::new(-5.0, 8.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_left);

        let c = Point2f::new(6.0, -4.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_left);

        let c = Point2f::new(-2.0, -3.0);
        let is_left = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_left);
    }
    #[test]
    fn test_horizontal_line() {
        let vertical_line = VirtualLine::new(Point2f::new(4.0, 6.0), Point2f::new(9.0, 6.4));
        let c = Point2f::new(3.0, 8.0);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_above);

        let c = Point2f::new(5.0, 3.0);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_above);

        let c = Point2f::new(0.0, 5.5);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_above);

        let c = Point2f::new(0.0, 6.5);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_above);

        let c = Point2f::new(10.0, 5.5);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_above);

        let c = Point2f::new(35.1, 8.0);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_above);

        let c = Point2f::new(2.0, 6.5);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_above);

        let c = Point2f::new(-2.0, 3.0);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(false, is_above);

        let c = Point2f::new(75.0, 15.0);
        let is_above = vertical_line.is_left(c.x, c.y);
        assert_eq!(true, is_above);
    }
    #[test]
    fn test_object_entered_cv() {
        let polygon = Zone::default_from_cv(
            vec![
                Point2f::new(23.0, 15.0),
                Point2f::new(67.0, 15.0),
                Point2f::new(67.0, 41.0),
                Point2f::new(23.0, 41.0),
            ]
        );
    
        let a_track_must_enter = vec![
            Point2f::new(34.0, 13.0),
            Point2f::new(36.0, 21.0),
        ];
        let entered = polygon.object_entered_cv(a_track_must_enter[0], a_track_must_enter[1]);
        assert_eq!(entered, true);
    
        let b_track_must_not_enter = vec![
            Point2f::new(46.0, 38.0),
            Point2f::new(49.0, 46.0),
        ];
        let entered = polygon.object_entered_cv(b_track_must_not_enter[0], b_track_must_not_enter[1]);
        assert_eq!(entered, false);
    
        let c_track_must_not_enter = vec![
            Point2f::new(55.0, 23.0),
            Point2f::new(55.0, 29.0),
        ];
        let entered = polygon.object_entered_cv(c_track_must_not_enter[0], c_track_must_not_enter[1]);
        assert_eq!(entered, false);
    
        let d_track_must_not_enter = vec![
            Point2f::new(19.0, 20.0),
            Point2f::new(19.0, 25.0),
        ];
        let entered = polygon.object_entered_cv(d_track_must_not_enter[0], d_track_must_not_enter[1]);
        assert_eq!(entered, false);
    }
    #[test]
    fn test_object_left_cv() {
        let polygon = Zone::default_from_cv(
            vec![
                Point2f::new(23.0, 15.0),
                Point2f::new(67.0, 15.0),
                Point2f::new(67.0, 41.0),
                Point2f::new(23.0, 41.0),
            ]
        );
    
        let a_track_must_enter = vec![
            Point2f::new(34.0, 13.0),
            Point2f::new(36.0, 21.0),
        ];
        let left = polygon.object_left_cv(a_track_must_enter[0], a_track_must_enter[1]);
        assert_eq!(left, false);
    
        let b_track_must_not_enter = vec![
            Point2f::new(46.0, 38.0),
            Point2f::new(49.0, 46.0),
        ];
        let left = polygon.object_left_cv(b_track_must_not_enter[0], b_track_must_not_enter[1]);
        assert_eq!(left, true);
    
        let c_track_must_not_enter = vec![
            Point2f::new(55.0, 23.0),
            Point2f::new(55.0, 29.0),
        ];
        let left = polygon.object_left_cv(c_track_must_not_enter[0], c_track_must_not_enter[1]);
        assert_eq!(left, false);
    
        let d_track_must_not_enter = vec![
            Point2f::new(19.0, 20.0),
            Point2f::new(19.0, 25.0),
        ];
        let left = polygon.object_left_cv(d_track_must_not_enter[0], d_track_must_not_enter[1]);
        assert_eq!(left, false);
    }
}