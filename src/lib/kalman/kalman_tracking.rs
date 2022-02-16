use opencv::{
    prelude::*,
    core::Mat,
    core::CV_32F,
    core::Point,
    video::KalmanFilter as KF,
};

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
    pub fn new(model_type: KalmanModelType, init_x: f32, init_y: f32, init_vx: f32, init_vy: f32) -> Self {
        let tmp = match model_type {
            KalmanModelType::ConstantVelocity => {
                let opencv_kf = KF::new(4, 2, 0, CV_32F).unwrap();
                let mut kw = KalmanWrapper{
                    model_type: KalmanModelType::ConstantVelocity,
                    opencv_kf: opencv_kf,
                    measurement: Mat::zeros(2, 1, CV_32F).unwrap().to_mat().unwrap()
                };

                // println!("Kalman filter parameters for constant velocity model:");

                // Set initial data as corrected state
                let state_post_data: Vec<Vec<f32>> = vec![
                    vec![init_x],
                    vec![init_y],
                    vec![init_vx],
                    vec![init_vy],
                ]; 
                let state_post = Mat::from_slice_2d(&state_post_data).unwrap();
                kw.opencv_kf.set_state_post(state_post);
                // println!("\tstatePost {:?}", kw.opencv_kf.transition_matrix().data_typed::<f32>().unwrap());

                // Transition matrix 'a'
                let transition_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 0.0, 1.0, 0.0],
                    vec![0.0, 1.0, 0.0, 1.0],
                    vec![0.0, 0.0, 1.0, 0.0],
                    vec![0.0, 0.0, 0.0, 1.0],
                ];
                let transition_matrix = Mat::from_slice_2d(&transition_matrix_data).unwrap();
                kw.opencv_kf.set_transition_matrix(transition_matrix);
                // println!("\tTransition matrix 'a' {:?}", kw.opencv_kf.transition_matrix().data_typed::<f32>().unwrap());
                
                // Measurement matrix 'H'
                let measurement_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 0.0, 0.0, 0.0],
                    vec![0.0, 1.0, 0.0, 0.0],
                ];
                let measurement_matrix = Mat::from_slice_2d(&measurement_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_matrix(measurement_matrix);
                // println!("\tMeasurement matrix 'H' {:?}", kw.opencv_kf.measurement_matrix().data_typed::<f32>().unwrap());

                // Noise covariance matrix 'p'
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
                // println!("\tNoise covariance matrix 'p' {:?}", kw.opencv_kf.error_cov_post().data_typed::<f32>().unwrap());
                
                // Covariance matrix 'q'
                let covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![25.0, 0.0,  0.0,  0.0],
                    vec![0.0,  25.0, 0.0,  0.0],
                    vec![0.0,  0.0,  10.0, 0.0],
                    vec![0.0,  0.0,  0.0,  10.0],
                ];
                let covariance_matrix = Mat::from_slice_2d(&covariance_matrix_data).unwrap();
                kw.opencv_kf.set_process_noise_cov(covariance_matrix);
                // println!("\tCovariance matrix 'q' {:?}", kw.opencv_kf.process_noise_cov().data_typed::<f32>().unwrap());

                // Measurement noise covariance matrix 'p'
                let measurement_noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![0.0, 0.0],
                    vec![0.0,  0.0],
                ];
                let measurement_noise_covariance_matrix = Mat::from_slice_2d(&measurement_noise_covariance_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_noise_cov(measurement_noise_covariance_matrix);
                // println!("\tMeasurement matrix 'r' {:?}", kw.opencv_kf.measurement_noise_cov().data_typed::<f32>().unwrap());

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

                // Set initial data as corrected state
                let state_post_data: Vec<Vec<f32>> = vec![
                    vec![init_x],
                    vec![init_y],
                    vec![init_vx],
                    vec![init_vy],
                    vec![0.0], // Assume that initial acceleration a{x} = 0
                    vec![0.0], // Assume that initial acceleration a{y} = 0
                ]; 
                let state_post = Mat::from_slice_2d(&state_post_data).unwrap();
                kw.opencv_kf.set_state_post(state_post);
                // println!("\tstatePost {:?}", kw.opencv_kf.transition_matrix().data_typed::<f32>().unwrap());

                // Transition matrix 'a'
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
                // println!("\tTransition matrix 'a' {:?}", kw.opencv_kf.transition_matrix().data_typed::<f32>().unwrap());

                // Measurement matrix 'H'
                let measurement_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                ];
                let measurement_matrix = Mat::from_slice_2d(&measurement_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_matrix(measurement_matrix);
                // println!("\tMeasurement matrix 'H' {:?}", kw.opencv_kf.measurement_matrix().data_typed::<f32>().unwrap());

                // Noise covariance matrix 'p'
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
                // println!("\tNoise covariance matrix 'p' {:?}", kw.opencv_kf.error_cov_post().data_typed::<f32>().unwrap());
                
                // Covariance matrix 'q'
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
                // println!("\tCovariance matrix 'q' {:?}", kw.opencv_kf.process_noise_cov().data_typed::<f32>().unwrap());

                // Measurement noise covariance matrix 'p'
                let measurement_noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![25.0, 0.0],
                    vec![0.0,  25.0],
                ];
                let measurement_noise_covariance_matrix = Mat::from_slice_2d(&measurement_noise_covariance_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_noise_cov(measurement_noise_covariance_matrix);
                // println!("\tMeasurement matrix 'r' {:?}", kw.opencv_kf.measurement_noise_cov().data_typed::<f32>().unwrap());

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
                        panic!("Error prediction x: {:?}", err);
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
                        panic!("Error correction x: {:?}", err);
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