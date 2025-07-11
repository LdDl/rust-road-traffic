* Main functionality:
    * ~~Read video~~
    * ~~Initialize neural network~~
    * ~~Extract bounding boxes of detected objects~~
    * ~~Filter detected objects by classname and confidence~~
    * ~~Do non maximum suppression for additional filtering~~
    * ~~Put bounding boxes and classnames onto image after NMS~~
    * ~~Display an output~~
    * ~~Redis (pub to user defined channel + consider password usage)~~
    * ~~Implement SORT + Deep SORT [I guess we need [ndarray](https://github.com/rust-ndarray/ndarray), not [nalgebra](https://github.com/dimforge/nalgebra)] ~~ This moved here: https://github.com/LdDl/mot-rs
  
* Additional functionality
    * ~~Usage of custom implementation (via [nalgebra](https://github.com/dimforge/nalgebra)) of Kalman filter~~
    * ~~Tracking via custom implementation of Kalman filter~~
    * ~~Usage of OpenCV-based Kalman filter~~ *Not needed*
    * ~~Tracking via OpenCV-based Kalman filter~~ *Not needed*
    * ~~Spatial converter~~
        * ~~Tranform matrix~~
        * ~~Convert function~~
    * ~~Speed evaluations~~
        * ~~Haversine function~~
        * ~~Spatial converter usage~~
        * ~~Apply math function to objects~~
    * ~~Read frames in one thread and do neural network's job in another one~~
    * Proper logging?
    * Error handling __W.I.P.__
    * ~~MJPEG streamer (via [actix-web](https://github.com/actix/actix-web#actix-web) I guess?)~~ [it's implemented via mspc and tokio, but I'm not sure if I do threads stuff correctly. MJPEG streaming are laggy currently, need to investigate]
    * gRPC for clients (do we need this?) [I guess it should work as redis publisher, so only client will be implemented?]
    * ~~REST JSON for clients~~ __Still ugly__ 
    * ~~REST Websockets for clients (do we need this?. __No__)~~
    * Installation instructions (Makefile+Ubuntu18-20)
    * ~~TOML configuration~~
    * ~~Convex polygons math~~
        * ~~Check if point is in polygon~~
        * ~~Check if point has entered into polygon~~
        * ~~Check if point has left polygon~~
    * ~~Hashmap and timer for estimating average values of traffic (speed, intensity)~~
        * ~~ Hashmap and thread ~~
        * ~~ Reset values ~~
        * ~~ Time intervals (threads) ~~
        * ~~Refactor~~
    * ~~Optional choice between Tiny YOLO and MobilenetSSD(Caffe)~~[removed]
    * ~~Fill Jetson Nano instructions~~
    * ~~Estimate speed and intensity for each vehicle type~~
    * RabbitMQ/NATS/Tarantool/Kafka integration module? Do we need those?
    * ~~Verbose output as option (fps/workers output)~~
    * Consider color as hex representation (additional field with priority lower than RGBA)
    * ~~Consider training YOLO v4 tiny for vehicle classes only (we do need false positive detection for peoples/boats/chairs and etc.)~~ [I want to provide link for custom trained network: only vehicles + network size is 416x256 rather than 416x416 due the problem when resizing image provided too much squeezing]
    * ~~Organize structure to reduce connectiviy of components of applications~~
    * ~~Consider YOLOX (nano/tiny)~~ [Well, I checked it and it not that good as I was thought]
    * ~~More neural network parameters in TOML ~~
    * ~~Figure it out, how to boost perfomance for YOLOv4-tiny (or mobilenet-ssd?). Best idea I have so far is: pick tensort-rt and onnx and do magick trick.~~ [But there are no good and mature tensor-rt libs for Rust I guess...]
    * ~~Scaling by x/y~~
    * ~~Swagger docs.~~
    

* Some bugs
    * RAM consumption grows up too much.
