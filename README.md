# W.I.P
# Rust toy utility for monitoring road traffic

## Table of Contents
- [About](#about)
- [Installation and usage](#installation-and-usage)
- [Screenshots](#screenshots)
- [Roadmap](#roadmap)
- [Support](#support)

## About

Vehicle detection/tracking and speed estimation via next instruments:
1. Rust programming language - https://www.rust-lang.org/
2. OpenCV bindings - https://github.com/twistedfall/opencv-rust#rust-opencv-bindings
3. Linear algebra - https://github.com/dimforge/nalgebra
4. YOLO v4 (its tiny version) - https://arxiv.org/abs/2004.10934 | MobilenetSSD - https://github.com/chuanqi305/MobileNet-SSD
5. actix-web for web part - https://actix.rs/

## Installation and usage
1. You need installed Rust compiler obviously. Follow instruction of official site: https://www.rust-lang.org/tools/install
2. You need installed OpenCV and its contributors modules. I'm using OpenCV 4.5.3. I'd highly recommend to use OpenCV with CUDA. Here is [Makefile](Makefile) adopted from [this one](https://github.com/hybridgroup/gocv/blob/release/Makefile) if you want build it from sources (it's targeted for Linux user obviously).
    ```shell
    sudo make install_cuda
    ```

3. OpenCV's bindings have already meant as dependencies in [Cargo.toml](Cargo.toml)
4. Clone the repo
    ```shell
    git clone https://github.com/LdDl/rust-road-traffic.git
    ```
    Well, actually I provide yolov4-tiny configuration and weights file from [official repository](https://github.com/AlexeyAB/darknet) (authors of YOLOv4), but you are free to use yours.
    I provide video file as sample also.
    
    If you want to change parameters of this utility then navigate to [configuration file](data/conf.toml):
    ```toml
    [input]
        video_src = "./data/sample_960_540.mp4"
        # Use string below for usage with CSI camera (where sensor-id is camera indentifier)
        # video_src = "nvarguscamerasrc sensor-id=0 ! video/x-raw(memory:NVMM), width=(int)1280, height=(int)720, format=(string)NV12, framerate=(fraction)30/1 ! nvvidconv flip-method=0 ! video/x-raw, width=(int)1280, height=(int)720, format=(string)BGRx ! videoconvert ! video/x-raw, format=(string)BGR ! appsink"
        # Two options: rtsp / any number corresponding to local camera
        typ = "rtsp"
        # typ = "local"
    [output]
        enable = true
        width = 500
        height = 500
        window_name = "Toy GUI"

    [detection]
        # *.weight/*.cfg + "Darknet" for YOLO
        network_weights = "./data/yolov4-tiny.weights"
        network_cfg = "./data/yolov4-tiny.cfg"
        network_type = "Darknet"
        # *.prototxt/*.caffemodel + "Caffe-MobileNet-SSD" for Caffe
        # network_weights = "./data/MobileNetSSD_deploy.prototxt"
        # network_cfg = "./data/MobileNetSSD_deploy.caffemodel"
        # network_type = "Caffe-MobileNet-SSD"
        conf_threshold = 0.25
        nms_threshold = 0.3

    [tracking]
        max_points_in_track = 100

    [equipment_info]
        # Just field for future identification of application. Could be any string. I've used https://www.uuidgenerator.net/version4 for ID generation
        id = "1e23985f-1fa3-45d0-a365-2d8525a23ddd"

    [[road_lanes]]
        lane_number = 0
        lane_direction = 0
        # left-bot, right-bot, right-top, left-top
        geometry = [[51, 266], [281, 264], [334, 80], [179, 68]]
        geometry_wgs84 = [[37.61891987174749, 54.20564462974709], [37.618926241993904, 54.20564482584264], [37.61894233524799, 54.205666592443166], [37.61893227696419, 54.205668161206724]]
        color_rgb = [255, 0, 0]
    [[road_lanes]]
        lane_number = 1
        lane_direction = 0
        # left-bot, right-bot, right-top, left-top
        geometry = [[315, 267], [572, 265], [547, 66], [359, 69]]
        geometry_wgs84 = [[37.618908137083054, 54.20564619851147], [37.61891517788172, 54.20564502193819], [37.618927247822285, 54.205668749493036], [37.61892020702362, 54.2056701221611]]
        color_rgb = [0, 255, 0]
    [[road_lanes]]
        lane_number = 2
        lane_direction = 0
        # left-bot, right-bot, right-top, left-top
        geometry = [[604, 267], [885, 267], [746, 58], [575, 68]]
        geometry_wgs84 = [[37.61890344321728, 54.205646982893654], [37.61891350150108, 54.20566796511128], [37.61890981346368, 54.20566972997024], [37.61890009045601, 54.20564835556243]]
        color_rgb = [0, 0, 255]

    [worker]
        reset_data_milliseconds = 30000

    [rest_api]
        enable = true
        host = "0.0.0.0"
        back_end_port = 42001
        api_scope = "/api"

    [redis_publisher]
        enable = true
        host = "localhost"
        port = 6379
        password = ""
        db_index = 0
        channel_name = "DETECTORS_STATISTICS"
    ```
5. Run
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
    If you want to do some REST calls you can do following (based on *rest_api* field in TOML configuration files)
    ```bash
    # Get polygons (GeoJSON) in which road traffic monitoring is requested
    curl -XGET 'http://localhost:42001/api/polygons/geojson'
    # Get statistics info for each polygon and each vehicle type in that polygon
    curl -XGET 'http://localhost:42001/api/stats/all'
    ```

## Screenshots
* imshow() output:

    <img src="data/tiny-yolov4-example-output-1.jpeg" width="320"> | <img src="data/tiny-yolov4-example-output-2.jpeg" width="320">

# ROADMAP
Please see [this](ROADMAP.md) file
## Support
If you have troubles or questions please [open an issue](https://github.com/LdDl/rust-road-traffic/issues/new).