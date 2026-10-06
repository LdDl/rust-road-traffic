use std::fmt;
use tracing::info;

use crate::lib::detection::{Detector, DetectorError};
use crate::lib::logging;
use crate::settings::{InferenceModelSettings, PlatesSettings};

pub struct PlateModels {
    pub detection: Detector,
    pub ocr: Option<Detector>,
}

#[derive(Debug)]
pub struct PlateModelLoadError {
    model: &'static str,
    source: DetectorError,
}

impl fmt::Display for PlateModelLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.model, self.source)
    }
}

impl std::error::Error for PlateModelLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl PlateModels {
    pub fn from_settings(
        settings: Option<&PlatesSettings>,
    ) -> Result<Option<Self>, PlateModelLoadError> {
        let Some(settings) = settings.filter(|settings| settings.enable) else {
            return Ok(None);
        };

        let detection = Self::load_model(&settings.detection, "plates.detection")?;
        let ocr = if settings.ocr.enable {
            Some(Self::load_model(&settings.ocr.model, "plates.ocr")?)
        } else {
            None
        };

        Ok(Some(Self { detection, ocr }))
    }

    fn load_model(
        settings: &InferenceModelSettings,
        model: &'static str,
    ) -> Result<Detector, PlateModelLoadError> {
        info!(
            scope = logging::SCOPE_STARTUP,
            model,
            weights = %settings.network_weights,
            "Loading plate model"
        );
        let detector = Detector::new(
            &settings.network_weights,
            settings.net_width.zip(settings.net_height),
            None,
        )
        .map_err(|source| PlateModelLoadError { model, source })?;
        info!(scope = logging::SCOPE_STARTUP, model, "Plate model loaded");
        Ok(detector)
    }
}
