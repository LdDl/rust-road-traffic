use std::error::Error;
use std::fmt;

#[derive(Debug)]
struct KalmanFilterLinearError(String);
impl fmt::Display for KalmanFilterLinearError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "There is an error: {}", self.0)
    }
}
impl Error for KalmanFilterLinearError {}

use nalgebra;

type Matrix6x6f32 = nalgebra::SMatrix<f32, 6, 6>;
type Matrix4x4f32 = nalgebra::SMatrix<f32, 4, 4>;
type Matrix2x6f32 = nalgebra::SMatrix<f32, 2, 6>;
type Matrix2x4f32 = nalgebra::SMatrix<f32, 2, 4>;
type Matrix2x2f32 = nalgebra::SMatrix<f32, 2, 2>;
type Matrix3x3f32 = nalgebra::SMatrix<f32, 3, 3>;
pub type Matrix4x1f32 = nalgebra::SMatrix<f32, 4, 1>;
pub type Matrix6x1f32 = nalgebra::SMatrix<f32, 6, 1>;
pub type Matrix2x1f32 = nalgebra::SMatrix<f32, 2, 1>;

const DIAG_ONES: Matrix6x6f32 = Matrix6x6f32::new(
    1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
    0.0, 0.0, 0.0, 0.0, 0.0, 1.0
);

pub fn new_transition_matrix(dt: f32) -> Matrix3x3f32 {
    return Matrix3x3f32::new(
        1.0, dt, dt * dt * 0.5,
        0.0, 1.0, dt,
        0.0, 0.0, 1.0
    )
}

pub fn new_process_covariance(dt: f32) -> Matrix3x3f32 {
    return Matrix3x3f32::new(
        f32::powf(dt, 6.0) / 36.0, f32::powf(dt, 5.0) / 24.0, f32::powf(dt, 4.0) / 6.0,
        f32::powf(dt, 5.0) / 24.0, f32::powf(dt, 4.0) / 4.0, f32::powf(dt, 3.0) / 2.0,
        f32::powf(dt, 4.0) / 6.0, f32::powf(dt, 3.0) / 2.0, f32::powf(dt, 2.0)
    )
}

#[derive(Debug)]
pub struct KalmanFilterLinear {
    // Transition matrix
    a: Matrix6x6f32,
    // Control input
	b: Matrix6x6f32,
    // Measurement Matrix
	c: Matrix2x6f32,
    // Prediction (state) covariance
	p: Matrix6x6f32,
    // Process covariance
	q: Matrix6x6f32,
    // Measurement covariance
	r: Matrix2x2f32,
	x: Matrix6x1f32
}

