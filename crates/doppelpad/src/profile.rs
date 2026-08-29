//! Controller profiles: the byte-level knowledge for one controller identity.

use crate::backend::DeviceConfig;
use crate::state::{GamepadState, Rumble};

/// The maximum HID report size any profile produces. `VirtualGamepad` encodes
/// into a stack buffer of this size, so `set_state` never allocates.
pub const MAX_REPORT_SIZE: usize = 64;

/// Encodes typed gamepad state into HID reports for a specific controller
/// identity, and decodes that controller's output reports into rumble.
pub trait GamepadProfile: Sync {
    /// The device identity and HID descriptor to create.
    fn device_config(&self) -> DeviceConfig<'static>;

    /// Encode `state` into `out`, returning the report length. Implementations
    /// must write no more than `MAX_REPORT_SIZE` bytes.
    fn encode_input(&self, state: &GamepadState, out: &mut [u8]) -> usize;

    /// Decode an output report into rumble, or `None` if it carries nothing we
    /// map (e.g. a keep-alive with zero magnitude is still decoded, as zero).
    fn decode_output(&self, report_id: u8, data: &[u8]) -> Option<Rumble>;
}
