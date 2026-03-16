use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::Path;

use opencv::{
    core::{Mat, Vector},
    imgcodecs::imencode,
    prelude::*,
};

use crate::lib::draw::primitives::{draw_line_thick, draw_text, scalar_to_bgr};

use crate::lib::data_storage::DataStorage;

pub fn generate_report(
    data_storage: &DataStorage,
    video_src: &str,
    output_path: &str,
    first_frame: &Mat,
) -> Result<String, Box<dyn Error>> {
    let video_stem = Path::new(video_src)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");

    let zones = data_storage.zones.read().map_err(|e| format!("{}", e))?;

    // Collect traffic counts, zone coordinates and OD matrix data
    let mut traffic_csv = String::from("vehicle_type;zone_id;count\n");
    let mut zones_csv = String::from("zone_id;x1;y1;x2;y2;x3;y3;x4;y4\n");
    let mut od_csv = String::from("zone_from;zone_to;count\n");
    let mut frame = first_frame.clone();

    let w = frame.cols() as usize;
    let h = frame.rows() as usize;
    let step = w * frame.elem_size().unwrap();

    let mut zone_id_to_key: HashMap<String, String> = HashMap::new();

    for (_, zone_mutex) in zones.iter() {
        let zone = zone_mutex.lock().map_err(|e| format!("{}", e))?;
        let zone_id = zone.get_id();
        let zone_key = format!(
            "dir_{}_lane_{}",
            zone.road_lane_direction, zone.road_lane_num
        );
        zone_id_to_key.insert(zone_id.clone(), zone_key);

        // traffic_counts.csv
        for (vehicle_type, params) in zone.statistics.vehicles_data.iter() {
            traffic_csv.push_str(&format!(
                "{};{};{}\n",
                vehicle_type, zone_id, params.sum_intensity
            ));
        }

        // zones.csv
        let coords: Vec<String> = zone
            .pixel_coordinates
            .iter()
            .flat_map(|pt| vec![format!("{}", pt.x as i32), format!("{}", pt.y as i32)])
            .collect();
        zones_csv.push_str(&format!("{};{}\n", zone_id, coords.join(";")));

        // Draw zone polygon on frame
        let n = zone.pixel_coordinates.len();
        let bgr = scalar_to_bgr(&zone.color);
        let bytes = frame.data_bytes_mut()?;
        for i in 0..n {
            let j = (i + 1) % n;
            draw_line_thick(
                bytes,
                step,
                w,
                h,
                zone.pixel_coordinates[i].x as i32,
                zone.pixel_coordinates[i].y as i32,
                zone.pixel_coordinates[j].x as i32,
                zone.pixel_coordinates[j].y as i32,
                bgr,
                2,
            );
        }

        // Draw zone ID label
        draw_text(
            bytes,
            step,
            w,
            h,
            zone.pixel_coordinates[0].x as i32 + 5,
            zone.pixel_coordinates[0].y as i32 - 15,
            &zone_id,
            bgr,
            1,
        );

        // Draw vertex coordinates
        for (idx, pt) in zone.pixel_coordinates.iter().enumerate() {
            let text = format!("P{}({},{})", idx + 1, pt.x as i32, pt.y as i32);
            draw_text(
                bytes,
                step,
                w,
                h,
                pt.x as i32 + 5,
                pt.y as i32 + 8,
                &text,
                bgr,
                1,
            );
        }
    }

    let zones = data_storage.zones.read().map_err(|e| format!("{}", e))?;
    for (_, zone_mutex) in zones.iter() {
        let zone = zone_mutex.lock().map_err(|e| format!("{}", e))?;
        let to_key = zone_id_to_key.get(&zone.get_id()).unwrap();
        for (from_zone_id, flow_count) in zone.statistics.income.iter() {
            if let Some(from_key) = zone_id_to_key.get(from_zone_id) {
                if *flow_count > 0 {
                    od_csv.push_str(&format!("{};{};{}\n", from_key, to_key, flow_count));
                }
            }
        }
    }
    drop(zones);

    // Encode PNG
    let mut png_buffer = Vector::<u8>::new();
    let params = Vector::<i32>::new();
    imencode(".png", &frame, &mut png_buffer, &params)?;

    // Create ZIP
    let png_filename = format!("{}_zones.png", video_stem);
    let zip_filename = format!("{}_report.zip", video_stem);

    fs::create_dir_all(output_path)?;
    let zip_path = format!("{}/{}", output_path, zip_filename);
    let file = fs::File::create(&zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("traffic_counts.csv", options)?;
    zip.write_all(traffic_csv.as_bytes())?;

    zip.start_file("zones.csv", options)?;
    zip.write_all(zones_csv.as_bytes())?;

    zip.start_file("od_matrix.csv", options)?;
    zip.write_all(od_csv.as_bytes())?;

    zip.start_file(&png_filename, options)?;
    zip.write_all(&png_buffer.to_vec())?;

    zip.finish()?;

    Ok(zip_path)
}
