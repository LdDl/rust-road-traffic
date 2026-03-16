// Own scalar type for BGR color values (just copy of opencv::core::Scalar)

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scalar(pub [f64; 4]);

impl Default for Scalar {
    fn default() -> Self {
        Scalar([0.0, 0.0, 0.0, 0.0])
    }
}

impl From<(f64, f64, f64)> for Scalar {
    fn from((v0, v1, v2): (f64, f64, f64)) -> Self {
        Scalar([v0, v1, v2, 0.0])
    }
}

impl std::ops::Index<usize> for Scalar {
    type Output = f64;
    fn index(&self, i: usize) -> &f64 {
        &self.0[i]
    }
}
