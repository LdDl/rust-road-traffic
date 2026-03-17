# Rust toy utility for monitoring road traffic

## Table of Contents
- [Video showcase](#video-showcase)
- [About](#about)
- [Inference Backends](#inference-backends)
- [Traffic flow parameters](#traffic-flow-parameters)
- [Installation and usage](#installation-and-usage)
- [Virtual lines](#virtual-lines)
- [Dataset collection](#dataset-collection-auto-labeling)
- [Report mode](#report-mode)
- [ROADMAP](#roadmap)
- [Support](#support)

## Video showcase

<video src='https://github.com/user-attachments/assets/49fcc355-c05f-4847-961f-13a4abb1b0a6' width="720px"></video>

## About

Vehicle detection/tracking and speed estimation via next instruments:
1. Rust programming language - https://www.rust-lang.org/
2. Object detection - [od_opencv](https://github.com/LdDl/od_opencv) crate with three inference backends:
   - OpenCV DNN (optional) - https://github.com/twistedfall/opencv-rust
   - ONNX Runtime - https://onnxruntime.ai/
   - TensorRT - https://developer.nvidia.com/tensorrt
3. Linear algebra - https://github.com/dimforge/nalgebra
4. YOLO models: YOLOv3/v4/v7 (Darknet), YOLOv8/v9/v11 (Ultralytics) - https://github.com/ultralytics/ultralytics
5. actix-web for web part - https://actix.rs/

Notice:

UI is developed in seprate repository: https://github.com/LdDl/rust-road-traffic-ui. Prepared `static` directory after `npm run build` is [here](src/lib/rest_api/static/)' 

## Inference Backends

This project supports three inference backends via compile-time feature flags:

| Backend | Feature Flag | Models Supported | GPU Support | Requires OpenCV |
|---------|--------------|------------------|-------------|-----------------|
| OpenCV DNN | `opencv-backend` (default) | YOLOv3/v4/v7 (Darknet), YOLOv8/v9/v11 (ONNX) | CUDA, OpenCL | Yes |
| ONNX Runtime | `ort-backend` | YOLOv8/v9/v11 (ONNX only) | CUDA 12.x | No |
| TensorRT | `tensorrt-backend` | YOLOv8/v9/v11 (`.engine` only) | CUDA (native TensorRT) | No |

**`ort-backend` and `tensorrt-backend` do NOT require OpenCV** on the system. Video capture uses ffmpeg/GStreamer subprocesses, drawing uses own primitives, image encoding uses [`turbojpeg`](https://github.com/libjpeg-turbo/libjpeg-turbo)/[`png`](https://github.com/image-rs/image-png) crates.

**`tensorrt-backend`** is designed for NVIDIA embedded platforms (e.g. Jetson Nano) and discrete NVIDIA GPUs with TensorRT installed.

**Network input size:** For Darknet models (`.cfg` + `.weights`), `net_width`/`net_height` in TOML config are __ignored__ - the input size is read directly from the `[net]` section of the `.cfg` file. For TensorRT (`.engine`), input size is also auto-detected from the engine bindings. For ONNX models, `net_width`/`net_height` must be specified in the TOML config.

**Note:** In case of non-OpenCV backend, traditional models (YOLOv3/v4/v7) in Darknet format (`.cfg` + `.weights`) should be converted to ONNX first via [darknet2onnx](https://github.com/LdDl/darknet2onnx) with `--format yolov8` flag. The `--format yolov5` output is **not supported** (different post-processing). For TensorRT, convert ONNX to `.engine` via `trtexec`.

### Build Commands

By default MJPEG streaming links to system `libturbojpeg` via `pkg-config`. Add `--features turbojpeg-vendor` to any build command below to build libjpeg-turbo from source instead (requires `cmake`).

```bash
# OpenCV backend (default) - requires OpenCV installed, supports traditional and Ultralytics models
cargo build --release

# ORT backend - no OpenCV needed at all
cargo build --release --no-default-features --features ort-backend

# TensorRT backend - no OpenCV needed at all (Jetson / NVIDIA GPU)
cargo build --release --no-default-features --features tensorrt-backend

# Any backend + vendored libjpeg-turbo (no system lib needed, requires cmake)
cargo build --release --features turbojpeg-vendor
cargo build --release --no-default-features --features tensorrt-backend,turbojpeg-vendor
cargo build --release --no-default-features --features ort-backend,turbojpeg-vendor
```

### Run Commands

```bash
# OpenCV backend
cargo run --release -- path-to-toml-file

# ORT backend
cargo run --release --no-default-features --features ort-backend -- path-to-toml-file

# TensorRT backend
cargo run --release --no-default-features --features tensorrt-backend -- path-to-toml-file
```

### Converting models to TensorRT `.engine` format

Use `trtexec` (included with TensorRT / JetPack) to convert an ONNX model:

```bash
trtexec --onnx=yolov8n.onnx --saveEngine=yolov8n.engine --fp16
```

On Jetson devices, `trtexec` is typically located at `/usr/src/tensorrt/bin/trtexec`.

Be aware that Traditional models `v3`, `v4` and `v7` should be converted to ONNX first via [darknet2onnx](https://github.com/LdDl/darknet2onnx) with `--format yolov8` flag, and then via `trtexec` to TensorRT engine format.

## Traffic flow parameters

Both REST API and Redis publisher export following parameters for each user-defined vehicle class:

- __Flow__ (a.k.a intensity)

    Number of vehicles that pass given zone (or cross virtual line) in a specified period of time - `vehicles per period`.

    You can cast any given flow to `vehicles/hour` by multiplying the value by specific multiplier. E.g. software outputs 100 `vehicles per 15 minutes`, then you may have 100*15=1500 `vehicles/hour`.
    
    Look up at [Virtual lines](#virtual-lines) section on how vehicles are counted for additional information.

- __Defined flow__

    Same as just flow, but it is a number of vehicles that both pass given zone (or cross virtual line) in specified period of time and HAVE defined speed. Sometimes software just can't calculate speed, therefore _defined_ flow has been introduced. _This parameter could be renamed in further (in both documentation and code/api)._

- __Average speed of the flow__

   Basically it is just average value among all speeds of vehicles that pass given zone (or cross virtual line) in a specified period of time. If speed could not be determined due some circumstances it is considered to be `-1` (and not to be used in average aggregation).

For the all user-defined vehicles' classes there are:
- Same as for single vehicle class: __Flow__, __Defined Flow__, __Average speed of the flow__. 
- __Average headway__

    Average value amoung all calculated headway values. Headhway is the time that elapsed between the arrival of the leading vehicle and following vehicle to the zone (or virtual line).

    Let's break down an example: you have 3 vehicles crossed a certain virtual line at specific times: `[10:00:01, 10:00:07, 10:00:09]`. Then you have differences: `[10:00:07 - 10:00:01, 10:00:09 - 10:00:07]` which gives `[6 seconds, 2 seconds]` headways which gives `(6+2)/2 = 4 seconds` as average headway.

    You may ask: why average headway is not calculated for single class? 
    -- It does not make that much sense to estimate it because headway is not that representative for some specific classes (e.g. bus) due the nature of distribution of that classes among the popular ones (e.g. personal cars). It could be reconsidered in further for some edge cases (PR's are welcome).

* __OD (origin-destination) matrix__ - connections between different zones. Each connection is represented as number of vehicles moved from origin zone to destination one.

Locally you can access Swagger UI documentation via http://localhost:42001/api/docs:

<img src="data/docs_1.png" width="720">

## Screenshots
* MJPEG streaming output:

    <img src="data/ui3_new.png" width="320">

    <details>
    <summary>Legacy screenshots</summary>
    <img src="data/tiny-yolov4-example-output-1.jpeg" width="320"> | <img src="data/tiny-yolov4-example-output-2.jpeg" width="320">
    </details>

* Web-UI for configuration:

    <img src="data/ui2_new.png" width="640">

    <img src="data/ui3_new.gif" width="640">

## Installation and usage
1. You need installed Rust compiler obviously. Follow instruction of official site: https://www.rust-lang.org/tools/install

2. **OpenCV** is only required for `opencv-backend` (the default). If you use `ort-backend` or `tensorrt-backend`, skip this step.
    In case of need `opencv-backend` I'd highly recommend to use OpenCV >= 4.7.0 with CUDA. Here is [Makefile](Makefile) adopted from [this one](https://github.com/hybridgroup/gocv/blob/release/Makefile) if you want build it from sources (it's targeted for Linux user obviously).
    ```shell
    sudo make install_cuda
    ```

    __Be aware: OpenCV < 4.7.0 probably wont work with YOLOv8 (even with ONNX opset12) if you need those.__

3. You need `ffmpeg` and `ffprobe` installed for video capture (files, RTSP streams, USB cameras). For GStreamer pipelines (e.g. CSI cameras on Jetson) you need `gst-launch-1.0` as well.
    ```shell
    # Debian/Ubuntu
    sudo apt install ffmpeg
    # GStreamer (optional, for CSI/MIPI cameras)
    sudo apt install gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good

    # Arch Linux
    sudo pacman -S ffmpeg
    # GStreamer (optional, for CSI/MIPI cameras)
    sudo pacman -S gstreamer gst-plugins-base gst-plugins-good
    ```

    Source type is auto-detected from `video_src` in the configuration file:

    | Source | Example `video_src` | Backend |
    |--------|---------------------|---------|
    | Video file | `"./data/video.mp4"` | ffmpeg |
    | RTSP stream | `"rtsp://user:pass@192.168.1.10:554/stream"` | ffmpeg (TCP) |
    | USB camera (V4L2) | `"0"` or `"/dev/video0"` | ffmpeg |
    | GStreamer pipeline | starts with known source element (e.g. `nvarguscamerasrc`, `v4l2src`), contains ` ! ` | gst-launch-1.0 |

    GStreamer pipeline examples (use `appsink` as sink - it will be replaced with `fdsink fd=1` automatically; pipeline must output BGR format; width/height/framerate are parsed from caps in the pipeline string):

    ```toml
    # CSI camera via nvarguscamerasrc (Jetson Nano 4gb in my case)
    video_src = "nvarguscamerasrc sensor-id=0 ! video/x-raw(memory:NVMM), width=(int)1280, height=(int)720, format=(string)NV12, framerate=(fraction)30/1 ! nvvidconv flip-method=0 ! video/x-raw, width=(int)1280, height=(int)720, format=(string)BGRx ! videoconvert ! video/x-raw, format=(string)BGR ! appsink"

    # USB camera via GStreamer (alternative to V4L2)
    video_src = "v4l2src device=/dev/video0 ! video/x-raw, width=(int)640, height=(int)480, framerate=(fraction)30/1, format=(string)BGR ! appsink"
    ```

4. Dependencies are managed via [Cargo.toml](Cargo.toml). OpenCV bindings are pulled automatically when using `opencv-backend`.

5. Clone the repo
    ```shell
    git clone https://github.com/LdDl/rust-road-traffic.git
    ```
    Well, actually I provide yolov4-tiny configuration and weights file from [official repository](https://github.com/AlexeyAB/darknet) (authors of YOLOv4), but you are free to use yours.
    I provide video file as sample also.
    
6. Сhange parameters for this utility by using template of [configuration file](data/conf.toml). There is detailed explanation of each parameter.

7. Download weights and configuration files (optional)

    - YOLO v4 tiny - [yolov4-tiny-vehicles-rect_best.weights](https://github.com/LdDl/yolo_vehicles/releases/download/v0.0.1/yolov4-tiny-vehicles-rect_best.weights) + [yolov4-tiny-vehicles-rect.cfg](https://github.com/LdDl/yolo_vehicles/releases/download/v0.0.1/yolov4-tiny-vehicles-rect.cfg). It has been trained on filtered COCO dataset; classes are: "car", "motorbike", "bus", "train", "truck"

    - YOLO v3 tiny - [tinyv3-vehicles_best.weights](https://github.com/LdDl/yolo_vehicles/releases/download/v0.0.1/tinyv3-vehicles_best.weights) + [tinyv3-vehicles.cfg](https://github.com/LdDl/yolo_vehicles/releases/download/v0.0.1/tinyv3-vehicles.cfg). It has been trained on AIC HCMC 2020 challenge data; classes are: "car", "motorbike", "bus", "truck". More information here: https://github.com/LdDl/yolo_vehicles . I like it more personally.

8. Run
    ```shell
    cargo run path-to-toml-file
    ```
    If you want to use some Rust's optimizations then call build and run
    ```shell
    cargo build --release && ./target/release/rust-road-traffic path-to-toml-file
    ```
    If you want both optimized in term of perfomance and stripped executable binary (thanks to https://github.com/rust-lang/cargo/issues/3483)
    ```shell
    export RUSTFLAGS='-C link-arg=-s' && cargo build --release && ./target/release/rust-road-traffic path-to-toml-file
    ```

9. UI configuration

    If you enabled both REST API and MJPEG streaming and you want to adjust parameters for detection zones you could open http://localhost:42001/ in your browser and adjust polygons as you need (this UI still needs to be debugged and polished):

    <img src="data/ui1_new.png" width="640">

    Configuration file lines:
    ```toml
    [rest_api]
        enable = true
        host = "0.0.0.0"
        back_end_port = 42001
        api_scope = "/api"
        [rest_api.mjpeg_streaming]
            enable = true
            # 0-100
            quality = 80
    ```

10. Tracker configuration
    It is possible to pick either iou_naive or bytetrack tracker for tracking objects.

    ```toml
    [tracking]
        # Either "bytetrack" or "iou_naive". Default is "iou_naive"
        type = "iou_naive"
        # Adjust number of points for each object in its track
        max_points_in_track = 100
        # Kalman filter type: "centroid" or "bbox"
        # Default is "centroid"
        kalman_filter = "centroid"
        # Maximum number of frames to keep tracking an object without new detections. Default is 60.
        # Increase this value (e.g., 90-120) for better OD matrix results when objects need to traverse between distant zones.
        max_no_match = 60
        # IoU threshold for matching detections to existing tracks. Default is 0.3.
        # Lower values (0.2-0.25) make matching stricter, higher values (0.4-0.5) make it more permissive.
        iou_threshold = 0.3
    ```

    **Configurable parameters:**
    - `max_no_match` (default: 60): Maximum consecutive frames without detection before dropping a track. Increase for better OD matrix tracking across distant zones.
    - `iou_threshold` (default: 0.3): IoU threshold for matching detections to tracks. Lower = stricter matching.

    **Fixed parameters for ByteTrack:**
    - high_thresh = 0.7
    - low_thresh = 0.3
    - algorithm = Hungarian (matching algorithm)
    
    ### Kalman Filter Types

    The `kalman_filter` option selects the internal Kalman filter used for state prediction:

    | Feature | `centroid` | `bbox` |
    |---------|------------|--------|
    | State vector | $(x, y, v_x, v_y)$ | $(x, y, w, h, v_x, v_y, v_w, v_h)$ |
    | State dimensions | 4D | 8D |
    | Tracks position | v | v |
    | Tracks velocity | v | v |
    | Tracks size (width/height) | no | v |
    | Best for | Objects with stable size | Objects changing size/aspect ratio |
    | Computational cost | Lower | Higher |

    **When to use `bbox`:**
    - Objects moving towards/away from camera (size changes)
    - Varying aspect ratios during movement
    - Better bounding box stability during occlusions

    **When to use `centroid` (default):**
    - Side-view cameras where object size is relatively stable
    - Lower computational requirements
    - Simpler tracking scenarios

11. REST API

    If you want to do some REST calls you can do following (based on *rest_api* field in TOML configuration files)
    ```bash
    # Get polygons (GeoJSON) in which road traffic monitoring is requested
    curl -XGET 'http://localhost:42001/api/polygons/geojson'
    # Get statistics info for each polygon and each vehicle type in that polygon
    curl -XGET 'http://localhost:42001/api/stats/all'
    ```
   
12. Export data

    If you've enabled Redis output you can connect to Redis server (e.g. via CLI) and monitor incoming messages:
    
    <img src="data/redis.png" width="640">

    Configuration file lines:
    ```toml
    [redis_publisher]
        enable = true
        host = "localhost"
        port = 6379
        password = ""
        db_index = 0
        channel_name = "DETECTORS_STATISTICS"
    ```

    Both REST API and Redis publisher reset statistics in specific amount of time which could be adjusted via `reset_data_milliseconds` option:
    ```toml
    [worker]
        reset_data_milliseconds = 30000
    ```

## Virtual lines

This utility supports vehicle counting via two approaches:
| Vehicle appeared in the zone    | Vehicle crossed the line        |
:--------------------------------:|:--------------------------------:
<img src="data/without-line.gif" width="320"> | <img src="data/with-line.gif" width="320">

_But what is the point to have optional virtual line for zone? Why just not use either line or zone at all?_

-- Well, zone is essential for estimating speed, so it is needed for sure. Why need line then: sometimes it is needed to register vehicles in specific direction only or at specific moment of time (in center of zone, in bottom of zone, after zone and etc.).

You can configure virtual lines via configuration file or via UI (look at [showcase](#video-showcase)):
```toml
[[road_lanes]]
    lane_number = 0
    lane_direction = 0
    geometry = [[204, 542], [398, 558], [506, 325], [402, 318]]
    geometry_wgs84 = [[-3.7058048784300297,40.39308821416677],[-3.7058296599552705,40.39306089952626],[-3.7059466895758533,40.393116604041296],[-3.705927467488266,40.39314855180666]]
    color_rgb = [255, 0, 0]
    # Remove lines below if you don't want to include virtual line
    # Note: There is only one possible virtual line for given zone 
    [road_lanes.virtual_line]
        geometry = [[254, 456], [456, 475]]
        color_rgb = [255, 0, 0]
        # inbound - inbound traffic (towards target side)
        # outbound - outbound traffic (away from target side)
        direction = "inbound"
```

## Dataset Collection (Auto-labeling)

This utility can also automatically collect training datasets from video streams using a strong detection model. Useful for preparing YOLO training data on your specific use case.

### How it works
1. Run software with a reliable detection model (e.g., traditional YOLOv7 or Ultralytics YOLOv8)
2. Use static video file (I mean you can prepare dataset from RTSP stream, but it is recommended to use video file for reproducibility and also could be use to retrain model and check results on same video again)
2. Software tracks detected vehicles over time
3. When a track is "mature" (visible for N frames, not touching frame edges), it saves:
   - Raw image (no overlays/drawings) to `output_dir/images/`
   - YOLO format annotations to `output_dir/labels/`

### Configuration
```toml
# Dataset collector - auto-labeling feature for collecting training data
# This section is optional - if not present, the feature is disabled
[dataset_collector]
    # Enable/disable dataset collection
    enabled = true
    # Output directory for images/ and labels/ folders
    output_dir = "./collected_dataset"
    # Label format: "yolo" for standard YOLO format (class_id center_x center_y width height)
    # Currently only "yolo" is supported
    label_format = "yolo"
    # Minimum number of frames a track must exist before capturing (avoids partially visible objects)
    min_track_age = 15
    # Skip objects whose bounding box touches or is near frame edges
    skip_edge_objects = true
    # Margin in pixels to consider as "edge"
    edge_margin_pixels = 5
    # Maximum number of captures per unique track ID (1 = capture once per vehicle)
    max_captures_per_track = 1
    # Frames between captures for the same track (only used when max_captures_per_track > 1)
    capture_interval = 30
```

### Output format
```
collected_dataset/
├── images/
│   └── 20251212_143052_123456_42.jpg
└── labels/
    └── 20251212_143052_123456_42.txt
```

Label format (YOLO): `class_id center_x center_y width height` (all values normalized 0-1)

Example label file:
```
2 0.453125 0.621528 0.089844 0.156944
7 0.712500 0.534722 0.125000 0.180556
```

### Tips
- Use a strong/accurate model for better pseudo-labels quality
- Set higher `min_track_age` (e.g., 20-30) for cleaner samples
- Review collected data before training - auto-labels may contain errors
- Class IDs in labels correspond to `net_classes` order in `[detection]` section
- You can use https://github.com/LdDl/yolo-ann tool to visualize and manage collected dataset

## Report mode

This utility can generate a ZIP report after processing a video file. Report mode processes the entire video, accumulates all traffic statistics (no periodic resets) and produces a ZIP archive.

**Note:** Report mode is only available for video files, not for RTSP streams or cameras.

When report mode is enabled it automatically disables REST API, MJPEG streaming and periodic statistics reset.

### Configuration
```toml
[report]
    enabled = true
    output_path = "./reports"
```

### Output

ZIP archive `{video_name}_report.zip` in configured `output_path` directory containing:

| File | Description |
|------|-------------|
| `traffic_counts.csv` | Vehicle counts per type per zone |
| `zones.csv` | Zone polygon coordinates (4 vertices) |
| `od_matrix.csv` | Origin-Destination matrix (zone movements) |
| `{video_name}_zones.png` | First frame with zones drawn, IDs and coordinates labeled |

CSV files use semicolon (`;`) as delimiter.

`traffic_counts.csv` example:
```
vehicle_type;zone_id;count
car;dir_0_lane_0;42
truck;dir_0_lane_0;15
```

`zones.csv` example:
```
zone_id;x1;y1;x2;y2;x3;y3;x4;y4
dir_0_lane_0;204;542;398;558;506;325;402;318
```

`od_matrix.csv` example:
```
zone_from;zone_to;count
dir_0_lane_0;dir_0_lane_1;15
dir_0_lane_1;dir_1_lane_0;8
```

## ROADMAP
Please see [this](ROADMAP.md) file
## Support
If you have troubles or questions please [open an issue](https://github.com/LdDl/rust-road-traffic/issues/new).
