/// Raw BGR24 frame, row-major layout, no padding.
#[derive(Clone)]
pub struct RawFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl RawFrame {
    /// Create a zeroed frame with given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0u8; width as usize * height as usize * 3],
            width,
            height,
        }
    }

    /// Frame width (matches Mat::cols() signature).
    pub fn cols(&self) -> i32 {
        self.width as i32
    }

    /// Frame height (matches Mat::rows() signature).
    pub fn rows(&self) -> i32 {
        self.height as i32
    }

    /// Row stride in bytes (width * 3 for BGR24).
    pub fn step(&self) -> usize {
        self.width as usize * 3
    }

    /// Immutable access to raw pixel data.
    pub fn data_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Mutable access to raw pixel data.
    pub fn data_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// True if frame has no data.
    pub fn empty(&self) -> bool {
        self.data.is_empty()
    }
}
