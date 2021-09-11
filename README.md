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
3. YOLO v4 (its tiny version) - https://arxiv.org/abs/2004.10934

## Installation and usage
1. You need installed Rust compiler obviously. Follow instruction of official site: https://www.rust-lang.org/tools/install
2. You need installed OpenCV and its contributors modules. I'm using OpenCV 4.5.3. Here is [Makefile](Makefile) if you want build it from sources.
3. OpenCV's bindings have already meant as dependencies in [Cargo.toml](Cargo.toml)
4. Clone the repo
    ```shell
    git clone https://github.com/LdDl/rust-road-traffic.git
    ```
    Well, actually I provide yolov4-tiny configuration and weights file from [official repository](https://github.com/AlexeyAB/darknet), but you are free to user yours.
    I provide video file as sample also.
    If you want to change those you can navigate to source code:
    ```rust
    const COCO_CLASSNAMES: &'static [&'static str] = &[/*place whatever classnames your network can handle*/]
    const COCO_FILTERED_CLASSNAMES: &'static [&'static str] = &[/*place whatever classnames you want to filter*/]
    let video_src = "./data/sample_960_540.mp4";
    let weights_src = "./data/yolov4-tiny.weights";
    let cfg_src = "./data/yolov4-tiny.cfg";
    ```
5. Run
    ```shell
    cargo run
    ```
    If you want to use some Rust's optimizations then call build and run
    ```shell
    cargo build --release && ./target/release/rust-road-traffic
    ```

## Screenshots
* imshow() output:

    <img src="data/tiny-yolov4-example-output-1.jpeg" width="480">

# ROADMAP
* Main functionality:
    * ~~Read video~~
    * ~~Initialize neural network~~
    * ~~Extract bounding boxes of detected objects~~
    * ~~Filter detected objects by classname and confidence~~
    * ~~Do non maximum suppression for additional filtering~~
    * ~~Put bounding boxes and classnames onto image after NMS~~
    * ~~Display an output~~

* Additional functionality
    * Kalman filtering
    * Tracking
    * Spatial converter
    * Speed evaluations
    

## Support
If you have troubles or questions please [open an issue](https://github.com/LdDl/rust-road-traffic/issues/new).