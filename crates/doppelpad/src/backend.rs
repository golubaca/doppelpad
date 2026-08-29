//! The platform backend abstraction.
//!
//! A backend is a HID transport: it creates a virtual HID device from a
//! descriptor and identity, accepts input reports, and delivers output reports.
//! All gamepad-specific knowledge lives in profiles, not backends.

use crate::error::Result;

/// Identity and HID descriptor for a device to create.
pub struct DeviceConfig<'a> {
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    pub report_descriptor: &'a [u8],
    /// Extra PnP hardware IDs (e.g. `HID\VID_045E&PID_02FF&IG_00`).
    pub hardware_ids: &'a [&'a str],
}

/// Sink for output reports, invoked on the backend's reader thread with a
/// borrowed slice (report id, then body) so no allocation is needed per event.
pub type OutputSink = Box<dyn FnMut(u8, &[u8]) + Send>;

/// A platform backend that creates virtual HID devices.
pub trait Backend {
    type Device: HidDevice;

    /// Create and start a device. If `output` is `Some`, the backend delivers
    /// output reports to it.
    fn create_device(
        &self,
        config: &DeviceConfig<'_>,
        output: Option<OutputSink>,
    ) -> Result<Self::Device>;
}

/// A live virtual HID device. Dropping it removes the device.
pub trait HidDevice: Send {
    /// Submit a raw HID input report. Called per frame; must not allocate.
    fn submit_input_report(&self, report: &[u8]) -> Result<()>;
}
