//! Spike: a virtual Xbox controller via the `xinputhid` route, probed through
//! XInput in the same process.
//!
//! Creates a gamepad with the Xbox One identity (VID 045E / PID 02FF) and the
//! `HID\VID_045E&PID_02FF&IG_00` hardware ID that `xinputhid.inf` matches, so
//! Windows binds `xinputhid.sys` and synthesizes XInput. Then it submits input
//! reports, reads them back through `XInputGetState`, and triggers rumble via
//! `XInputSetState` to confirm the output report reaches our callback.
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

    use doppelpad_windows::{OutputReport, VirtualHidDeviceBuilder};
    use windows::Win32::UI::Input::XboxController::{
        XINPUT_STATE, XINPUT_VIBRATION, XInputGetState, XInputSetState,
    };

    // Xbox One gamepad HID report descriptor (ported from WinUHid's WinUHidXOne).
    // Input report is 17 bytes: X,Y,Rx,Ry (u16 each), Z,Rz triggers (10-bit),
    // 10 buttons, hat, sys-menu bit, battery.
    const XONE_DESCRIPTOR: &[u8] = &[
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

    const CENTER: u16 = 0x7FFF;
    // Button bit 0 (usage 1) maps to Xbox "A" in XInput's synthesis.
    const BTN_A: u16 = 0x0001;

    fn input_report(buttons: u16) -> [u8; 17] {
        let mut r = [0u8; 17];
        r[0..2].copy_from_slice(&CENTER.to_le_bytes()); // X
        r[2..4].copy_from_slice(&CENTER.to_le_bytes()); // Y
        r[4..6].copy_from_slice(&CENTER.to_le_bytes()); // Rx
        r[6..8].copy_from_slice(&CENTER.to_le_bytes()); // Ry
        // triggers (r[8..10], r[10..12]) left 0
        r[12] = (buttons & 0xFF) as u8; // 10 buttons: low byte
        r[13] = ((buttons >> 8) & 0x03) as u8; // high 2 button bits
        r[14] = 0x00; // hat null
        r[15] = 0x00; // sys menu
        r[16] = 0xFF; // battery full
        r
    }

    fn connected_slot() -> Option<u32> {
        for slot in 0..4u32 {
            let mut state = XINPUT_STATE::default();
            let rc = unsafe { XInputGetState(slot, &mut state) };
            if rc == 0 {
                return Some(slot);
            }
        }
        None
    }

    pub fn run() {
        // `xinputhid` streams continuous keep-alive output reports (all-zero
        // magnitude). Track the peak motor magnitude to prove real rumble, and
        // print only when the report changes so the console does not flood.
        let max_magnitude = Arc::new(AtomicU32::new(0));
        let mm = max_magnitude.clone();
        let gamepad = VirtualHidDeviceBuilder::new(0x045E, 0x02FF, XONE_DESCRIPTOR)
            .version_number(1)
            .hardware_ids(["HID\\VID_045E&PID_02FF&IG_00"])
            .on_output(Box::new({
                let mut last: Option<Vec<u8>> = None;
                move |o: OutputReport| {
                    // Bytes 3 and 4 are the left/right rumble motor magnitudes.
                    let mag = *o.data.get(3).unwrap_or(&0) as u32;
                    mm.fetch_max(mag, Ordering::SeqCst);
                    if last.as_deref() != Some(o.data.as_slice()) {
                        println!("  <- output report: {:02X?}", o.data);
                        last = Some(o.data);
                    }
                }
            }))
            .build()
            .expect("create virtual gamepad");
        println!(
            "gamepad created (interface v{})",
            gamepad.interface_version().unwrap()
        );

        // Send a neutral report and let PnP/xinputhid enumerate.
        gamepad.submit_input_report(&input_report(0)).ok();
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
            println!("RESULT: XInput did NOT enumerate the device (no connected slot).");
            return;
        };
        println!("RESULT: XInput sees the device in slot {slot}");

        // Press A for a few frames and confirm XInput reads it.
        let mut a_seen = false;
        for i in 0..20 {
            let pressed = i % 2 == 0;
            gamepad
                .submit_input_report(&input_report(if pressed { BTN_A } else { 0 }))
                .ok();
            sleep(Duration::from_millis(50));
            let mut state = XINPUT_STATE::default();
            if unsafe { XInputGetState(slot, &mut state) } == 0 {
                let buttons = state.Gamepad.wButtons.0;
                if pressed && buttons != 0 {
                    a_seen = true;
                    println!("  XInput read buttons=0x{buttons:04X} (A press seen)");
                }
            }
        }
        println!(
            "RESULT: input round-trip {}",
            if a_seen { "OK" } else { "FAILED" }
        );

        // Trigger rumble and confirm our callback fires.
        let vib = XINPUT_VIBRATION {
            wLeftMotorSpeed: 40000,
            wRightMotorSpeed: 40000,
        };
        let _ = unsafe { XInputSetState(slot, &vib) };
        sleep(Duration::from_millis(300));
        let stop = XINPUT_VIBRATION::default();
        let _ = unsafe { XInputSetState(slot, &stop) };
        sleep(Duration::from_millis(200));
        let peak = max_magnitude.load(Ordering::SeqCst);
        // 40000/65535 * 100 ~= 61 (0x3D); any non-zero magnitude proves the path.
        println!(
            "RESULT: rumble round-trip {} (peak motor magnitude {peak})",
            if peak > 0 { "OK" } else { "FAILED" }
        );

        println!("removing device");
    }
}
