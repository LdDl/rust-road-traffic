use opencv::{
    prelude::*,
    core::Mat,
    core::CV_32F,
    core::Point,
    video::KalmanFilter as KF
};

use std::error::Error;
use std::fmt;
#[derive(Debug)]
struct PredictionError {
    details: String
}
impl PredictionError {
    fn new(msg: &str) -> PredictionError {
        PredictionError{details: msg.to_string()}
    }
}
impl fmt::Display for PredictionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.details)
    }
}
impl Error for PredictionError {
    fn description(&self) -> &str {
        &self.details
    }
}

pub struct KalmanWrapper {
    pub model_type: KalmanModelType,
    opencv_kf: KF,
    measurement: Mat
}

#[derive(Copy, Clone)]
pub enum KalmanModelType {
    ConstantVelocity,
    Acceleration
}

impl KalmanWrapper {
    pub fn new(model_type: KalmanModelType) -> Self {
        let tmp = match model_type {
            KalmanModelType::ConstantVelocity => {
                let opencv_kf = KF::new(4, 2, 0, CV_32F).unwrap();
                let mut kw = KalmanWrapper{
                    model_type: KalmanModelType::ConstantVelocity,
                    opencv_kf: opencv_kf,
                    measurement: Mat::zeros(2, 1, CV_32F).unwrap().to_mat().unwrap()
                };

                // println!("Kalman filter parameters for constant velocity model:");

                // Transition matrix 'A'
                let transition_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 0.0, 1.0, 0.0],
                    vec![0.0, 1.0, 0.0, 1.0],
                    vec![0.0, 0.0, 1.0, 0.0],
                    vec![0.0, 0.0, 0.0, 1.0],
                ];
                let transition_matrix = Mat::from_slice_2d(&transition_matrix_data).unwrap();
                kw.opencv_kf.set_transition_matrix(transition_matrix);
                // println!("\tTransition matrix 'A' {:?}", kw.opencv_kf.transition_matrix().data_typed::<f32>().unwrap());
                
                // Measurement matrix 'H'
                let measurement_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 0.0, 0.0, 0.0],
                    vec![0.0, 1.0, 0.0, 0.0],
                ];
                let measurement_matrix = Mat::from_slice_2d(&measurement_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_matrix(measurement_matrix);
                // println!("\tMeasurement matrix 'H' {:?}", kw.opencv_kf.measurement_matrix().data_typed::<f32>().unwrap());

                // Noise covariance matrix 'P'
                let noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 0.0, 0.0, 0.0],
                    vec![0.0, 1.0, 0.0, 0.0],
                    vec![0.0, 0.0, 1.0, 0.0],
                    vec![0.0, 0.0, 0.0, 1.0],
                ];
                // let noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                //     vec![10e5, 0.0,  0.0,  0.0],
                //     vec![0.0,  10e5, 0.0,  0.0],
                //     vec![0.0,  0.0,  10e5, 0.0],
                //     vec![0.0,  0.0,  0.0,  10e5],
                // ];
                let noise_covariance_matrix = Mat::from_slice_2d(&noise_covariance_matrix_data).unwrap();
                kw.opencv_kf.set_error_cov_post(noise_covariance_matrix);
                // println!("\tNoise covariance matrix 'P' {:?}", kw.opencv_kf.error_cov_post().data_typed::<f32>().unwrap());
                
                // Covariance matrix 'Q'
                let covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![25.0, 0.0,  0.0,  0.0],
                    vec![0.0,  25.0, 0.0,  0.0],
                    vec![0.0,  0.0,  10.0, 0.0],
                    vec![0.0,  0.0,  0.0,  10.0],
                ];
                let covariance_matrix = Mat::from_slice_2d(&covariance_matrix_data).unwrap();
                kw.opencv_kf.set_process_noise_cov(covariance_matrix);
                // println!("\tCovariance matrix 'Q' {:?}", kw.opencv_kf.process_noise_cov().data_typed::<f32>().unwrap());

                // Measurement noise covariance matrix 'P'
                let measurement_noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![0.0, 0.0],
                    vec![0.0,  0.0],
                ];
                let measurement_noise_covariance_matrix = Mat::from_slice_2d(&measurement_noise_covariance_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_noise_cov(measurement_noise_covariance_matrix);
                // println!("\tMeasurement matrix 'R' {:?}", kw.opencv_kf.measurement_noise_cov().data_typed::<f32>().unwrap());

                kw
            },
            KalmanModelType::Acceleration => {
                let opencv_kf = KF::new(6, 2, 0, CV_32F).unwrap();
                let mut kw = KalmanWrapper{
                    model_type: KalmanModelType::Acceleration,
                    opencv_kf: opencv_kf,
                    measurement: Mat::zeros(2, 1, CV_32F).unwrap().to_mat().unwrap()
                };

                // println!("Kalman filter parameters for acceleration model:");

                // Transition matrix 'A'
                let transition_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 0.0, 1.0, 0.0, 0.5, 0.0],
                    vec![0.0, 1.0, 0.0, 1.0, 0.0, 0.5],
                    vec![0.0, 0.0, 1.0, 0.0, 1.0, 0.0],
                    vec![0.0, 0.0, 0.0, 1.0, 0.0, 1.0],
                    vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                    vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
                ];
                let transition_matrix = Mat::from_slice_2d(&transition_matrix_data).unwrap();
                kw.opencv_kf.set_transition_matrix(transition_matrix);
                // println!("\tTransition matrix 'A' {:?}", kw.opencv_kf.transition_matrix().data_typed::<f32>().unwrap());

                // Measurement matrix 'H'
                let measurement_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                ];
                let measurement_matrix = Mat::from_slice_2d(&measurement_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_matrix(measurement_matrix);
                // println!("\tMeasurement matrix 'H' {:?}", kw.opencv_kf.measurement_matrix().data_typed::<f32>().unwrap());

                // Noise covariance matrix 'P'
                let noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                    vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
                    vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                    vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
                ];
                // let noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                //     vec![10e5, 0.0,  0.0,  0.0,  0.0,  0.0],
                //     vec![0.0,  10e5, 0.0,  0.0,  0.0,  0.0],
                //     vec![0.0,  0.0,  10e5, 0.0,  0.0,  0.0],
                //     vec![0.0,  0.0,  0.0,  10e5, 0.0,  0.0],
                //     vec![0.0,  0.0,  0.0,  0.0,  10e5, 0.0],
                //     vec![0.0,  0.0,  0.0,  0.0,  0.0,  10e5],
                // ];
                let noise_covariance_matrix = Mat::from_slice_2d(&noise_covariance_matrix_data).unwrap();
                kw.opencv_kf.set_error_cov_post(noise_covariance_matrix);
                // println!("\tNoise covariance matrix 'P' {:?}", kw.opencv_kf.error_cov_post().data_typed::<f32>().unwrap());
                
                // Covariance matrix 'Q'
                let covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![25.0, 0.0,  0.0,  0.0,  0.0, 0.0],
                    vec![0.0,  25.0, 0.0,  0.0,  0.0, 0.0],
                    vec![0.0,  0.0,  10.0, 0.0,  0.0, 0.0],
                    vec![0.0,  0.0,  0.0,  10.0, 0.0, 0.0],
                    vec![0.0,  0.0,  0.0,  0.0,  1.0, 0.0],
                    vec![0.0,  0.0,  0.0,  0.0,  0.0, 1.0],
                ];
                let covariance_matrix = Mat::from_slice_2d(&covariance_matrix_data).unwrap();
                kw.opencv_kf.set_process_noise_cov(covariance_matrix);
                // println!("\tCovariance matrix 'Q' {:?}", kw.opencv_kf.process_noise_cov().data_typed::<f32>().unwrap());

                // Measurement noise covariance matrix 'P'
                let measurement_noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![25.0, 0.0],
                    vec![0.0,  25.0],
                ];
                let measurement_noise_covariance_matrix = Mat::from_slice_2d(&measurement_noise_covariance_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_noise_cov(measurement_noise_covariance_matrix);
                // println!("\tMeasurement matrix 'R' {:?}", kw.opencv_kf.measurement_noise_cov().data_typed::<f32>().unwrap());

                kw
            }
        };
        return tmp
    }
    pub fn predict(&mut self) -> Option<Point> {
        // @todo: handle possible errors
        match self.opencv_kf.predict(&Mat::default()) {
            Ok(prediction) => {
                let prediction_point_x = match prediction.at::<f32>(0) {
                    Ok(x) => *x,
                    Err(err) => {
                        panic!("Error prediction X: {:?}", err);
                    }
                };
                let prediction_point_y = match prediction.at::<f32>(1) {
                    Ok(y) => *y,
                    Err(err) => {
                        panic!("Error prediction Y: {:?}", err);
                    }
                };
                let prediction_point = Point::new(prediction_point_x.round() as i32, prediction_point_y.round() as i32);
                return Some(prediction_point)
            },
            Err(err) => {
                panic!("Error prediction: {:?}", err);
            }
        }
    }
    pub fn correct(&mut self, x: f32, y: f32) -> Option<Point> {
        // @todo: handle possible errors
        self.measurement = Mat::from_slice_2d(&vec![vec![x], vec![y]]).unwrap();
        match self.opencv_kf.correct(&self.measurement) {
            Ok(estimated) => {
                let state_point_x = match estimated.at::<f32>(0) {
                    Ok(x) => *x,
                    Err(err) => {
                        panic!("Error correction X: {:?}", err);
                    }
                };
                let state_point_y = match estimated.at::<f32>(1) {
                    Ok(y) => *y,
                    Err(err) => {
                        panic!("Error correction Y: {:?}", err);
                    }
                };
                let state_point = Point::new(state_point_x.round() as i32, state_point_y.round() as i32);
                return Some(state_point)
            },
            Err(err) => {
                panic!("Error correction: {:?}", err);
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_constant_velocity_model() {
        let xs = vec![311, 312, 313, 311, 311, 312, 312, 313, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 311, 311, 311, 311, 311, 310, 311, 311, 311, 310, 310, 308, 307, 308, 308, 308, 307, 307, 307, 308, 307, 307, 307, 307, 307, 308, 307, 309, 306, 307, 306, 307, 308, 306, 306, 306, 305, 307, 307, 307, 306, 306, 306, 307, 307, 308, 307, 307, 308, 307, 306, 308, 309, 309, 309, 309, 308, 309, 309, 309, 308, 311, 311, 307, 311, 307, 313, 311, 307, 311, 311, 306, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312];
        let ys = vec![5, 6, 8, 10, 11, 12, 12, 13, 16, 16, 18, 18, 19, 19, 20, 20, 22, 22, 23, 23, 24, 24, 28, 30, 32, 35, 39, 42, 44, 46, 56, 58, 70, 60, 52, 64, 51, 70, 70, 70, 66, 83, 80, 85, 80, 98, 79, 98, 61, 94, 101, 94, 104, 94, 107, 112, 108, 108, 109, 109, 121, 108, 108, 120, 122, 122, 128, 130, 122, 140, 122, 122, 140, 122, 134, 141, 136, 136, 154, 155, 155, 150, 161, 162, 169, 171, 181, 175, 175, 163, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178];
        let mut kf = KalmanWrapper::new(KalmanModelType::ConstantVelocity);
        for (i, _) in xs.iter().enumerate() {
            println!("Step#{}:", i);
            let x = xs[i];
            let y = ys[i];
            println!("\tpoint {} {}", x, y);
            let predicted = kf.predict();
            println!("\tpredicted {:?}", predicted);
            let state = kf.correct(x as f32, y as f32);
            println!("\tstate {:?}", state);
            // @todo need to make comparison with valid answer
        }
        assert_eq!(1 + 1, 2);
    }
    #[test]
    fn test_acceleration_model() {
        let xs = vec![311, 312, 313, 311, 311, 312, 312, 313, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 311, 311, 311, 311, 311, 310, 311, 311, 311, 310, 310, 308, 307, 308, 308, 308, 307, 307, 307, 308, 307, 307, 307, 307, 307, 308, 307, 309, 306, 307, 306, 307, 308, 306, 306, 306, 305, 307, 307, 307, 306, 306, 306, 307, 307, 308, 307, 307, 308, 307, 306, 308, 309, 309, 309, 309, 308, 309, 309, 309, 308, 311, 311, 307, 311, 307, 313, 311, 307, 311, 311, 306, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312];
        let ys = vec![5, 6, 8, 10, 11, 12, 12, 13, 16, 16, 18, 18, 19, 19, 20, 20, 22, 22, 23, 23, 24, 24, 28, 30, 32, 35, 39, 42, 44, 46, 56, 58, 70, 60, 52, 64, 51, 70, 70, 70, 66, 83, 80, 85, 80, 98, 79, 98, 61, 94, 101, 94, 104, 94, 107, 112, 108, 108, 109, 109, 121, 108, 108, 120, 122, 122, 128, 130, 122, 140, 122, 122, 140, 122, 134, 141, 136, 136, 154, 155, 155, 150, 161, 162, 169, 171, 181, 175, 175, 163, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178];
        let mut kf = KalmanWrapper::new(KalmanModelType::Acceleration);
        for (i, _) in xs.iter().enumerate() {
            println!("Step#{}:", i);
            let x = xs[i];
            let y = ys[i];
            println!("\tpoint {} {}", x, y);
            let predicted = kf.predict();
            println!("\tpredicted {:?}", predicted);
            let state = kf.correct(x as f32, y as f32);
            println!("\tstate {:?}", state);
            // @todo need to make comparison with valid answer
        }
        assert_eq!(1 + 1, 2);
    }
}