impl KalmanFilterLinear {
    pub fn new() -> Self {
        let dt = 1.0;
        let a = new_transition_matrix(dt);
        let q = new_process_covariance(dt);
        let process_noise_scale = 1.0;
        return KalmanFilterLinear {
            // Transition state matrix
            a: Matrix6x6f32::new(
                a[(0, 0)], a[(0, 1)], a[(0, 2)], 0.0, 0.0, 0.0,
                a[(1, 0)], a[(1, 1)], a[(1, 2)], 0.0, 0.0, 0.0,
                a[(2, 0)], a[(2, 1)], a[(2, 2)], 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, a[(0, 0)], a[(0, 1)], a[(0, 2)],
                0.0, 0.0, 0.0, a[(1, 0)], a[(1, 1)], a[(1, 2)],
                0.0, 0.0, 0.0, a[(2, 0)], a[(2, 1)], a[(2, 2)]
            ),
            // Control input
            b: Matrix6x6f32::new(
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0
            ),
            // Measure matrix
            c: Matrix2x6f32::new(
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 1.0, 0.0, 0.0
            ),
            // Prediction (state) covariance
            p: Matrix6x6f32::new(
                1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
                1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
                1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
                1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
                1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
                1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
            ),
            // Process covariance
            q: Matrix6x6f32::new(
                process_noise_scale * q[(0, 0)], process_noise_scale * q[(0, 1)],  process_noise_scale * q[(0, 2)],  0.0,  0.0,  0.0,
                process_noise_scale * q[(1, 0)], process_noise_scale * q[(1, 1)],  process_noise_scale * q[(1, 2)],  0.0,  0.0,  0.0,
                process_noise_scale * q[(2, 0)], process_noise_scale * q[(2, 1)],  process_noise_scale * q[(2, 2)], 0.0,  0.0,  0.0,
                0.0,  0.0,  0.0,  process_noise_scale * q[(0, 0)], process_noise_scale * q[(0, 1)],  process_noise_scale * q[(0, 2)],
                0.0,  0.0,  0.0,  process_noise_scale * q[(1, 0)], process_noise_scale * q[(1, 1)],  process_noise_scale * q[(1, 2)],
                0.0,  0.0,  0.0,  process_noise_scale * q[(2, 0)], process_noise_scale * q[(2, 1)],  process_noise_scale * q[(2, 2)],
            ),
            // Measurement covariance
            r: Matrix2x2f32::new(
                1.0, 0.0,
                0.0,  1.0
            ),
            // State (initial indeed)
            x: Matrix6x1f32::new(
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0
            )
        }
    }
    pub fn step(&mut self, u: Matrix6x1f32, z: Matrix2x1f32) -> Result<Matrix6x1f32, Box<dyn Error>> {
        self.predict(u);
        self.update(z)?;
        return Ok(self.x);
    }
    pub fn predict(&mut self, u: Matrix6x1f32) {
        self.x = (self.a * self.x) + (self.b * u);
        self.p = ((self.a * self.p) * self.a.transpose()) + self.q;
    }
    pub fn update(&mut self, z: Matrix2x1f32) -> Result<(), Box<dyn Error>> {
        let y = z - self.c * self.x;
        let innovation_covariance = (self.c * (self.p * self.c.transpose())) + self.r;        
        let innovation_covariance_inv = match innovation_covariance.try_inverse() {
            Some(result) => result,
            None => {
                return Err(Box::new(KalmanFilterLinearError("Can't do inversion for innovation covariance".into())));
            }
        };
        let optimal_kalman_gain = (self.p * self.c.transpose()) * innovation_covariance_inv;
        self.x = self.x + optimal_kalman_gain * y;
        let _t1 = DIAG_ONES - (optimal_kalman_gain * self.c);
        let t1 = (_t1 * self.p) * _t1.transpose();
        let t2 = (optimal_kalman_gain * self.r) * optimal_kalman_gain.transpose();
        self.p = t1 + t2;
        return Ok(());
    }
    pub fn set_state_value(&mut self, x: f32, y: f32) {
        self.x[(0, 0)] = x;
        self.x[(1, 0)] = 0.0;
        self.x[(2, 0)] = 0.0;
        self.x[(3, 0)] = y;
        self.x[(4, 0)] = 0.0;
        self.x[(5, 0)] = 0.0;
    }
    pub fn set_time(&mut self, dt: f32) {
        self.a[(0, 2)] = dt;
        self.a[(1, 3)] = dt;
    }
}

pub struct KFTrackerConstantAcceleration {
    time_step: u32,
    process_noise_scale: f32,
    measurement_noise_scale: f32
}

impl KFTrackerConstantAcceleration {
    pub fn default() -> Self {
        let kf = KFTrackerConstantAcceleration{
            time_step: 1,
            process_noise_scale: 1.0,
            measurement_noise_scale: 1.0
        };

        return kf
    }
}

#[cfg(test)]
mod tests {
    use opencv::{
        prelude::*,
        core::Mat,
        core::Point,
        core::CV_8UC3,
        imgproc::line,
        imgproc::circle,
        core::Scalar,
        imgproc::LINE_8,
        highgui
    };
    // #[test]
    // fn test_custom_linear_kalman() {
    //     let xs = vec![311, 312, 313, 311, 311, 312, 312, 313, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 311, 311, 311, 311, 311, 310, 311, 311, 311, 310, 310, 308, 307, 308, 308, 308, 307, 307, 307, 308, 307, 307, 307, 307, 307, 308, 307, 309, 306, 307, 306, 307, 308, 306, 306, 306, 305, 307, 307, 307, 306, 306, 306, 307, 307, 308, 307, 307, 308, 307, 306, 308, 309, 309, 309, 309, 308, 309, 309, 309, 308, 311, 311, 307, 311, 307, 313, 311, 307, 311, 311, 306, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312];
    //     let ys = vec![5, 6, 8, 10, 11, 12, 12, 13, 16, 16, 18, 18, 19, 19, 20, 20, 22, 22, 23, 23, 24, 24, 28, 30, 32, 35, 39, 42, 44, 46, 56, 58, 70, 60, 52, 64, 51, 70, 70, 70, 66, 83, 80, 85, 80, 98, 79, 98, 61, 94, 101, 94, 104, 94, 107, 112, 108, 108, 109, 109, 121, 108, 108, 120, 122, 122, 128, 130, 122, 140, 122, 122, 140, 122, 134, 141, 136, 136, 154, 155, 155, 150, 161, 162, 169, 171, 181, 175, 175, 163, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178, 178];
        
