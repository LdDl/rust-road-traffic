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
    * Speed evaluations __W.I.P.__
        * ~~Haversine function~~
        * Spatial converter usage
    * Read frames in one thread and do neural network's job in another one __W.I.P.__
    * Error handling __W.I.P.__
    * MJPEG streamer (via [actix-web](https://github.com/actix/actix-web#actix-web) I guess?)
    * gRPC or REST (websockets) for clients
    * Installation instructions (Makefile+Ubuntu18-20)
    * TOML configuration __W.I.P.__
    * Convex polygons math __W.I.P.__
        * ~~Check if point is in polygon~~
        * Check if point has entered into polygon __W.I.P.__
        * Check if point has left polygon __W.I.P.__
    * ~~Optional choice between Tiny YOLO and MobilenetSSD(Caffe)~~