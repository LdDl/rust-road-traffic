use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ZonesFeatureCollection {
    #[serde(rename(serialize = "type"))]
    pub typ: String,
    pub features: Vec<ZoneFeature>
}

impl ZonesFeatureCollection {
    pub fn new() -> Self {
        return ZonesFeatureCollection {
            typ: "FeatureCollection".to_string(),
            features: vec![]
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ZoneFeature {
    #[serde(rename(serialize = "type"))]
    pub typ: String,
    pub id: String,
    pub properties: ZonePropertiesGeoJSON,
    pub geometry: GeoPolygon,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ZonePropertiesGeoJSON {
    pub road_lane_num: u16,
    pub road_lane_direction: u8,
    pub coordinates: Vec<Vec<i32>>,
    pub color_rgb: [i16; 3]
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct GeoPolygon {
    #[serde(rename(serialize = "type", deserialize = "type"))]
    pub geometry_type: String,
    #[serde(rename(serialize = "coordinates", deserialize = "coordinates"))]
    pub coordinates: Vec<Vec<Vec<f32>>>,
}