    //     let correct_xs = vec![311, 312, 311, 311, 311, 311, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 311, 311, 311, 311, 311, 311, 310, 310, 310, 310, 310, 310, 309, 309, 308, 308, 308, 307, 307, 307, 307, 307, 306, 306, 306, 306, 306, 306, 306, 306, 306, 306, 306, 306, 306, 305, 305, 305, 305, 305, 305, 305, 305, 305, 305, 306, 306, 306, 306, 306, 306, 306, 306, 307, 307, 307, 307, 308, 308, 308, 308, 308, 309, 309, 309, 309, 309, 310, 309, 310, 310, 309, 310, 310, 310, 311, 311, 311, 311, 311, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312, 312];
    //     let correct_ys = vec![5, 7, 9, 11, 12, 12, 13, 15, 16, 17, 18, 19, 20, 20, 21, 22, 23, 23, 24, 25, 25, 26, 28, 29, 31, 33, 35, 37, 40, 43, 46, 51, 54, 55, 58, 58, 61, 64, 67, 68, 72, 75, 78, 80, 84, 85, 89, 87, 90, 93, 95, 98, 99, 102, 105, 108, 110, 111, 113, 116, 117, 117, 119, 121, 123, 125, 128, 129, 132, 132, 133, 135, 135, 136, 138, 139, 140, 143, 146, 149, 151, 154, 156, 160, 163, 167, 170, 173, 173, 176, 178, 180, 182, 183, 184, 185, 186, 186, 187, 187, 187, 187, 187, 187, 187, 186, 186, 186, 185, 185, 185];

    //     assert_eq!(xs.len(), ys.len());
    //     assert_eq!(xs.len(), correct_xs.len() + 1);
    //     assert_eq!(xs.len(), correct_ys.len() + 1);
    //     assert_eq!(ys.len(), correct_xs.len() + 1);
    //     assert_eq!(ys.len(), correct_ys.len() + 1);
    //     assert_eq!(correct_xs.len(), correct_ys.len());

    //     let mut frame = Mat::zeros(640, 360, CV_8UC3).unwrap().to_mat().unwrap();
    //     let dt = 1.0;
    //     let mut tracker = KalmanFilterLinear::new();
    //     tracker.set_state_value(xs[0] as f32, ys[0] as f32);

    //     for tm in 1..xs.len() {
    //         let xt = xs[tm] as f32;
    //         let yt = ys[tm] as f32;
    //         let z = Matrix2x1f32::new(
    //             xt,
    //             yt
    //         );
    //         // tracker.set_time(dt);
    //         let u = Matrix6x1f32::new(
    //             0.0,
    //             0.0,
    //             0.0,
    //             0.0,
    //             0.0,
    //             0.0
    //         );
    //         let state = tracker.step(u, z).unwrap();
    //         println!("state {:?}", state);
    //         let kalman_x = state[(0, 0)];
    //         let kalman_y = state[(3, 0)];

    //         let correct_kalman_x = correct_xs[tm-1];
    //         let correct_kalman_y = correct_ys[tm-1];

    //         // match circle(&mut frame, Point::new(xs[tm], ys[tm]), 1, Scalar::from((0.0, 0.0, 255.0)), 1, LINE_8, 0) {
    //         //     Ok(_) => {},
    //         //     Err(err) => {
    //         //         panic!("Can't draw circle at blob's predicted position due the error: {:?}", err)
    //         //     }
    //         // };
    //         // match circle(&mut frame, Point::new(kalman_x as i32, kalman_y as i32), 1, Scalar::from((255.0, 0.0, 0.0)), 1, LINE_8, 0) {
    //         //     Ok(_) => {},
    //         //     Err(err) => {
    //         //         panic!("Can't draw circle at blob's predicted position due the error: {:?}", err)
    //         //     }
    //         // };
    //         println!("\tcorrect x {} kalman x {}", correct_kalman_x, kalman_x);
    //         println!("\tcorrect y {} kalman y {}", correct_kalman_y, kalman_y);
    //         // assert_eq!(correct_kalman_x, kalman_x as i32);
    //         // assert_eq!(correct_kalman_y, kalman_y as i32);
    //     };
    //     // let window = "test_constant_velocity_model()";
    //     // match highgui::named_window(window, 1) {
    //     //     Ok(_) => {},
    //     //     Err(err) =>{
    //     //         panic!("Can't give a name to output window due the error: {:?}", err)
    //     //     }
    //     // };
    //     // loop {
    //     //     highgui::imshow(window, &mut frame).unwrap();
    //     //     let key = highgui::wait_key(10).unwrap();
    //     //     if key > 0 && key != 255 {
    //     //         break;
    //     //     }
    //     // }
    // }
}