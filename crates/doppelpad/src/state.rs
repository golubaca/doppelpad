//! Typed gamepad input state and decoded output.
//!
//! State uses XInput-style ranges (sticks `i16`, triggers `u8`) — the shape
//! games and XInput clients speak. Profiles map it to their HID descriptor's
//! ranges.

use bitflags::bitflags;

bitflags! {
    /// Digital gamepad buttons, matching the common Xbox layout.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Buttons: u16 {
        const A = 1 << 0;
        const B = 1 << 1;
        const X = 1 << 2;
        const Y = 1 << 3;
        const LEFT_BUMPER = 1 << 4;
        const RIGHT_BUMPER = 1 << 5;
        const BACK = 1 << 6;
        const START = 1 << 7;
        const LEFT_STICK = 1 << 8;
        const RIGHT_STICK = 1 << 9;
        const GUIDE = 1 << 10;
    }
}

/// Eight-way directional pad position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dpad {
    #[default]
    Neutral,
    Up,
    UpRight,
    Right,
    DownRight,
    Down,
    DownLeft,
    Left,
    UpLeft,
}

/// An analog stick. `x` is right-positive, `y` is up-positive (XInput
/// convention).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stick {
    pub x: i16,
    pub y: i16,
}

impl Stick {
    pub fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }
}

/// Complete gamepad state for one frame. `Default` is the neutral pose:
/// centered sticks, released triggers and buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GamepadState {
    pub buttons: Buttons,
    pub dpad: Dpad,
    pub left_stick: Stick,
    pub right_stick: Stick,
    pub left_trigger: u8,
    pub right_trigger: u8,
}

/// A decoded rumble command from the host, delivered to the rumble callback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Rumble {
    /// Low-frequency (strong) motor magnitude, 0..=255.
    pub left_motor: u8,
    /// High-frequency (weak) motor magnitude, 0..=255.
    pub right_motor: u8,
    /// Left trigger motor ("impulse trigger") magnitude, 0..=255. Zero for
    /// profiles without trigger motors.
    pub left_trigger: u8,
    /// Right trigger motor magnitude, 0..=255. Zero for profiles without
    /// trigger motors.
    pub right_trigger: u8,
}
