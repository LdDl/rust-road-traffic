use opencv::{
    prelude::*,
    core::Mat,
    core::CV_32F,
    core::Point,
    video::KalmanFilter as KF,
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


use nalgebra;

type Matrix4x4f32 = nalgebra::SMatrix<f32, 4, 4>;
type Matrix2x4f32 = nalgebra::SMatrix<f32, 2, 4>;
type Matrix2x2f32 = nalgebra::SMatrix<f32, 2, 2>;
type Matrix4x1f32 = nalgebra::SMatrix<f32, 4, 1>;
type Matrix2x1f32 = nalgebra::SMatrix<f32, 2, 1>;

const diag_ones: Matrix4x4f32 = Matrix4x4f32::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 1.0
);

pub struct KalmanFilterLinear {
    a: Matrix4x4f32,
	b: Matrix4x4f32,
	c: Matrix2x4f32,
	p: Matrix4x4f32,
	q: Matrix4x4f32,
	r: Matrix2x2f32,
	x: Matrix4x1f32
}

impl KalmanFilterLinear {
    pub fn new() -> Self {
        return KalmanFilterLinear {
            // Transition state matrix
            a: Matrix4x4f32::new(
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0
            ),
            // Control input
            b: Matrix4x4f32::new(
                0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0
            ),
            // Measure matrix
            c: Matrix2x4f32::new(
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0
            ),
            // State covariance
            p: Matrix4x4f32::new(
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0
            ),
            // Process covariance
            q: Matrix4x4f32::new(
                1e-5, 0.0,  0.0,  0.0,
                0.0,  1e-5, 0.0,  0.0,
                0.0,  0.0,  1e-5, 0.0,
                0.0,  0.0,  0.0,  1e-5
            ),
            // Measurement covariance
            r: Matrix2x2f32::new(
                1e-1, 0.0,
                0.0,  1e-1
            ),
            // State (initial indeed)
            x: Matrix4x1f32::new(
                0.0,
                0.0,
                0.0,
                0.0
            )
        }
    }
    pub fn step(&mut self, u: Matrix4x1f32, y: Matrix2x1f32) -> Matrix4x1f32 {
        self.predict(u);
        self.update(y);
        return self.x;
    }
    fn predict(&mut self, u: Matrix4x1f32) {
        // Evaluate x:
	    // x = A ⋅ x + b ⋅ u
        self.x = (self.a * self.x) + (self.b * u);
        // Evaluate state covariance as:
	    // p = A ⋅ p ⋅ Transponse(a) + q
        self.p = ((self.a * self.p) * self.a.transpose()) + self.q;
    }
    fn update(&mut self, y: Matrix2x1f32) {
        // Temporary result of
	    // tmpPC = p ⋅ Transponse(c)
        let tmp_pc = self.p * self.c.transpose();
        // K = tmpPC ⋅ [((c ⋅ tmpPC)  + r)^-1]
	    // p.s. "^-1" - stands for inverse matrix
        let tmp_inversed = ((self.c * tmp_pc) + self.r).try_inverse().unwrap();
        let k = tmp_pc * tmp_inversed;
        // Update state as:
	    // x{k} = x{k-1} + K ⋅ (y - c ⋅ x{k-1})
        self.x = self.x + (k * (y - (self.c * self.x)));
        // Update state covariance as:
	    // p{k} = (Diag(4, 1) - K ⋅ c) ⋅ p{k-1}
        let kc = k * self.c;
        let diagonal = diag_ones - kc;
        self.p = diagonal * self.p;
    }
    pub fn set_state_value(&mut self, x: f32, y: f32, vx: f32, vy: f32) {
        self.x[(0, 0)] = x;
        self.x[(1, 0)] = y;
        self.x[(2, 0)] = vx;
        self.x[(3, 0)] = vy;
    }
    pub fn set_time(&mut self, dt: f32) {
        self.a[(0, 2)] = dt;
        self.a[(1, 3)] = dt;
    }
}

