* Main functionality:
    * ~~Read video~~
    * ~~Initialize neural network~~
    * ~~Extract bounding boxes of detected objects~~
    * ~~Filter detected objects by classname and confidence~~
    * ~~Do non maximum suppression for additional filtering~~
    * ~~Put bounding boxes and classnames onto image after NMS~~
    * ~~Display an output~~

* Additional functionality
    * ~~Usage of custom implementation (via [nalgebra](https://github.com/dimforge/nalgebra)) of Kalman filter~~
    * ~~Tracking via custom implementation of Kalman filter~~
    * Usage of OpenCV-based Kalman filter *Need help to figure it out. PR's are welcome* 
    * Tracking via OpenCV-based Kalman filter *PR's are welcome: for both constant velocity model and acceleration model*
    * ~~Spatial converter~~
        * ~~Tranform matrix~~
        * ~~Convert function~~
    * ~~Speed evaluations~~
        * ~~Haversine function~~
        * ~~Spatial converter usage~~
        * ~~Apply math function to objects~~
    * Read frames in one thread and do neural network's job in another one __W.I.P.__
    * Error handling __W.I.P.__
    * MJPEG streamer (via [actix-web](https://github.com/actix/actix-web#actix-web) I guess?)
    * gRPC for clients (do we need this?)
    * REST JSON for clients __W.I.P. Still ugly__ 
    * REST Websockets for clients (do we need this?)
    * Installation instructions (Makefile+Ubuntu18-20)
    * TOML configuration __W.I.P.__
    * ~~Convex polygons math~~
        * ~~Check if point is in polygon~~
        * ~~Check if point has entered into polygon~~
        * ~~Check if point has left polygon~~
    * Hashmap and timer for estimating average values of traffic (speed, intensity) __W.I.P.__
        * Hashmap and thread __W.I.P.__
        * Reset values __W.I.P.__
        * Time intervals (threads) __W.I.P.__ (
    * ~~Optional choice between Tiny YOLO and MobilenetSSD(Caffe)~~
    * ~~Fill Jetson Nano instructions~~
    * ~~Estimate speed and intensity for each vehicle type~~
