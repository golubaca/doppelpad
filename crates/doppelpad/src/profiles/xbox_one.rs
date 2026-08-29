//! Xbox One controller profile (VID 045E / PID 02FF), the identity proven in the
//! #5 spike to bind `xinputhid` for XInput and rumble.

use crate::backend::DeviceConfig;
use crate::profile::GamepadProfile;
use crate::state::{Buttons, Dpad, GamepadState, Rumble};

/// The `XBOX_ONE` profile instance. Pass `&doppelpad::profiles::XBOX_ONE` to
/// [`crate::VirtualGamepad::new`].
pub static XBOX_ONE: XboxOne = XboxOne;

/// Xbox One controller profile. Use the [`XBOX_ONE`] instance.
pub struct XboxOne;

const HW_IDS: &[&str] = &["HID\\VID_045E&PID_02FF&IG_00"];

// HID report descriptor ported from WinUHid's Xbox One sample. 17-byte input
// report: X, Y, Rx, Ry (u16), Z, Rz triggers (10-bit), 10 buttons, hat,
// sys-menu bit, battery. 8-byte output report for rumble.
#[rustfmt::skip]
const DESCRIPTOR: &[u8] = &[
    0x05, 0x01, 0x09, 0x05, 0xA1, 0x01, 0xA1, 0x00, 0x09, 0x30, 0x09, 0x31, 0x15, 0x00, 0x27,
    0xFF, 0xFF, 0x00, 0x00, 0x95, 0x02, 0x75, 0x10, 0x81, 0x02, 0xC0, 0xA1, 0x00, 0x09, 0x33,
    0x09, 0x34, 0x15, 0x00, 0x27, 0xFF, 0xFF, 0x00, 0x00, 0x95, 0x02, 0x75, 0x10, 0x81, 0x02,
    0xC0, 0x05, 0x01, 0x09, 0x32, 0x15, 0x00, 0x26, 0xFF, 0x03, 0x95, 0x01, 0x75, 0x0A, 0x81,
    0x02, 0x15, 0x00, 0x25, 0x00, 0x75, 0x06, 0x95, 0x01, 0x81, 0x03, 0x05, 0x01, 0x09, 0x35,
    0x15, 0x00, 0x26, 0xFF, 0x03, 0x95, 0x01, 0x75, 0x0A, 0x81, 0x02, 0x15, 0x00, 0x25, 0x00,
    0x75, 0x06, 0x95, 0x01, 0x81, 0x03, 0x05, 0x09, 0x19, 0x01, 0x29, 0x0A, 0x95, 0x0A, 0x75,
    0x01, 0x81, 0x02, 0x15, 0x00, 0x25, 0x00, 0x75, 0x06, 0x95, 0x01, 0x81, 0x03, 0x05, 0x01,
    0x09, 0x39, 0x15, 0x01, 0x25, 0x08, 0x35, 0x00, 0x46, 0x3B, 0x01, 0x66, 0x14, 0x00, 0x75,
    0x04, 0x95, 0x01, 0x81, 0x42, 0x75, 0x04, 0x95, 0x01, 0x15, 0x00, 0x25, 0x00, 0x35, 0x00,
    0x45, 0x00, 0x65, 0x00, 0x81, 0x03, 0xA1, 0x02, 0x05, 0x0F, 0x09, 0x97, 0x15, 0x00, 0x25,
    0x01, 0x75, 0x04, 0x95, 0x01, 0x91, 0x02, 0x15, 0x00, 0x25, 0x00, 0x91, 0x03, 0x09, 0x70,
    0x15, 0x00, 0x25, 0x64, 0x75, 0x08, 0x95, 0x04, 0x91, 0x02, 0x09, 0x50, 0x66, 0x01, 0x10,
    0x55, 0x0E, 0x26, 0xFF, 0x00, 0x95, 0x01, 0x91, 0x02, 0x09, 0xA7, 0x91, 0x02, 0x65, 0x00,
    0x55, 0x00, 0x09, 0x7C, 0x91, 0x02, 0xC0, 0x05, 0x01, 0x09, 0x80, 0xA1, 0x00, 0x09, 0x85,
    0x15, 0x00, 0x25, 0x01, 0x95, 0x01, 0x75, 0x01, 0x81, 0x02, 0x15, 0x00, 0x25, 0x00, 0x75,
    0x07, 0x95, 0x01, 0x81, 0x03, 0xC0, 0x05, 0x06, 0x09, 0x20, 0x15, 0x00, 0x26, 0xFF, 0x00,
    0x75, 0x08, 0x95, 0x01, 0x81, 0x02, 0xC0,
];

const REPORT_LEN: usize = 17;

