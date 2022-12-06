use std::collections::HashMap;
use chrono::{DateTime, Utc};
use uuid::Uuid;

mod geometry;
use geometry::PointsOrientation;
use geometry::{
    is_intersects,
    get_orientation,
    is_on_segment
};

use crate::lib::geojson::{
    PolygonFeatureGeoJSON,
    PolygonFeaturePropertiesGeoJSON,
    GeoPolygon
};

use opencv::{
    core::Point,
    core::Point2f,
    core::Scalar,
    core::Mat,
    imgproc::put_text,
    imgproc::FONT_HERSHEY_SIMPLEX,
    imgproc::LINE_8,
    imgproc::line
};
use std::collections::HashSet;
use crate::lib::tracking::BlobID;
use crate::lib::spatial::SpatialConverter;

#[derive(Debug)]
pub struct ConvexPolygon {
    pub id: String,
    pub pixel_coordinates: Vec<Point2f>,
    pub spatial_cooridnates:  Vec<Point2f>,
    pub color: Scalar,
    pub avg_speed: f32,
    pub sum_intensity: u32,
    pub road_lane_num: u16,
    pub road_lane_direction: u8,
    pub spatial_converter: SpatialConverter,
    pub blobs: HashSet<BlobID>,
    pub estimated_avg_speed: f32,
    pub estimated_sum_intensity: u32,
    pub statistics: HashMap<String, VehicleTypeParameters>,
    pub period_start: DateTime<Utc>,
    pub period_end: Option<DateTime<Utc>>
}

#[derive(Debug)]
pub struct VehicleTypeParameters {
    pub avg_speed: f32,
    pub sum_intensity: u32,
    pub estimated_avg_speed: f32,
    pub estimated_sum_intensity: u32
}

