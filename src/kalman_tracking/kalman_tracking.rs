use opencv::{
    prelude::*,
    core::CV_32F,
    video::KalmanFilter as KF
};

pub struct KalmanWrapper {
    pub model_type: KalmanModelType,
    opencv_kf: KF
}

pub enum KalmanModelType {
    ConstantVelocity,
    Acceleration
}

impl KalmanWrapper {
    fn new(model_type: KalmanModelType, init_x: f32, init_y: f32) -> Self {
        let tmp = match model_type {
            ConstantVelocity => {
                let opencv_kf = KF::new(4, 2, 0, CV_32F).unwrap();
                KalmanWrapper{
                    model_type: ConstantVelocity,
                    opencv_kf: opencv_kf
                }
            },
            Acceleration => {
                let opencv_kf = KF::new(6, 2, 0, CV_32F).unwrap();
                KalmanWrapper{
                    model_type: Acceleration,
                    opencv_kf: opencv_kf
                }
            }
        };
        return tmp
    }
}