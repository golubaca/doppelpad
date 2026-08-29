//! Spike-turned-example: a virtual Xbox controller via the high-level core API,
//! probed through XInput in the same process.
//!
//! Uses `doppelpad::VirtualGamepad` with the `XBOX_ONE` profile on the Windows
//! backend, then reads the pad back through XInput and triggers rumble to
//! confirm the round-trips. See `docs/xbox-spike.md` for the findings.
//!
//! Requires the DoppelPad driver installed and an elevated session.
//! Run: `cargo run -p doppelpad-windows --example xbox_gamepad`

#[cfg(not(windows))]
fn main() {
    eprintln!("The xbox_gamepad example is Windows-only.");
}

#[cfg(windows)]
fn main() {
    windows_impl::run();
}

#[cfg(windows)]
mod windows_impl {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread::sleep;
    use std::time::Duration;

    use doppelpad::profiles::XBOX_ONE;
    use doppelpad::{Buttons, GamepadState, Rumble, Stick, VirtualGamepad};
    use doppelpad_windows::WindowsBackend;
    use windows::Win32::UI::Input::XboxController::{
        XINPUT_STATE, XINPUT_VIBRATION, XInputGetState, XInputSetState,
    };

    fn connected_slot() -> Option<u32> {
        for slot in 0..4u32 {
            let mut state = XINPUT_STATE::default();
            if unsafe { XInputGetState(slot, &mut state) } == 0 {
                return Some(slot);
            }
        }
        None
    }

    pub fn run() {
        let peak_rumble = Arc::new(AtomicU32::new(0));
        let peak = peak_rumble.clone();

        let backend = WindowsBackend::new();
        let pad = VirtualGamepad::new(&backend, &XBOX_ONE, move |r: Rumble| {
            peak.fetch_max(r.left_motor.max(r.right_motor) as u32, Ordering::SeqCst);
        })
        .expect("create virtual gamepad");
        println!("gamepad created via core API");

        pad.set_state(&GamepadState::default()).ok();
        println!("waiting for XInput to pick up the device...");
        let mut slot = None;
        for _ in 0..50 {
            sleep(Duration::from_millis(200));
            slot = connected_slot();
            if slot.is_some() {
                break;
            }
        }
        let Some(slot) = slot else {
            println!("RESULT: XInput did NOT enumerate the device.");
            return;
        };
        println!("RESULT: XInput sees the device in slot {slot}");

        // Press A and push the left stick, and read it back through XInput.
        let mut input_ok = false;
        for i in 0..20 {
            let pressed = i % 2 == 0;
            let state = GamepadState {
                buttons: if pressed {
                    Buttons::A
                } else {
                    Buttons::empty()
                },
                left_stick: Stick::new(if pressed { 30000 } else { 0 }, 0),
                ..Default::default()
            };
            pad.set_state(&state).ok();
            sleep(Duration::from_millis(50));

            let mut xs = XINPUT_STATE::default();
            if unsafe { XInputGetState(slot, &mut xs) } == 0 && pressed {
                let buttons = xs.Gamepad.wButtons.0;
                let lx = xs.Gamepad.sThumbLX;
                if buttons != 0 && lx > 10000 {
                    input_ok = true;
                    println!("  XInput read buttons=0x{buttons:04X} LX={lx}");
                }
            }
        }
        println!(
            "RESULT: input round-trip {}",
            if input_ok { "OK" } else { "FAILED" }
        );

        // Rumble.
        let vib = XINPUT_VIBRATION {
            wLeftMotorSpeed: 40000,
            wRightMotorSpeed: 40000,
        };
        let _ = unsafe { XInputSetState(slot, &vib) };
        sleep(Duration::from_millis(300));
        let _ = unsafe { XInputSetState(slot, &XINPUT_VIBRATION::default()) };
        sleep(Duration::from_millis(200));
        let peak = peak_rumble.load(Ordering::SeqCst);
        println!(
            "RESULT: rumble round-trip {} (peak motor magnitude {peak})",
            if peak > 0 { "OK" } else { "FAILED" }
        );

        println!("removing device");
    }
}
