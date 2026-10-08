use std::sync::Arc;
use tracing::warn;
use turbojpeg::{Compressor, Image, PixelFormat, Subsamp};

use crate::lib::cv::RawFrame;
use crate::lib::logging;
use crate::settings::EventImage;

pub type JpegImage = Arc<Vec<u8>>;

pub struct EventImages {
    mode: EventImage,
    compressor: Option<Compressor>,
    full_frame: Option<Option<JpegImage>>,
}

impl EventImages {
    pub fn new(mode: EventImage) -> Self {
        let compressor = if mode == EventImage::None {
            None
        } else {
            match Self::compressor() {
                Ok(compressor) => Some(compressor),
                Err(error) => {
                    warn!(scope = logging::SCOPE_PROCESSING, %error, "Event image encoder unavailable");
                    None
                }
            }
        };
        Self {
            mode,
            compressor,
            full_frame: None,
        }
    }

    fn compressor() -> Result<Compressor, turbojpeg::Error> {
        let mut compressor = Compressor::new()?;
        compressor.set_quality(90)?;
        compressor.set_subsamp(Subsamp::None)?;
        Ok(compressor)
    }

    pub fn mode(&self) -> EventImage {
        self.mode
    }

    pub fn begin_frame(&mut self) {
        self.full_frame = None;
    }

    pub fn full_frame(&mut self, frame: &RawFrame) -> Option<JpegImage> {
        if self.mode != EventImage::Full {
            return None;
        }
        // Tracks using the same source frame share one compressed image, including pending candidates.
        if self.full_frame.is_none() {
            self.full_frame = Some(self.encode(frame));
        }
        self.full_frame.as_ref()?.clone()
    }

    pub fn encode(&mut self, frame: &RawFrame) -> Option<JpegImage> {
        let compressor = self.compressor.as_mut()?;
        let image = Image {
            pixels: frame.data.as_slice(),
            width: frame.width as usize,
            pitch: frame.step(),
            height: frame.height as usize,
            format: PixelFormat::BGR,
        };
        match compressor.compress_to_vec(image) {
            Ok(bytes) => Some(Arc::new(bytes)),
            Err(error) => {
                warn!(scope = logging::SCOPE_PROCESSING, %error, "Event image encoding failed");
                None
            }
        }
    }
}
