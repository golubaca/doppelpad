//! Creates a virtual mouse and nudges the cursor in a small square.
//!
//! Requires the DoppelPad driver to be installed (see `driver/windows/tools`).
//!
//! Run with: `cargo run -p doppelpad-windows --example mouse`

#[cfg(not(windows))]
fn main() {
    eprintln!("The mouse example is Windows-only.");
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::thread::sleep;
    use std::time::Duration;

    use doppelpad_windows::VirtualHidDeviceBuilder;

    // A standard 3-byte relative mouse report: [buttons, dx, dy].
    const MOUSE_DESCRIPTOR: &[u8] = &[
        0x05, 0x01, // Usage Page (Generic Desktop)
        0x09, 0x02, // Usage (Mouse)
        0xA1, 0x01, // Collection (Application)
        0x09, 0x01, //   Usage (Pointer)
        0xA1, 0x00, //   Collection (Physical)
        0x05, 0x09, //     Usage Page (Button)
        0x19, 0x01, //     Usage Minimum (1)
        0x29, 0x03, //     Usage Maximum (3)
        0x15, 0x00, //     Logical Minimum (0)
        0x25, 0x01, //     Logical Maximum (1)
        0x95, 0x03, //     Report Count (3)
        0x75, 0x01, //     Report Size (1)
        0x81, 0x02, //     Input (Data,Var,Abs)
        0x95, 0x01, //     Report Count (1)
        0x75, 0x05, //     Report Size (5)
        0x81, 0x03, //     Input (Const) padding
        0x05, 0x01, //     Usage Page (Generic Desktop)
        0x09, 0x30, //     Usage (X)
        0x09, 0x31, //     Usage (Y)
        0x15, 0x81, //     Logical Minimum (-127)
        0x25, 0x7F, //     Logical Maximum (127)
        0x75, 0x08, //     Report Size (8)
        0x95, 0x02, //     Report Count (2)
        0x81, 0x06, //     Input (Data,Var,Rel)
        0xC0, //   End Collection
        0xC0, // End Collection
    ];

    let mouse = VirtualHidDeviceBuilder::new(0x1209, 0x0001, MOUSE_DESCRIPTOR)
        .version_number(1)
        .build()?;
    println!(
        "virtual mouse created (interface v{})",
        mouse.interface_version()?
    );

    sleep(Duration::from_millis(500));

    let step: i8 = 12;
    let legs: [(i8, i8); 4] = [(step, 0), (0, step), (-step, 0), (0, -step)];
    for (dx, dy) in legs {
        for _ in 0..20 {
            mouse.submit_input_report(&[0x00, dx as u8, dy as u8])?;
            sleep(Duration::from_millis(8));
        }
    }

    println!("done; removing device");
    Ok(())
}
