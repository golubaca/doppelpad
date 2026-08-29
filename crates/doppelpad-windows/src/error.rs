//! Error type for the Windows driver client.

use std::fmt;

/// Errors returned when talking to the DoppelPad driver.
#[derive(Debug)]
pub enum Error {
    /// The control device could not be opened. Usually means the driver is not
    /// installed, or the caller lacks the required privileges.
    Open(windows::core::Error),
    /// A control request (ioctl) failed. `op` names the step that failed.
    Ioctl {
        op: &'static str,
        source: windows::core::Error,
    },
    /// Submitting an input report failed.
    SubmitReport(windows::core::Error),
    /// The report descriptor was empty or too large (must fit in a u16 length).
    InvalidReportDescriptor,
    /// A driver response was smaller than its fixed-size header.
    ShortResponse { op: &'static str },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Open(e) => write!(f, "failed to open the DoppelPad control device: {e}"),
            Error::Ioctl { op, source } => write!(f, "driver request '{op}' failed: {source}"),
            Error::SubmitReport(e) => write!(f, "failed to submit input report: {e}"),
            Error::InvalidReportDescriptor => {
                write!(f, "report descriptor is empty or larger than 65535 bytes")
            }
            Error::ShortResponse { op } => write!(f, "driver response for '{op}' was too short"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Open(e) | Error::SubmitReport(e) => Some(e),
            Error::Ioctl { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, Error>;