/// Map an XInput-style axis (`i16`, center 0) to the descriptor's 0..=65534
/// range (center ~32767). HID gamepad Y is down-positive while XInput Y is
/// up-positive, so the vertical axes are inverted.
fn axis(v: i16, invert: bool) -> u16 {
    let val = if invert { -(v as i32) } else { v as i32 };
    (val + 32768).clamp(0, 65534) as u16
}

/// Map a u8 trigger (0..=255) to the descriptor's 10-bit range (0..=1023).
fn trigger(v: u8) -> u16 {
    ((v as u32 * 1023) / 255) as u16
}

fn hat(dpad: Dpad) -> u8 {
    // Descriptor logical range is 1..=8 clockwise from Up; 0 is the null state.
    match dpad {
        Dpad::Neutral => 0,
        Dpad::Up => 1,
        Dpad::UpRight => 2,
        Dpad::Right => 3,
        Dpad::DownRight => 4,
        Dpad::Down => 5,
        Dpad::DownLeft => 6,
        Dpad::Left => 7,
        Dpad::UpLeft => 8,
    }
}

impl GamepadProfile for XboxOne {
    fn device_config(&self) -> DeviceConfig<'static> {
        DeviceConfig {
            vendor_id: 0x045E,
            product_id: 0x02FF,
            version: 1,
            report_descriptor: DESCRIPTOR,
            hardware_ids: HW_IDS,
        }
    }

    fn encode_input(&self, s: &GamepadState, out: &mut [u8]) -> usize {
        let r = &mut out[..REPORT_LEN];
        r.fill(0);
        r[0..2].copy_from_slice(&axis(s.left_stick.x, false).to_le_bytes());
        r[2..4].copy_from_slice(&axis(s.left_stick.y, true).to_le_bytes());
        r[4..6].copy_from_slice(&axis(s.right_stick.x, false).to_le_bytes());
        r[6..8].copy_from_slice(&axis(s.right_stick.y, true).to_le_bytes());
        let lt = trigger(s.left_trigger);
        r[8] = (lt & 0xFF) as u8;
        r[9] = ((lt >> 8) & 0x03) as u8;
        let rt = trigger(s.right_trigger);
        r[10] = (rt & 0xFF) as u8;
        r[11] = ((rt >> 8) & 0x03) as u8;
        // Buttons A..RightStick occupy the low 10 bits in the same order our
        // `Buttons` flags define, so they map directly. Guide is the sys-menu.
        let btn = s.buttons.bits() & 0x03FF;
        r[12] = (btn & 0xFF) as u8;
        r[13] = ((btn >> 8) & 0x03) as u8;
        r[14] = hat(s.dpad);
        r[15] = u8::from(s.buttons.contains(Buttons::GUIDE));
        r[16] = 0xFF; // battery full
        REPORT_LEN
    }

    fn decode_output(&self, _report_id: u8, data: &[u8]) -> Option<Rumble> {
        // Output report: [MotorsEnabled, LTrig, RTrig, LeftMotor, RightMotor, ...].
        if data.len() < 5 {
            return None;
        }
        let enabled = data[0];
        // Motor magnitudes are 0..=100 (descriptor logical max); scale to 0..=255.
        let scale = |v: u8| ((v as u16 * 255) / 100).min(255) as u8;
        Some(Rumble {
            left_motor: if enabled & 0x1 != 0 {
                scale(data[3])
            } else {
                0
            },
            right_motor: if enabled & 0x2 != 0 {
                scale(data[4])
            } else {
                0
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::MAX_REPORT_SIZE;

    #[test]
    fn neutral_state_encodes_centered() {
        let mut buf = [0u8; MAX_REPORT_SIZE];
        let n = XBOX_ONE.encode_input(&GamepadState::default(), &mut buf);
        assert_eq!(n, REPORT_LEN);
        // Centered sticks ~ 0x8000.
        assert_eq!(u16::from_le_bytes([buf[0], buf[1]]), 32768);
        assert_eq!(buf[12], 0); // no buttons
        assert_eq!(buf[14], 0); // dpad neutral
    }

    #[test]
    fn button_a_sets_bit_zero() {
        let mut buf = [0u8; MAX_REPORT_SIZE];
        let state = GamepadState {
            buttons: Buttons::A,
            ..Default::default()
        };
        XBOX_ONE.encode_input(&state, &mut buf);
        assert_eq!(buf[12] & 0x01, 0x01);
    }

    #[test]
    fn decodes_rumble_magnitude() {
        // Both motors enabled, left magnitude 100 (max).
        let report = [0x0F, 0, 0, 100, 0, 0xFF, 0, 0];
        let r = XBOX_ONE.decode_output(0, &report).unwrap();
        assert_eq!(r.left_motor, 255);
        assert_eq!(r.right_motor, 0);
    }
}
