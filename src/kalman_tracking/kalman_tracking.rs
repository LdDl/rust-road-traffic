use opencv::{
    prelude::*,
    core::Mat,
    core::CV_32F,
    video::KalmanFilter as KF
};

pub struct KalmanWrapper {
    pub model_type: KalmanModelType,
    opencv_kf: KF,
    measurement: Mat
}

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

                println!("Kalman filter parameters for constant velocity model:");

                // Transition matrix 'A'
                let transition_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 1.0, 0.0, 0.0],
                    vec![0.0, 1.0, 0.0, 0.0],
                    vec![0.0, 0.0, 1.0, 1.0],
                    vec![0.0, 0.0, 0.0, 1.0],
                ];
                let transition_matrix = Mat::from_slice_2d(&transition_matrix_data).unwrap();
                kw.opencv_kf.set_transition_matrix(transition_matrix);
                println!("\tTransition matrix 'A' {:?}", kw.opencv_kf.transition_matrix().data_typed::<f32>().unwrap());
                
                // Measurement matrix 'H'
                let measurement_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 0.0, 0.0, 0.0],
                    vec![0.0, 0.0, 1.0, 0.0],
                ];
                let measurement_matrix = Mat::from_slice_2d(&measurement_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_matrix(measurement_matrix);
                println!("\tMeasurement matrix 'H' {:?}", kw.opencv_kf.measurement_matrix().data_typed::<f32>().unwrap());

                // Noise covariance matrix 'P'
                let noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![10e5, 0.0,  0.0,  0.0],
                    vec![0.0,  10e5, 0.0,  0.0],
                    vec![0.0,  0.0,  10e5, 0.0],
                    vec![0.0,  0.0,  0.0,  10e5],
                ];
                let noise_covariance_matrix = Mat::from_slice_2d(&noise_covariance_matrix_data).unwrap();
                kw.opencv_kf.set_error_cov_post(noise_covariance_matrix);
                println!("\tNoise covariance matrix 'P' {:?}", kw.opencv_kf.error_cov_post().data_typed::<f32>().unwrap());
                
                // Covariance matrix 'Q'
                let covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![25.0, 0.0,  0.0,  0.0],
                    vec![0.0,  10.0, 0.0,  0.0],
                    vec![0.0,  0.0,  25.0, 0.0],
                    vec![0.0,  0.0,  0.0,  10.0],
                ];
                let covariance_matrix = Mat::from_slice_2d(&covariance_matrix_data).unwrap();
                kw.opencv_kf.set_process_noise_cov(covariance_matrix);
                println!("\tCovariance matrix 'Q' {:?}", kw.opencv_kf.process_noise_cov().data_typed::<f32>().unwrap());

                // Measurement noise covariance matrix 'P'
                let measurement_noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![25.0, 0.0],
                    vec![0.0,  25.0],
                ];
                let measurement_noise_covariance_matrix = Mat::from_slice_2d(&measurement_noise_covariance_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_noise_cov(measurement_noise_covariance_matrix);
                println!("\tMeasurement matrix 'R' {:?}", kw.opencv_kf.measurement_noise_cov().data_typed::<f32>().unwrap());

                kw
            },
            KalmanModelType::Acceleration => {
                let opencv_kf = KF::new(6, 2, 0, CV_32F).unwrap();
                let mut kw = KalmanWrapper{
                    model_type: KalmanModelType::Acceleration,
                    opencv_kf: opencv_kf,
                    measurement: Mat::zeros(2, 1, CV_32F).unwrap().to_mat().unwrap()
                };

                println!("Kalman filter parameters for acceleration model:");

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
                println!("\tTransition matrix 'A' {:?}", kw.opencv_kf.transition_matrix().data_typed::<f32>().unwrap());

                // Measurement matrix 'H'
                let measurement_matrix_data: Vec<Vec<f32>> = vec![
                    vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                ];
                let measurement_matrix = Mat::from_slice_2d(&measurement_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_matrix(measurement_matrix);
                println!("\tMeasurement matrix 'H' {:?}", kw.opencv_kf.measurement_matrix().data_typed::<f32>().unwrap());

                // Noise covariance matrix 'P'
                let noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![10e5, 0.0,  0.0,  0.0,  0.0,  0.0],
                    vec![0.0,  10e5, 0.0,  0.0,  0.0,  0.0],
                    vec![0.0,  0.0,  10e5, 0.0,  0.0,  0.0],
                    vec![0.0,  0.0,  0.0,  10e5, 0.0,  0.0],
                    vec![0.0,  0.0,  0.0,  0.0,  10e5, 0.0],
                    vec![0.0,  0.0,  0.0,  0.0,  0.0,  10e5],
                ];
                let noise_covariance_matrix = Mat::from_slice_2d(&noise_covariance_matrix_data).unwrap();
                kw.opencv_kf.set_error_cov_post(noise_covariance_matrix);
                println!("\tNoise covariance matrix 'P' {:?}", kw.opencv_kf.error_cov_post().data_typed::<f32>().unwrap());
                
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
                println!("\tCovariance matrix 'Q' {:?}", kw.opencv_kf.process_noise_cov().data_typed::<f32>().unwrap());

                // Measurement noise covariance matrix 'P'
                let measurement_noise_covariance_matrix_data: Vec<Vec<f32>> = vec![
                    vec![25.0, 0.0],
                    vec![0.0,  25.0],
                ];
                let measurement_noise_covariance_matrix = Mat::from_slice_2d(&measurement_noise_covariance_matrix_data).unwrap();
                kw.opencv_kf.set_measurement_noise_cov(measurement_noise_covariance_matrix);
                println!("\tMeasurement matrix 'R' {:?}", kw.opencv_kf.measurement_noise_cov().data_typed::<f32>().unwrap());

                kw
            }
        };
        return tmp
    }
    pub fn predict(&self) {
        // @todo
    }
    pub fn correct(&self, x: f32, y: f32) {
        // @todo
    }
}