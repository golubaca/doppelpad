//! Windows backend for doppelpad.
//!
//! A native Rust client for the DoppelPad UMDF2 virtual HID driver. It opens the
//! driver's control device and drives it over the ioctl interface defined in
//! `driver/windows/Public.h`: configure identity and HID report descriptor,
//! create and start the device, then submit input reports.
//!
//! This crate is Windows-only and requires the DoppelPad driver to be installed
//! (see `driver/windows/tools`).
//!
//! ```no_run
//! # #[cfg(windows)]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use doppelpad_windows::VirtualHidDeviceBuilder;
//!
//! let descriptor: &[u8] = &[/* HID report descriptor */];
//! let device = VirtualHidDeviceBuilder::new(0x1234, 0x5678, descriptor).build()?;
//! device.submit_input_report(&[/* report bytes */])?;
//! # Ok(())
//! # }
//! # #[cfg(not(windows))]
//! # fn main() {}
//! ```

#![cfg(windows)]

mod device;
mod error;
mod ioctl;

pub use device::{
    OutputCallback, OutputReport, SupportedEvents, VirtualHidDevice, VirtualHidDeviceBuilder,
};
pub use error::{Error, Result};
