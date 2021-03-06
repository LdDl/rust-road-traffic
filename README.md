# W.I.P
# Rust toy utility for monitoring road traffic

## Table of Contents
- [W.I.P](#wip)
- [Rust toy utility for monitoring road traffic](#rust-toy-utility-for-monitoring-road-traffic)
  - [Table of Contents](#table-of-contents)
  - [About](#about)
  - [Installation and usage](#installation-and-usage)
  - [Screenshots](#screenshots)
- [ROADMAP](#roadmap)
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
    
    If you want to change parameters of this utility then navigate to [configuration file](data/conf.toml)

4. Download weights (optional)
   In this section I'd like to provide YOLOv4-tiny trained on classic COCO dataset for 'vehicle'-based classes only, such as: car, motorbike, bus, train and truck (COCO dataset provides only them). There is also a little modification if configuration: I've changed size of an input. So it's not a classic 416x416, but 416x256: so 'vehicles' objects wouldn't be squeezed too much.
   Navigate to [data](/data) folder and run script
   ```shell
   cd ./data
   ./download_yolo_v4_only_vehicles.sh
   ```
    If case downloading from Google drive not working (it could be since Google can change anything anytime):
    Weights - [link](https://drive.google.com/file/d/1_NNRyXO1r-FjDmJ_q9bqo_2TpVsK0n13/view?usp=sharing)
    Configuration - [link](https://drive.google.com/file/d/10L8mfn8oGLZJmqSxNtGg42bYD0QCkQAv/view?usp=sharing)

    __You can skip this step if you want to use default YOLOv4-tiny weights and configuration. Just make sure to prepare [configuration file](/data/conf.toml) correctly__

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