impl VehicleTypeParameters {
    pub fn default() -> Self {
        return VehicleTypeParameters{
            avg_speed: -1.0,
            sum_intensity: 0,
            estimated_avg_speed: 0.0,
            estimated_sum_intensity: 0
        }
    }
}
impl ConvexPolygon {
    pub fn default_from(points: Vec<Point2f>) -> Self{
        return ConvexPolygon{
            id: "dir_0_lane_0".to_string(),
            pixel_coordinates: points,
            spatial_cooridnates: vec![],
            color: Scalar::from((255.0, 255.0, 255.0)),
            avg_speed: -1.0,
            sum_intensity: 0,
            estimated_avg_speed: 0.0,
            estimated_sum_intensity: 0,
            road_lane_num: 0,
            road_lane_direction: 0,
            spatial_converter: SpatialConverter::empty(),
            blobs: HashSet::new(),
            statistics: HashMap::new(),
            period_start: Utc::now(),
            period_end: None,
        }
    }
    pub fn empty() -> Self{
        return ConvexPolygon{
            id: Uuid::new_v4().to_string(),
            pixel_coordinates: vec![],
            spatial_cooridnates: vec![],
            color: Scalar::from((255.0, 255.0, 255.0)),
            avg_speed: -1.0,
            sum_intensity: 0,
            estimated_avg_speed: 0.0,
            estimated_sum_intensity: 0,
            road_lane_num: 0,
            road_lane_direction: 0,
            spatial_converter: SpatialConverter::empty(),
            blobs: HashSet::new(),
            statistics: HashMap::new(),
            period_start: Utc::now(),
            period_end: None,
        }
    }
    pub fn new(id: String, coordinates: Vec<Point2f>, spatial_cooridnates: Vec<Point2f>, color: Scalar, road_lane_num: u16, road_lane_direction: u8) -> Self {
        let converter = SpatialConverter::new_from(coordinates.clone(), spatial_cooridnates.clone());
        ConvexPolygon{
            id: id,
            pixel_coordinates: coordinates,
            spatial_cooridnates: spatial_cooridnates,
            color: color,
            avg_speed: -1.0,
            sum_intensity: 0,
            estimated_avg_speed: 0.0,
            estimated_sum_intensity: 0,
            road_lane_num: road_lane_num,
            road_lane_direction: road_lane_direction,
            spatial_converter: converter,
            blobs: HashSet::new(),
            statistics: HashMap::new(),
            period_start: Utc::now(),
            period_end: None,
        }
    }
    pub fn get_id(&self) -> String {
        return self.id.clone();
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
    pub fn update_pixel_map(&mut self, pixel_src_points: Vec<Point2f>) {
        self.pixel_coordinates = pixel_src_points;
        if self.spatial_cooridnates.len() == 0 {
            self.spatial_cooridnates = self.pixel_coordinates.iter().map(|pt| Point2f::new(pt.x as f32, pt.y as f32)).collect();
        }
        self.spatial_converter = SpatialConverter::new_from(self.pixel_coordinates.clone(), self.spatial_cooridnates.clone());
        
    }
    pub fn update_spatial_map(&mut self, spatial_dest_points: Vec<Point2f>) {
        self.spatial_cooridnates = spatial_dest_points;
        if self.pixel_coordinates.len() == 0 {
            self.pixel_coordinates = self.pixel_coordinates.iter().map(|pt| Point2f::new(pt.x as f32, pt.y as f32)).collect();
        }
        self.spatial_converter = SpatialConverter::new_from(self.pixel_coordinates.clone(), self.spatial_cooridnates.clone());
    }
    pub fn update_pixel_map_arr(&mut self, pixel_src_points: [[u16; 2]; 4]) {
        let val = pixel_src_points.iter()
            .map(|pt| Point2f::new(pt[0] as f32, pt[1] as f32))
            .collect();
        self.update_pixel_map(val);
    }
    pub fn update_spatial_map_arr(&mut self, spatial_dest_points: [[f32; 2]; 4]) {
        let val = spatial_dest_points.iter()
            .map(|pt| Point2f::new(pt[0], pt[1]))
            .collect();
        self.update_spatial_map(val);
    }
    pub fn update_pixel_map_vec(&mut self, pixel_src_points: &Vec<Vec<u16>>) {
        let val = pixel_src_points.iter()
            .map(|pt| Point2f::new(pt[0] as f32, pt[1] as f32))
            .collect();
        self.update_pixel_map(val);
    }
    pub fn update_spatial_map_vec(&mut self, spatial_dest_points: &Vec<Vec<f32>>) {
        let val = spatial_dest_points.iter()
            .map(|pt| Point2f::new(pt[0], pt[1]))
            .collect();
        self.update_spatial_map(val);
    }
    pub fn set_color(&mut self, color_rgb: [i16; 3]) {
        self.color = Scalar::from((color_rgb[2] as f64, color_rgb[1] as f64, color_rgb[0] as f64))
    }
    pub fn set_target_classes(&mut self, vehicle_types: &'static [&'static str]) {
        for class in vehicle_types.iter() {
            self.statistics.insert(class.to_string(), VehicleTypeParameters::default());
        }
    }
    // Checks if given polygon contains a point
    // Code has been taken from: https://github.com/LdDl/odam/blob/master/virtual_polygons.go#L180
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        let n = self.pixel_coordinates.len();
        // @todo: math.maxInt could lead to overflow obviously. Need good workaround. PRs are welcome
        let extreme_point = vec![99999.0, y as f32];
        let mut intersections_cnt = 0;
	    let mut previous = 0;
        let x_f32 = x as f32;
        let y_f32 = y as f32;
        loop {
            let current = (previous + 1) % n;
            // Check if the segment from given point P to extreme point intersects with the segment from polygon point on previous interation to  polygon point on current interation
            if is_intersects(
                self.pixel_coordinates[previous].x as f32, self.pixel_coordinates[previous].y as f32,
                self.pixel_coordinates[current].x as f32, self.pixel_coordinates[current].y as f32,
                x_f32, y_f32,
                extreme_point[0], extreme_point[1]
            ) 
            {
                let orientation = get_orientation(
                    self.pixel_coordinates[previous].x as f32, self.pixel_coordinates[previous].y as f32,
                    x_f32, y_f32,
                    self.pixel_coordinates[current].x as f32, self.pixel_coordinates[current].y as f32
                );
                // If given point P is collinear with segment from polygon point on previous interation to  polygon point on current interation
                if orientation == PointsOrientation::Collinear {
                    // then check if it is on segment
				    // 'True' will be returns if it lies on segment. Otherwise 'False' will be returned
                    return is_on_segment(
                        self.pixel_coordinates[previous].x as f32, self.pixel_coordinates[previous].y as f32,
                        x_f32, y_f32,
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
        return false
    }
    pub fn contains_cv_point(&self, pt: &Point) -> bool {
        return self.contains_point(pt.x, pt.y);
    }
    // Checks if an object has entered the polygon
    // Let's clarify for future questions: we are assuming the object is represented by a center, not a bounding box
    // So object has entered polygon when its center had entered polygon too
    pub fn object_entered(&self, track: Vec<Point>) -> bool {
	    let n = track.len();
        if n < 2 {
            // If blob has been met for the first time
            return self.contains_cv_point(&track[0]);
        }
        let last_position = track[n-1];
	    let second_last_position = track[n-2];
        // If P(xN-1,yN-1) is not inside of polygon and P(xN,yN) is inside of polygon then object has entered the polygon
        if !self.contains_cv_point(&second_last_position) && self.contains_cv_point(&last_position) {
            return true;
        }
        return false;
    }
    // Checks if an object has left the polygon
    // Let's clarify for future questions: we are assuming the object is represented by a center, not a bounding box
    // So object has left polygon when its center had left polygon too
    pub fn object_left(&self, track: Vec<Point>) -> bool {
	    let n = track.len();
        if n < 2 {
            // Blob had to enter the polygon before leaving it. So track must contain atleast 2 points
            return false
        }
        let last_position = track[n-1];
	    let second_last_position = track[n-2];
        // If P(xN-1,yN-1) is not inside of polygon and P(xN,yN) is inside of polygon then object has entered the polygon
        if self.contains_cv_point(&second_last_position) && !self.contains_cv_point(&last_position) {
            return true;
        }
        return false;
    }
    pub fn blob_registered(&self, blob_id: &BlobID) -> bool {
        return self.blobs.contains(blob_id);
    }
    pub fn register_blob(&mut self, blob_id: BlobID) {
        self.blobs.insert(blob_id);
    }
    pub fn deregister_blob(&mut self, blob_id: &BlobID) {
        self.blobs.remove(blob_id);
    }
    pub fn increment_intensity(&mut self, vehicle_type: String) {
        // Certain vehicle type
        let mut vehicle_type_statistics = self.statistics.entry(vehicle_type).or_insert(VehicleTypeParameters::default());
        vehicle_type_statistics.sum_intensity += 1;
        // Summary
        self.sum_intensity += 1;
    }
    pub fn consider_speed(&mut self, vehicle_type: String, speed_value: f32) {
        // Certain vehicle type
        let mut vehicle_type_statistics = self.statistics.entry(vehicle_type).or_insert(VehicleTypeParameters::default());
        if vehicle_type_statistics.avg_speed < 0.0 {
            vehicle_type_statistics.avg_speed = speed_value;
        } else if vehicle_type_statistics.avg_speed == f32::NAN {
            vehicle_type_statistics.avg_speed = speed_value;
        } else if vehicle_type_statistics.avg_speed == f32::INFINITY {
            vehicle_type_statistics.avg_speed = speed_value;
        } else {
            let x = 1.0 / (vehicle_type_statistics.sum_intensity as f32);
            let y = 1.0 - x;
            vehicle_type_statistics.avg_speed = x * speed_value + y * vehicle_type_statistics.avg_speed;
        }
        // Summary
        if self.avg_speed < 0.0 {
            self.avg_speed = speed_value;
        } else if self.avg_speed == f32::NAN {
            self.avg_speed = speed_value;
        } else if self.avg_speed == f32::INFINITY {
            self.avg_speed = speed_value;
        } else {
            let x = 1.0 / (self.sum_intensity as f32);
            let y = 1.0 - x;
            self.avg_speed = x * speed_value + y * self.avg_speed;
        }

    }
    pub fn scale_geom(&mut self, scale_factor_x: f32, scale_factor_y: f32) {
        for pair in self.pixel_coordinates.iter_mut() {
            pair.x = (pair.x * scale_factor_x).floor();
            pair.y = (pair.y * scale_factor_y).floor();
        }
    }
    pub fn draw_geom(&self, img: &mut Mat) {
        // @todo: proper error handling
        for i in 1..self.pixel_coordinates.len() {
            let prev_pt = Point::new(self.pixel_coordinates[i - 1].x as i32, self.pixel_coordinates[i - 1].y as i32);
            let current_pt = Point::new(self.pixel_coordinates[i].x as i32, self.pixel_coordinates[i].y as i32);
            match line(img, prev_pt, current_pt, self.color, 2, LINE_8, 0) {
                Ok(_) => {},
                Err(err) => {
                    panic!("Can't draw line for polygon due the error: {:?}", err)
                }
            };
        }
        let last_pt = Point::new(self.pixel_coordinates[self.pixel_coordinates.len() - 1].x as i32, self.pixel_coordinates[self.pixel_coordinates.len() - 1].y as i32);
        let first_pt = Point::new(self.pixel_coordinates[0].x as i32, self.pixel_coordinates[0].y as i32);
        match line(img, last_pt, first_pt, self.color, 2, LINE_8, 0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw line for polygon due the error: {:?}", err)
            }
        };
    }
    pub fn draw_params(&self, img: &mut Mat) {
        let first_pt = Point::new(self.pixel_coordinates[0].x as i32, self.pixel_coordinates[0].y as i32);
        let anchor_speed = Point::new(first_pt.x, first_pt.y + 15);
        match put_text(img, &format!("speed: {:.2} km/h", self.avg_speed), anchor_speed, FONT_HERSHEY_SIMPLEX, 0.7, Scalar::from((0.0, 255.0, 255.0)), 2, LINE_8, false) {
            Ok(_) => {},
            Err(err) => {
                println!("Can't display average speed for polygon due the error {:?}", err);
            }
        };
        let anchor_intensity = Point::new(first_pt.x, first_pt.y + 35);
        match put_text(img, &format!("count: {}", self.sum_intensity), anchor_intensity, FONT_HERSHEY_SIMPLEX, 0.7, Scalar::from((0.0, 255.0, 255.0)), 2, LINE_8, false) {
            Ok(_) => {},
            Err(err) => {
                println!("Can't display summary intensity for polygon due the error {:?}", err);
            }
        };
    }
    pub fn to_geojson(&self) -> PolygonFeatureGeoJSON {
        let mut euclidean: Vec<Vec<i32>> = Vec::new();
        for pt in self.pixel_coordinates.iter() {
            euclidean.push(vec![pt.x as i32, pt.y as i32]);
        }
        let mut geojson_poly = vec![];
        let mut poly_element = vec![];
        for v in self.spatial_cooridnates.iter() {
            poly_element.push(vec![v.x, v.y]);
        }
        poly_element.push(vec![self.spatial_cooridnates[0].x, self.spatial_cooridnates[0].y]);
        geojson_poly.push(poly_element);
        return PolygonFeatureGeoJSON{
            typ: "Feature".to_string(),
            id: self.id.clone(),
            properties: PolygonFeaturePropertiesGeoJSON{
                road_lane_num: self.road_lane_num,
                road_lane_direction: self.road_lane_direction,
                coordinates: euclidean,
                color_rgb: [self.color[2] as i16, self.color[1] as i16, self.color[0] as i16]
            },
            geometry: GeoPolygon{
                geometry_type: "Polygon".to_string(),
                coordinates: geojson_poly
            },
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_contains_point() {
        let convex_polygons = vec![
            ConvexPolygon::default_from(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(0.0, 5.0),
                ]
            ),
            ConvexPolygon::default_from(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(0.0, 5.0),
                ]
            ),
            ConvexPolygon::default_from(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(5.0, 0.0),
                ]
            ),
            ConvexPolygon::default_from(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(5.0, 0.0),
                ]
            ),
            ConvexPolygon::default_from(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(5.0, 0.0),
                ]
            ),
            ConvexPolygon::default_from(
                vec![
                    Point2f::new(0.0, 0.0),
                    Point2f::new(5.0, 0.0),
                    Point2f::new(5.0, 5.0),
                    Point2f::new(0.0, 5.0),
                ]
            )
        ];
        let points = vec![
            Point::new(20, 20),
            Point::new(4, 4),
            Point::new(3, 3),
            Point::new(5, 1),
            Point::new(7, 2),
            Point::new(-2, 12)
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
    fn test_object_entered() {
        let polygon = ConvexPolygon::default_from(
            vec![
                Point2f::new(23.0, 15.0),
                Point2f::new(67.0, 15.0),
                Point2f::new(67.0, 41.0),
                Point2f::new(23.0, 41.0),
            ]
        );

        let a_track_must_enter = vec![
            Point::new(36, 7),
            Point::new(34, 13),
            Point::new(36, 21),
        ];
        let entered = polygon.object_entered(a_track_must_enter);
        assert_eq!(entered, true);

        let b_track_must_not_enter = vec![
            Point::new(45, 35),
            Point::new(46, 38),
            Point::new(49, 46),
        ];
        let entered = polygon.object_entered(b_track_must_not_enter);
        assert_eq!(entered, false);

        let c_track_must_not_enter = vec![
            Point::new(56, 19),
            Point::new(55, 23),
            Point::new(55, 29),
        ];
        let entered = polygon.object_entered(c_track_must_not_enter);
        assert_eq!(entered, false);

        let d_track_must_not_enter = vec![
            Point::new(17, 13),
            Point::new(19, 20),
            Point::new(19, 25),
        ];
        let entered = polygon.object_entered(d_track_must_not_enter);
        assert_eq!(entered, false);
    }
    #[test]
    fn test_object_left() {
        let polygon = ConvexPolygon::default_from(
            vec![
                Point2f::new(23.0, 15.0),
                Point2f::new(67.0, 15.0),
                Point2f::new(67.0, 41.0),
                Point2f::new(23.0, 41.0),
            ]
        );

        let a_track_must_enter = vec![
            Point::new(36, 7),
            Point::new(34, 13),
            Point::new(36, 21),
        ];
        let left = polygon.object_left(a_track_must_enter);
        assert_eq!(left, false);

        let b_track_must_not_enter = vec![
            Point::new(45, 35),
            Point::new(46, 38),
            Point::new(49, 46),
        ];
        let left = polygon.object_left(b_track_must_not_enter);
        assert_eq!(left, true);

        let c_track_must_not_enter = vec![
            Point::new(56, 19),
            Point::new(55, 23),
            Point::new(55, 29),
        ];
        let left = polygon.object_left(c_track_must_not_enter);
        assert_eq!(left, false);

        let d_track_must_not_enter = vec![
            Point::new(17, 13),
            Point::new(19, 20),
            Point::new(19, 25),
        ];
        let left = polygon.object_left(d_track_must_not_enter);
        assert_eq!(left, false);
    }
}