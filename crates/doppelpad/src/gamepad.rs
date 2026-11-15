//! The high-level virtual gamepad.

use crate::backend::{Backend, HidDevice, OutputSink};
use crate::error::Result;
use crate::profile::{GamepadProfile, MAX_REPORT_SIZE};
use crate::state::{GamepadState, Rumble};

/// A live virtual gamepad. Set typed state with [`set_state`](Self::set_state);
/// receive rumble via the callback given to [`new`](Self::new). The device is
/// removed when this value is dropped.
pub struct VirtualGamepad<D: HidDevice> {
    device: D,
    profile: &'static dyn GamepadProfile,
}

impl<D: HidDevice> VirtualGamepad<D> {
    /// Create a virtual gamepad for `profile` on `backend`, delivering rumble to
    /// `on_rumble` (invoked on the backend's reader thread).
    pub fn new<B, F>(
        backend: &B,
        profile: &'static dyn GamepadProfile,
        mut on_rumble: F,
    ) -> Result<Self>
    where
        B: Backend<Device = D>,
        F: FnMut(Rumble) + Send + 'static,
    {
        let config = profile.device_config();
        let sink: OutputSink = Box::new(move |report_id, data| {
            if let Some(rumble) = profile.decode_output(report_id, data) {
                on_rumble(rumble);
            }
        });
        let device = backend.create_device(&config, Some(sink))?;
        Ok(Self { device, profile })
    }

    /// Encode `state` and submit it as one HID input report. Allocation-free:
    /// the report is built in a stack buffer.
    pub fn set_state(&self, state: &GamepadState) -> Result<()> {
        let mut buf = [0u8; MAX_REPORT_SIZE];
        let len = self.profile.encode_input(state, &mut buf)?;
        self.device.submit_input_report(&buf[..len])
    }
}
