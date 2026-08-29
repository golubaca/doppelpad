//! Cross-platform virtual game controller library.
//!
//! Set typed gamepad state and receive rumble, with the platform HID work behind
//! a [`Backend`]. The backend for the current platform (e.g. `doppelpad-windows`)
//! is wired in by the consumer.
//!
//! With a real backend (e.g. `doppelpad_windows::WindowsBackend`):
//!
//! ```
//! use doppelpad::{
//!     profiles::XBOX_ONE, Backend, Buttons, DeviceConfig, GamepadState, HidDevice, OutputSink,
//!     Result, VirtualGamepad,
//! };
//! # struct MyBackend;
//! # struct MyDevice;
//! # impl Backend for MyBackend {
//! #     type Device = MyDevice;
//! #     fn create_device(&self, _c: &DeviceConfig, _o: Option<OutputSink>) -> Result<MyDevice> {
//! #         Ok(MyDevice)
//! #     }
//! # }
//! # impl HidDevice for MyDevice {
//! #     fn submit_input_report(&self, _r: &[u8]) -> Result<()> { Ok(()) }
//! # }
//! # fn main() -> Result<()> {
//! let backend = MyBackend;
//! let pad = VirtualGamepad::new(&backend, &XBOX_ONE, |rumble| {
//!     let _ = (rumble.left_motor, rumble.right_motor);
//! })?;
//!
//! pad.set_state(&GamepadState {
//!     buttons: Buttons::A,
//!     ..Default::default()
//! })?;
//! # Ok(())
//! # }
//! ```

mod backend;
mod error;
mod gamepad;
mod profile;
pub mod profiles;
mod state;

pub use backend::{Backend, DeviceConfig, HidDevice, OutputSink};
pub use error::{Error, Result};
pub use gamepad::VirtualGamepad;
pub use profile::{GamepadProfile, MAX_REPORT_SIZE};
pub use state::{Buttons, Dpad, GamepadState, Rumble, Stick};
