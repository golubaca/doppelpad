//! Adapter exposing the DoppelPad driver as a [`doppelpad::Backend`].

use doppelpad::{Backend, DeviceConfig, HidDevice, OutputSink};

use crate::device::{VirtualHidDevice, VirtualHidDeviceBuilder};

/// The Windows backend: creates virtual HID devices through the DoppelPad driver.
#[derive(Default)]
pub struct WindowsBackend;

impl WindowsBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Backend for WindowsBackend {
    type Device = WindowsHidDevice;

    fn create_device(
        &self,
        config: &DeviceConfig<'_>,
        output: Option<OutputSink>,
    ) -> doppelpad::Result<Self::Device> {
        let mut builder = VirtualHidDeviceBuilder::new(
            config.vendor_id,
            config.product_id,
            config.report_descriptor.to_vec(),
        )
        .version_number(config.version)
        .hardware_ids(config.hardware_ids.iter().copied());

        if let Some(sink) = output {
            // The core OutputSink and our OutputCallback are the same signature.
            builder = builder.on_output(sink);
        }

        let device = builder
            .build()
            .map_err(|e| doppelpad::Error::Backend(Box::new(e)))?;
        Ok(WindowsHidDevice(device))
    }
}

/// A live device created by [`WindowsBackend`].
pub struct WindowsHidDevice(VirtualHidDevice);

impl HidDevice for WindowsHidDevice {
    fn submit_input_report(&self, report: &[u8]) -> doppelpad::Result<()> {
        self.0
            .submit_input_report(report)
            .map_err(|e| doppelpad::Error::Backend(Box::new(e)))
    }
}
