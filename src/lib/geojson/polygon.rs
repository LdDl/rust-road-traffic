use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct PolygonsGeoJSON {
    #[serde(rename(serialize = "type"))]
    pub typ: String,
    pub features: Vec<PolygonFeatureGeoJSON>
}

impl PolygonsGeoJSON {
    pub fn new() -> Self {
        return PolygonsGeoJSON {
            typ: "FeatureCollection".to_string(),
            features: vec![]
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PolygonFeatureGeoJSON {
    #[serde(rename(serialize = "type"))]
    pub typ: String,
    pub id: String,
    pub properties: PolygonFeaturePropertiesGeoJSON,
    pub geometry: GeoPolygon,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PolygonFeaturePropertiesGeoJSON {
    pub road_lane_num: u16,
    pub road_lane_direction: u8,
    pub coordinates: Vec<Vec<i32>>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct GeoPolygon {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub geometry_type: String,
    #[serde(rename(serialize = "coordinates", deserialize = "coordinates"))]
    pub coordinates: Vec<Vec<Vec<f32>>>,
}