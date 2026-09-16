//! One shape for everything this API refuses.
//!
//! A body that could not be read at all is one kind of answer: what exactly
//! was wrong with it goes to the log, since it describes how this app is
//! built rather than what the caller should do. A body that was read but
//! holds settings this app will not take is the other kind, and there the
//! caller is told about every one of them at once, by name.
use serde::Serialize;
use utoipa::ToSchema;

/// A setting that was not accepted, and why
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FieldError {
    /// Dotted path, spelled the way `changed` and `unsaved_changes` spell it
    #[schema(example = "input.process_every_nth_frame")]
    pub field: String,
    /// What is wrong with it. The path is not repeated here
    #[schema(example = "must not be negative, got -3")]
    pub error: String,
}

impl FieldError {
    pub fn new(field: impl Into<String>, error: impl Into<String>) -> Self {
        FieldError {
            field: field.into(),
            error: error.into(),
        }
    }
}

/// Why a request was not accepted
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// What kind of refusal this is
    #[schema(example = "invalid settings")]
    pub error_text: String,
    /// One entry per setting that was not accepted; empty when the refusal is
    /// about the request as a whole
    pub details: Vec<FieldError>,
}

impl ErrorResponse {
    /// A refusal that is not about any particular setting
    pub fn text(error_text: impl Into<String>) -> Self {
        ErrorResponse {
            error_text: error_text.into(),
            details: Vec::new(),
        }
    }

    /// Settings that were not accepted, each with its own reason
    pub fn fields(details: Vec<FieldError>) -> Self {
        ErrorResponse {
            error_text: "invalid settings".to_string(),
            details,
        }
    }
}
