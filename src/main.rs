use chrono::Utc;

use lib::cv::Rect as RectCV;

use mot_rs::utils::Rect;
use uuid::Uuid;

#[path = "lib/mod.rs"]
mod lib;
use lib::data_storage::new_datastorage;
use lib::dataset_collector::DatasetCollector;
use lib::detection::DetectionBlobs::BBox;
use lib::detection::DetectionBlobs::Simple;
use lib::detection::Detector;
use lib::detection::KalmanFilterType;
use lib::detection::process_yolo_detections;
use lib::draw;
use lib::perf_stats::{PerfStats, Timer};
use lib::tracker::{SpatialInfo, TrackerTrait, new_tracker_from_type};
use lib::zones::Zone;

mod settings;
use settings::AppSettings;

mod video_capture;
use video_capture::{ThreadedFrame, VideoSource};

use lib::publisher::RedisConnection;

mod rest_api;

use std::collections::HashSet;
use std::env;
use std::fmt;
use std::iter::FromIterator;
use std::process;
use std::sync::mpsc;
use std::thread;
use std::time::Duration as STDDuration;
use std::time::SystemTime;

fn get_sys_time_in_secs() -> u64 {
    match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(n) => n.as_secs(),
        Err(_) => panic!("SystemTime before UNIX EPOCH!"),
    }
}

#[derive(Debug)]
struct AppVideoError {
    typ: i16,
}
impl fmt::Display for AppVideoError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.typ {
            1 => write!(f, "Can't open video"),
            2 => write!(f, "Can't make probe for video"),
            _ => write!(f, "Undefined application video error"),
        }
    }
}

#[derive(Debug)]
enum AppError {
    VideoError(AppVideoError),
    CaptureError(video_capture::CaptureError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::VideoError(e) => write!(f, "{}", e),
            AppError::CaptureError(e) => write!(f, "{}", e),
        }
    }
}

impl From<AppVideoError> for AppError {
    fn from(e: AppVideoError) -> Self {
        AppError::VideoError(e)
    }
}

impl From<video_capture::CaptureError> for AppError {
    fn from(e: video_capture::CaptureError) -> Self {
        AppError::CaptureError(e)
    }
}

