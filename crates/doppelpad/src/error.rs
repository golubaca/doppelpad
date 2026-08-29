//! Error type for the core API.

/// Errors from creating or driving a virtual gamepad.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// The platform backend failed. The source is the backend's own error.
    #[error("backend failure: {0}")]
    Backend(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// A profile could not encode a report into the provided buffer.
    #[error("report buffer too small: need {needed}, have {have}")]
    ReportBufferTooSmall { needed: usize, have: usize },
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, Error>;
