//! Integration test: creating and dropping an Xbox One pad with a rumble callback,
//! many times over, neither crashes nor leaks a device. This drives the reader
//! thread's stop path, where a pending event request must finish before its buffer,
//! OVERLAPPED and event are freed.
//!
//! Ignored by default because it requires the DoppelPad driver installed and an
//! elevated session; CI cannot run it. Run explicitly with:
//!
//! ```text
//! cargo test -p doppelpad-windows --test gamepad_teardown -- --ignored
//! ```

#![cfg(windows)]

use std::thread::sleep;
use std::time::Duration;

use doppelpad::profiles::XBOX_ONE;
use doppelpad::{GamepadState, VirtualGamepad};
use doppelpad_windows::WindowsBackend;
use windows::Win32::UI::Input::XboxController::{XINPUT_STATE, XInputGetState};

fn connected_pads() -> usize {
    (0..4u32)
        .filter(|&slot| {
            let mut state = XINPUT_STATE::default();
            unsafe { XInputGetState(slot, &mut state) == 0 }
        })
        .count()
}

#[test]
#[ignore = "requires the DoppelPad driver installed and an elevated session"]
fn repeated_create_and_drop_does_not_crash_or_leak() {
    let before = connected_pads();
    let backend = WindowsBackend::new();
    for i in 0..200u64 {
        let pad = VirtualGamepad::new(&backend, &XBOX_ONE, |_rumble| {}).expect("create pad");
        pad.set_state(&GamepadState::default()).expect("set_state");
        // Vary timing so drops land both while the reader waits and mid-event.
        sleep(Duration::from_millis(i % 5));
        drop(pad);
    }
    // XInput sees removals asynchronously.
    sleep(Duration::from_secs(1));
    assert_eq!(connected_pads(), before, "a virtual pad outlived its drop");
}
