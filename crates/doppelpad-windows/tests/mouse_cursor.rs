//! Integration test: a virtual mouse moves the real cursor, then unplugs cleanly.
//!
//! Ignored by default because it requires the DoppelPad driver to be installed
//! and it moves the system cursor. Run explicitly with:
//!
//! ```text
//! cargo test -p doppelpad-windows --test mouse_cursor -- --ignored
//! ```

#![cfg(windows)]

use std::thread::sleep;
use std::time::Duration;

use doppelpad_windows::VirtualHidDeviceBuilder;
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};

const MOUSE_DESCRIPTOR: &[u8] = &[
    0x05, 0x01, 0x09, 0x02, 0xA1, 0x01, 0x09, 0x01, 0xA1, 0x00, 0x05, 0x09, 0x19, 0x01, 0x29, 0x03,
    0x15, 0x00, 0x25, 0x01, 0x95, 0x03, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01, 0x75, 0x05, 0x81, 0x03,
    0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95, 0x02, 0x81, 0x06,
    0xC0, 0xC0,
];

fn cursor() -> POINT {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).expect("GetCursorPos") };
    p
}

#[test]
#[ignore = "requires the DoppelPad driver installed; moves the system cursor"]
fn virtual_mouse_moves_cursor_then_unplugs() {
    // Park the cursor away from the screen edges so rightward motion has room.
    unsafe { SetCursorPos(200, 200).expect("SetCursorPos") };
    sleep(Duration::from_millis(200));

    let mouse = VirtualHidDeviceBuilder::new(0x1209, 0x0001, MOUSE_DESCRIPTOR)
        .version_number(1)
        .build()
        .expect("create virtual mouse");

    assert_eq!(mouse.interface_version().expect("interface_version"), 1);

    // Give PnP a moment to enumerate the HID child before injecting input.
    sleep(Duration::from_millis(500));
    let start = cursor();

    for _ in 0..25 {
        mouse
            .submit_input_report(&[0x00, 20, 0])
            .expect("submit report");
        sleep(Duration::from_millis(10));
    }
    sleep(Duration::from_millis(200));
    let end = cursor();

    assert!(
        end.x > start.x,
        "cursor should have moved right: start.x={}, end.x={}",
        start.x,
        end.x
    );

    // Dropping removes the device; the control handle close tears down the child.
    drop(mouse);
    sleep(Duration::from_millis(200));
}