fn run(
    settings: &AppSettings,
    path_to_config: &str,
    tracker: &mut dyn TrackerTrait,
    detector: &mut Detector,
    verbose: bool,
) -> Result<(), AppError> {
    println!("Verbose is '{}'", verbose);
    println!("REST API is '{}'", settings.rest_api.enable);
    println!("Redis publisher is '{}'", settings.redis_publisher.enable);

    let report_mode = settings.is_report_mode();
    if report_mode && !std::path::Path::new(&settings.input.video_src).is_file() {
        println!(
            "Report mode is not available for RTSP streams or cameras. Only video files are supported."
        );
        return Ok(());
    }

    let (enable_mjpeg, mjpeg_quality) = match &settings.rest_api.mjpeg_streaming {
        Some(v) => {
            let enabled = v.enable & settings.rest_api.enable & !report_mode;
            (enabled, v.quality)
        }
        None => (false, 80),
    };

    println!("MJPEG is '{}' (quality: {})", enable_mjpeg, mjpeg_quality);
    if report_mode {
        println!("Report mode is enabled");
    }

    /* Preprocess spatial data */
    let data_storage = new_datastorage(settings.equipment_info.id.clone(), verbose);
    let target_classes = HashSet::from_iter(
        settings
            .detection
            .target_classes
            .to_owned()
            .unwrap_or(vec![]),
    );
    let net_classes = settings.detection.net_classes.to_owned();
    let net_classes_set = HashSet::from_iter(net_classes.clone());
    let class_colors = draw::ClassColors::new(&net_classes);

    if let Some(road_lanes) = &settings.road_lanes {
        for road_lane in road_lanes.iter() {
            let mut zone = Zone::from(road_lane);
            zone.set_target_classes(if !target_classes.is_empty() {
                &target_classes
            } else {
                &net_classes_set
            });
            match data_storage.write().unwrap().insert_zone(zone) {
                Ok(_) => {}
                Err(err) => {
                    panic!("Can't insert zone due the error {:?}", err);
                }
            };
        }
    }

    /* Initialize dataset collector if enabled */
    let mut dataset_collector: Option<DatasetCollector> = match &settings.dataset_collector {
        Some(dc_settings) if dc_settings.enabled => {
            match DatasetCollector::new(dc_settings.clone(), &net_classes) {
                Ok(collector) => Some(collector),
                Err(err) => {
                    println!(
                        "[WARNING] Can't initialize DatasetCollector: {}. Feature disabled.",
                        err
                    );
                    None
                }
            }
        }
        _ => None,
    };

    // let data_storage_threaded = data_storage.clone();

    println!("Press `Ctrl-C` to stop main programm");
    ctrlc::set_handler(move || {
        println!("Ctrl+C has been pressed! Exit in 2 seconds");
        thread::sleep(STDDuration::from_secs(2));
        process::exit(1);
    })
    .expect("Error setting `Ctrl-C` handler");

    /* Start statistics ("threading" is obsolete because of business-logic error) */
    let reset_time = settings.worker.reset_data_milliseconds;
    let next_reset = if report_mode {
        f32::MAX
    } else {
        reset_time as f32 / 1000.0
    };
    let ds_worker = data_storage.clone();

    /* Redis publisher */
    let redis_enabled = settings.redis_publisher.enable;
    let redis_worker = data_storage.clone();
    let redis_conn = match redis_enabled {
        true => {
            let redis_host = settings.redis_publisher.host.to_owned();
            let redis_port = settings.redis_publisher.port;
            let redis_password = settings.redis_publisher.password.to_owned();
            let redis_db_index = settings.redis_publisher.db_index;
            let redis_channel = settings.redis_publisher.channel_name.to_owned();
            let mut redis_conn = match redis_password.chars().count() {
                0 => RedisConnection::new(redis_host, redis_port, redis_db_index, redis_worker),
                _ => RedisConnection::new_with_password(
                    redis_host,
                    redis_port,
                    redis_db_index,
                    redis_password,
                    redis_worker,
                ),
            };
            if redis_channel.chars().count() != 0 {
                redis_conn.set_channel(redis_channel);
            }
            Some(redis_conn)
        }
        false => None,
    };

    /* Start REST API if needed */
    let overwrite_file = path_to_config.to_string();
    let (tx_mjpeg, rx_mjpeg) = mpsc::sync_channel(0);
    if settings.rest_api.enable && !report_mode {
        let settings_clone = settings.clone();
        let ds_api = data_storage.clone();
        thread::spawn(move || {
            match rest_api::start_rest_api(
                settings_clone.rest_api.host.clone(),
                settings_clone.rest_api.back_end_port,
                ds_api,
                enable_mjpeg,
                rx_mjpeg,
                settings_clone,
                &overwrite_file,
            ) {
                Ok(_) => {}
                Err(err) => {
                    println!("Can't start API due the error: {:?}", err)
                }
            }
        });
    }

    /* Probe video */
    let mut video_source = VideoSource::open(&settings.input.video_src)?;
    let width = video_source.width();
    let height = video_source.height();
    let fps = video_source.fps();
    let total_frames = video_source.total_frames();

    // Initialize zone grid with frame dimensions
    {
        let ds_guard = data_storage
            .read()
            .expect("DataStorage is poisoned [RWLock]");
        match ds_guard.initialize_zone_grid(width, height) {
            Ok(_) => println!(
                "Zone grid initialized: {}x{} with 32px cells",
                (width / 32.0).ceil() as u32,
                (height / 32.0).ceil() as u32
            ),
            Err(e) => println!("Warning: Failed to initialize zone grid: {}", e),
        }
    }

    /* Start capture loop */
    let (tx_capture, rx_capture): (
        mpsc::SyncSender<ThreadedFrame>,
        mpsc::Receiver<ThreadedFrame>,
    ) = mpsc::sync_channel(0);
    thread::spawn(move || {
        let mut frames_counter: f32 = 0.0;
        let mut total_seconds: f32 = 0.0;
        let mut overall_seconds: f32 = 0.0;
        let mut processed_frames: u32 = 0;
        // @experimental
        let skip_every_n_frame = 2;
        loop {
            let read_frame = match video_source.read_frame() {
                Ok(Some(frame)) => frame,
                Ok(None) => {
                    println!("End of video stream");
                    break;
                }
                Err(e) => {
                    println!("Can't read next frame: {}", e);
                    break;
                }
            };

            frames_counter += 1.0;
            let second_fraction = total_seconds + (frames_counter / fps);
            if frames_counter >= fps {
                total_seconds += 1.0;
                overall_seconds += 1.0;
                frames_counter = 0.0;
            }
            if frames_counter as i32 % skip_every_n_frame != 0 {
                continue;
            }

            /* Send frame and capture info */
            let frame = ThreadedFrame {
                frame: read_frame,
                overall_seconds: overall_seconds,
                current_second: second_fraction,
            };

            match tx_capture.send(frame) {
                Ok(_) => {}
                Err(_err) => {
                    // Closed channel?
                    // println!("Error on send frame to detection thread: {}", _err)
                }
            };

            processed_frames += 1;
            if report_mode && total_frames > 0.0 && processed_frames % 100 == 0 {
                let progress = (processed_frames as f32 / total_frames) * 100.0;
                println!(
                    "[Report] Progress: {}/{} frames ({:.1}%)",
                    processed_frames, total_frames as u32, progress
                );
            }

            if total_seconds >= next_reset {
                println!(
                    "Reset timer due analytics. Current local time is: {}",
                    second_fraction
                );
                total_seconds = 0.0;
                let mut ds_writer = ds_worker.write().expect("Bad DS");
                if ds_writer.period_end == ds_writer.period_start {
                    // First iteration
                    ds_writer.period_end = Utc::now();
                    ds_writer.period_start =
                        ds_writer.period_end - chrono::Duration::milliseconds(reset_time);
                } else {
                    // Next iterations
                    ds_writer.period_start = ds_writer.period_end;
                    ds_writer.period_end += chrono::Duration::milliseconds(reset_time);
                }

                match ds_writer.update_statistics() {
                    Ok(_) => {
                        // Do not forget to drop mutex explicitly since we possible need to work with DS in REST API and Redis
                        drop(ds_writer)
                    }
                    Err(err) => {
                        println!("Can't update statistics due the error: {}", err);
                    }
                }
                if redis_enabled {
                    redis_conn.as_ref().unwrap().push_statistics();
                }
            }
        }
        // VideoSource cleans up via Drop (kills subprocess)
    });

    /* Detection thread */
    let conf_threshold: f32 = settings.detection.conf_threshold;
    let nms_threshold: f32 = settings.detection.nms_threshold;
    let max_points_in_track: usize = settings.tracking.max_points_in_track;
    let kalman_filter: KalmanFilterType = settings
        .tracking
        .kalman_filter
        .as_deref()
        .unwrap_or("centroid")
        .parse()
        .unwrap_or_default();

    let ds_tracker = data_storage.clone();

    let tracker_dt = 1.0 / fps;

    /* Performance stats (optional) */
    let perf_stats_interval = settings.detection.perf_stats_interval;
    let mut perf_stats = if perf_stats_interval > 0 {
        Some(PerfStats::new(perf_stats_interval))
    } else {
        None
    };

    /* JPEG encoder for MJPEG streaming (reused across frames) */
    let mut jpeg_encoder = if enable_mjpeg {
        Some(lib::mjpeg_streaming::JpegEncoder::new(
            width as u32,
            height as u32,
            mjpeg_quality,
        ))
    } else {
        None
    };

    /* Can't create colors as const/static currently */
    let mut first_frame: Option<lib::cv::RawFrame> = None;
    for received in rx_capture {
        if report_mode && first_frame.is_none() {
            first_frame = Some(received.frame.clone());
        }
        // println!("Received frame from capture thread: {}", received.current_second);
        // Note: frame clone is deferred to only displaying

        /* Inference (preprocessing + forward pass + NMS) */
        let t_inference = Timer::start();
        let (nms_bboxes, nms_classes_ids, nms_confidences) =
            match detector.detect_frame(&received.frame, conf_threshold, nms_threshold) {
                Ok((a, b, c)) => (a, b, c),
                Err(err) => {
                    println!(
                        "Can't process input of neural network due the error {:?}",
                        err
                    );
                    continue;
                }
            };
        let inference_time = t_inference.elapsed();

        /* Postprocessing: create detection blobs */
        let t_postprocess = Timer::start();
        let mut tmp_detections = process_yolo_detections(
            &nms_bboxes,
            nms_classes_ids,
            nms_confidences,
            width,
            height,
            max_points_in_track,
            &net_classes,
            &target_classes,
            tracker_dt,
            kalman_filter,
        );
        let postprocess_time = t_postprocess.elapsed();

        /* Tracking: match detections to existing tracks */
        let t_tracking = Timer::start();
        let relative_time = received.overall_seconds;
        match tracker.match_objects(&mut tmp_detections, relative_time) {
            Ok(_) => {}
            Err(err) => {
                println!("Can't match objects due the error: {:?}", err);
                continue;
            }
        };
        let tracking_time = t_tracking.elapsed();

        /* Record performance stats */
        if let Some(ref mut stats) = perf_stats {
            stats.record(inference_time, postprocess_time, tracking_time);
        }

        /* Dataset collection - save raw frame and annotations */
        if let Some(ref mut collector) = dataset_collector {
            // Gather data from matched detections
            let blob_count = tmp_detections.blobs.len();
            let mut dc_bboxes: Vec<RectCV> = Vec::with_capacity(blob_count);
            let mut dc_track_ids: Vec<Uuid> = Vec::with_capacity(blob_count);
            let mut dc_track_ages: Vec<usize> = Vec::with_capacity(blob_count);

            // Extract blob info based on the detection type
            let blob_info: Vec<(Uuid, Rect)> = match &tmp_detections.blobs {
                Simple(blobs) => blobs.iter().map(|b| (b.get_id(), b.get_bbox())).collect(),
                BBox(blobs) => blobs.iter().map(|b| (b.get_id(), b.get_bbox())).collect(),
            };

            for (track_id, bbox) in blob_info.iter() {
                // Get the actual track age from the tracker (zero-copy lookup)
                let track_age = tracker
                    .get_tracked_object_ref(track_id)
                    .map(|obj| obj.get_track().len())
                    .unwrap_or(0);

                dc_bboxes.push(RectCV::new(
                    bbox.x as i32,
                    bbox.y as i32,
                    bbox.width as i32,
                    bbox.height as i32,
                ));
                dc_track_ids.push(*track_id);
                dc_track_ages.push(track_age);
            }

            // Use raw frame (before any drawing) for dataset
            if let Err(err) = collector.process_frame(
                &received.frame,
                &dc_bboxes,
                &tmp_detections.class_names,
                &dc_track_ids,
                &dc_track_ages,
            ) {
                println!("[DatasetCollector] Error processing frame: {}", err);
            }
        }

        let ds_guard = ds_tracker.read().expect("DataStorage is poisoned [RWLock]");
        let zones = ds_guard
            .zones
            .read()
            .expect("Spatial data is poisoned [RWLock]");
        let zone_grid = ds_guard
            .zone_grid
            .read()
            .expect("Zone grid is poisoned [RWLock]");

        // Reset current occupancy for zones
        let current_ut = get_sys_time_in_secs();
        for (_, zone_guarded) in zones.iter() {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.current_statistics.occupancy = 0;
            zone.current_statistics.last_time = current_ut;
            zone.current_statistics.last_time_relative = relative_time;
            drop(zone);
        }

        // Zero-copy filtering: collect IDs of active objects
        let object_ids: Vec<Uuid> = tracker
            .iter_tracked_objects()
            .filter_map(|(id, obj)| {
                if obj.get_no_match_times() <= 1 {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();

        for object_id in object_ids {
            // Zero-copy lookup for each object
            let object = match tracker.get_tracked_object_ref(&object_id) {
                Some(obj) => obj,
                None => continue,
            };
            if object.get_no_match_times() > 1 {
                // Skip, since object is lost for a while
                // println!("Object {} is lost for a while", object_id);
                continue;
            }
            let track: &Vec<mot_rs::utils::Point> = object.get_track();
            let track_len = track.len();
            let last_point = &track[track_len - 1];
            let last_point_x = last_point.x;
            let last_point_y = last_point.y;

            let last_before_point = if track_len >= 2 {
                let pt = &track[track_len - 2];
                Some((pt.x, pt.y))
            } else {
                None
            };

            // Get the vehicle's previous zone from tracking if possible
            let previous_zone_id = {
                let vehicle_zones = ds_guard
                    .vehicle_last_zone_cross
                    .read()
                    .expect("Vehicle zones is poisoned [RWLock]");
                let result = vehicle_zones.get(&object_id).cloned();
                drop(vehicle_zones);
                result
            };
            let object_extra = tracker.get_object_extra_mut(&object_id).unwrap();
            let times = &object_extra.times;
            let last_time = times[times.len() - 1];

            // Use zone grid for O(1) candidate lookup instead of O(m) iteration
            let candidate_zone_ids = zone_grid.get_candidate_zones(last_point_x, last_point_y);
            for zone_id in candidate_zone_ids {
                let zone_guarded = match zones.get(zone_id) {
                    Some(z) => z,
                    None => continue,
                };
                let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
                if !zone.contains_point(last_point_x, last_point_y) {
                    continue;
                }
                zone.current_statistics.occupancy += 1; // Increment current load to match number of objects in zone

                let projected_pt = zone.project_to_skeleton(last_point_x, last_point_y);
                let pixels_per_meters = zone.get_skeleton_ppm();

                let crossed = if let Some(before_point) = last_before_point {
                    zone.crossed_virtual_line(
                        last_point_x,
                        last_point_y,
                        before_point.0,
                        before_point.1,
                    )
                } else {
                    false
                };
                // Only provide zone_id_from when vehicle actually crosses the virtual line
                let zone_id_from = if crossed {
                    previous_zone_id.clone()
                } else {
                    None
                };
                match object_extra.spatial_info {
                    Some(ref mut spatial_info) => {
                        spatial_info.update_avg(
                            last_time,
                            last_point_x,
                            last_point_y,
                            projected_pt.0,
                            projected_pt.1,
                            pixels_per_meters,
                        );
                        zone.register_or_update_object(
                            object_id,
                            last_time,
                            relative_time,
                            spatial_info.speed,
                            object_extra.get_classname(),
                            crossed,
                            zone_id_from.clone(),
                        );
                    }
                    None => {
                        object_extra.spatial_info = Some(SpatialInfo::new(
                            last_time,
                            last_point_x,
                            last_point_y,
                            projected_pt.0,
                            projected_pt.1,
                        ));
                        zone.register_or_update_object(
                            object_id,
                            last_time,
                            relative_time,
                            -1.0,
                            object_extra.get_classname(),
                            crossed,
                            zone_id_from.clone(),
                        );
                    }
                }
                // Only update vehicle zone tracking when vehicle crosses virtual line
                if crossed {
                    let mut vehicle_zones = ds_guard
                        .vehicle_last_zone_cross
                        .write()
                        .expect("Vehicle zones is poisoned [RWLock]");
                    vehicle_zones.insert(object_id, zone.id.clone());
                    drop(vehicle_zones);
                }
                drop(zone);
                // Vehicle can only be in one zone at a time
                break;
            }
        }

        /* Re-stream input video as MJPEG */
        if enable_mjpeg {
            let mut frame = received.frame.clone();

            for (_, v) in zones.iter() {
                let zone = v.lock().expect("Mutex poisoned");
                zone.draw_geom(&mut frame);
                zone.draw_skeleton(&mut frame);
                zone.draw_current_intensity(&mut frame);
                zone.draw_virtual_line(&mut frame);
                drop(zone);
            }

            // We need drop here explicitly, since we need to release lock on zones for MJPEG / REST API / Redis publisher and statistics threads
            drop(zone_grid);
            drop(zones);
            drop(ds_guard);
            draw::draw_track(&mut frame, tracker, &class_colors);

            match jpeg_encoder.as_mut().unwrap().encode(frame.data_bytes()) {
                Ok(jpeg_buf) => {
                    match tx_mjpeg.send(jpeg_buf) {
                        Ok(_) => {}
                        Err(_err) => {
                            println!("Error on send frame to MJPEG thread: {}", _err)
                        }
                    };
                }
                Err(e) => {
                    println!("JPEG encode failed: {}", e);
                }
            }
        } else {
            // No visualization, but need release still
            drop(zone_grid);
            drop(zones);
            drop(ds_guard);
        }
    }

    if report_mode {
        println!("Video processing complete. Generating report...");
        let report_settings = settings.report.as_ref().unwrap();
        {
            let mut ds_writer = data_storage
                .write()
                .expect("DataStorage is poisoned [RWLock]");
            ds_writer.period_end = Utc::now();
            match ds_writer.update_statistics() {
                Ok(_) => {}
                Err(err) => {
                    println!("Can't compute final statistics due the error: {}", err);
                }
            }
        }
        let ds_reader = data_storage
            .read()
            .expect("DataStorage is poisoned [RWLock]");
        match first_frame {
            Some(ref frame) => {
                match lib::report::generate_report(
                    &ds_reader,
                    &settings.input.video_src,
                    &report_settings.output_path,
                    frame,
                ) {
                    Ok(zip_path) => println!("Report saved to: {}", zip_path),
                    Err(err) => println!("Can't generate report due the error: {}", err),
                }
            }
            None => {
                println!("Can't generate report: no frames were captured from video");
            }
        }
    }

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let path_to_config = match args.len() {
        2 => &args[1],
        _ => {
            println!(
                "Args should contain exactly one string: path to TOML configuration file. Setting to default './data/conf.toml'"
            );
            "./data/conf.toml"
        }
    };
    let app_settings = AppSettings::new(path_to_config);
    println!("Settings are:\n\t{}", app_settings);

    let kalman_filter: KalmanFilterType = app_settings
        .tracking
        .kalman_filter
        .as_deref()
        .unwrap_or("centroid")
        .parse()
        .unwrap_or_default();
    let mut tracker = new_tracker_from_type(
        &app_settings.tracking.typ.as_deref().unwrap_or("iou_naive"),
        kalman_filter,
        app_settings.tracking.max_no_match,
        app_settings.tracking.iou_threshold,
    );
    println!("Tracker is:\n\t{}", tracker);

    let net_size = match (
        app_settings.detection.net_width,
        app_settings.detection.net_height,
    ) {
        (Some(w), Some(h)) => Some((w, h)),
        _ => None,
    };
    let mut detector = Detector::new(
        &app_settings.detection.network_weights,
        net_size,
        app_settings.detection.network_cfg.as_deref(),
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to create detector: {}", e);
        std::process::exit(1);
    });

    let verbose = match &app_settings.debug {
        Some(x) => x.enable,
        None => false,
    };

    match run(
        &app_settings,
        path_to_config,
        &mut *tracker,
        &mut detector,
        verbose,
    ) {
        Ok(_) => {}
        Err(_err) => {
            println!("Error in main thread: {}", _err);
        }
    };
}