#[cfg(test)]
mod tests {
    use opencv::{
        core::CV_8UC3,
        imgproc::line,
        imgproc::circle,
        core::Scalar,
        imgproc::LINE_8,
        highgui
    };
    use super::*;
    #[test]
    fn test_custom_linear_kalman() {
        let xs = vec![311, 312, 313, 311, 311, 312, 312, 313, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 311, 311, 311, 311, 311, 310, 311, 311, 311, 310, 310, 308, 307, 308, 308, 308, 307, 307, 307, 308, 307, 307, 307, 307, 307, 308, 307, 309, 306, 307, 306, 307, 308, 306, 306, 306, 305, 307, 307, 307, 306, 306, 306, 307, 307, 308, 307, 307, 308, 307, 306, 308, 309, 309, 309, 309, 308, 309, 309, 309, 308, 311, 311, 307, 311, 307, 313, 311, 307, 311, 311, 306, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312];
        let ys = vec![5, 6, 8, 10, 11, 12, 12, 13, 16, 16, 18, 18, 19, 19, 20, 20, 22, 22, 23, 23, 24, 24, 28, 30, 32, 35, 39, 42, 44, 46, 56, 58, 70, 60, 52, 64, 51, 70, 70, 70, 66, 83, 80, 85, 80, 98, 79, 98, 61, 94, 101, 94, 104, 94, 107, 112, 108, 108, 109, 109, 121, 108, 108, 120, 122, 122, 128, 130, 122, 140, 122, 122, 140, 122, 134, 141, 136, 136, 154, 155, 155, 150, 161, 162, 169, 171, 181, 175, 175, 163, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178];
        
        let correct_xs = vec![311, 312, 311, 311, 311, 311, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 311, 311, 311, 311, 311, 311, 310, 310, 310, 310, 310, 310, 309, 309, 308, 308, 308, 307, 307, 307, 307, 307, 306, 306, 306, 306, 306, 306, 306, 306, 306, 306, 306, 306, 306, 305, 305, 305, 305, 305, 305, 305, 305, 305, 305, 306, 306, 306, 306, 306, 306, 306, 306, 307, 307, 307, 307, 308, 308, 308, 308, 308, 309, 309, 309, 309, 309, 310, 309, 310, 310, 309, 310, 310, 310, 311, 311, 311, 311, 311, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312];
        let correct_ys = vec![5, 7, 9, 11, 12, 12, 13, 15, 16, 17, 18, 19, 20, 20, 21, 22, 23, 23, 24, 25, 25, 26, 28, 29, 31, 33, 35, 37, 40, 43, 46, 51, 54, 55, 58, 58, 61, 64, 67, 68, 72, 75, 78, 80, 84, 85, 89, 87, 90, 93, 95, 98, 99, 102, 105, 108, 110, 111, 113, 116, 117, 117, 119, 121, 123, 125, 128, 129, 132, 132, 133, 135, 135, 136, 138, 139, 140, 143, 146, 149, 151, 154, 156, 160, 163, 167, 170, 173, 173, 176, 178, 180, 182, 183, 184, 185, 186, 186, 187, 187, 187, 187, 187, 187, 187, 186, 186, 186, 185, 185, 185];

        assert_eq!(xs.len(), ys.len());
        assert_eq!(xs.len(), correct_xs.len() + 1);
        assert_eq!(xs.len(), correct_ys.len() + 1);
        assert_eq!(ys.len(), correct_xs.len() + 1);
        assert_eq!(ys.len(), correct_ys.len() + 1);
        assert_eq!(correct_xs.len(), correct_ys.len());

        let dt = 1.0;
        let mut frame = Mat::zeros(640, 360, CV_8UC3).unwrap().to_mat().unwrap();
        let mut tracker = KalmanFilterLinear::new();
        tracker.set_state_value(xs[0] as f32, ys[0] as f32, 0.0, 0.0);

        for tm in 1..xs.len() {
            let xt = xs[tm] as f32;
            let yt = ys[tm] as f32;
            let y = Matrix2x1f32::new(
                xt,
                yt
            );
            tracker.set_time(dt);
            let u = Matrix4x1f32::new(
                0.0,
                0.0,
                0.0,
                0.0,
            );
            let state = tracker.step(u, y);
            let kalman_x = state[(0, 0)];
            let kalman_y = state[(1, 0)];

            let correct_kalman_x = correct_xs[tm-1];
            let correct_kalman_y = correct_ys[tm-1];
            assert_eq!(correct_kalman_x, kalman_x as i32);
            assert_eq!(correct_kalman_y, kalman_y as i32);

            match circle(&mut frame, Point::new(xs[tm], ys[tm]), 1, Scalar::from((0.0, 0.0, 255.0)), 1, LINE_8, 0) {
                Ok(_) => {},
                Err(err) => {
                    panic!("Can't draw circle at blob's predicted position due the error: {:?}", err)
                }
            };
            match circle(&mut frame, Point::new(kalman_x as i32, kalman_y as i32), 1, Scalar::from((255.0, 0.0, 0.0)), 1, LINE_8, 0) {
                Ok(_) => {},
                Err(err) => {
                    panic!("Can't draw circle at blob's predicted position due the error: {:?}", err)
                }
            };
        };
        
        // let window = "test_constant_velocity_model()";
        // match highgui::named_window(window, 1) {
        //     Ok(_) => {},
        //     Err(err) =>{
        //         panic!("Can't give a name to output window due the error: {:?}", err)
        //     }
        // };
        // loop {
        //     highgui::imshow(window, &mut frame).unwrap();
        //     let key = highgui::wait_key(10).unwrap();
        //     if key > 0 && key != 255 {
        //         break;
        //     }
        // }
        // assert_eq!(1 + 1, 3);
    }
}