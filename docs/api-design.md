# Core API design

Design for the platform-neutral `doppelpad` crate — the API NearZero and other
consumers use, with the platform HID work behind a backend trait. This is the
design record for issue #6; it is meant to be reviewed before the implementation
lands.

## Goals and constraints

- **Typed, high-level gamepad.** Consumers set button/stick/trigger state and
  receive rumble, without writing HID report descriptors or packing bytes.
- **Platform-neutral core, backends behind a trait.** `doppelpad-windows` is the
  first backend; Linux (`uinput`/`uhid`) and macOS follow without touching
  consumer code.
- **Per-frame submission is allocation-free.** NearZero receives controller
  state off the network at frame cadence (up to a few hundred Hz per pad) and
  forwards it; `set_state` must not allocate.
- **Low-latency output events.** Rumble is delivered via callback; the backend
  passes a borrowed slice (no per-event allocation).
- **No panics across the API.** Errors are values (`thiserror`).

## Crate architecture

```
doppelpad            (core)   traits + typed state + profiles. No platform code.
  ^
  | implements Backend
doppelpad-windows    (backend) wraps its VirtualHidDeviceBuilder.
doppelpad-linux      (backend) uinput/uhid (later).

consumer (NearZero)  depends on doppelpad + the platform backend, cfg-selected.
```

Dependency direction is one-way: backends depend on the core to implement its
trait; the core never depends on a backend, so there is no cycle. The consumer
wires a backend to the core — it already does per-platform selection, so this is
a `cfg` at the call site, e.g.:

```rust
#[cfg(windows)]
let backend = doppelpad_windows::WindowsBackend::new();
let pad = doppelpad::VirtualGamepad::new(&backend, &doppelpad::profiles::XBOX_ONE, on_rumble)?;
```

The low-level `VirtualHidDeviceBuilder` in `doppelpad-windows` stays public for
non-gamepad devices (the mouse example); the backend trait is a thin adapter
over it.

## Core types

```rust
/// XInput-range gamepad state (sticks i16, triggers u8) — the shape games and
/// XInput clients speak; profiles map it to their HID descriptor's ranges.
pub struct GamepadState {
    pub buttons: Buttons,          // bitflags: A B X Y, bumpers, stick clicks, start/back, guide
    pub dpad: Dpad,                // None | Up | UpRight | ... (8-way)
    pub left_stick: Stick,         // { x: i16, y: i16 }
    pub right_stick: Stick,
    pub left_trigger: u8,
    pub right_trigger: u8,
}

/// Decoded output event.
#[non_exhaustive]                  // more feedback fields can follow without a break
pub struct Rumble {
    pub left_motor: u8,            // 0..=255 low-frequency (strong) motor
    pub right_motor: u8,           // 0..=255 high-frequency (weak) motor
    pub left_trigger: u8,          // 0..=255 trigger motor; 0 if the profile has none
    pub right_trigger: u8,
}
```

`GamepadState` is `Copy` and `Default` (neutral: centered sticks, no buttons).

## Backend trait

Minimal HID transport. Everything gamepad-specific lives in the core's profiles,
not the backend.

```rust
pub trait Backend {
    type Device: HidDevice;
    fn create_device(&self, config: &DeviceConfig, output: Option<OutputSink>)
        -> Result<Self::Device>;
}

pub trait HidDevice: Send {
    /// Submit a raw HID input report. Called per frame; must not allocate.
    fn submit_input_report(&self, report: &[u8]) -> Result<()>;
}

pub struct DeviceConfig<'a> {
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: u16,
    pub report_descriptor: &'a [u8],
    pub hardware_ids: &'a [&'a str],
}

/// Invoked on the backend's reader thread for each output report, with a
/// borrowed slice (no allocation). report_id, then the report body.
pub type OutputSink = Box<dyn FnMut(u8, &[u8]) + Send>;
```

The Windows backend implements this by driving `VirtualHidDeviceBuilder`; its
reader calls the sink with a slice into its own buffer instead of allocating an
`OutputReport { Vec }` as it does today.

## Profiles

A profile is the byte-level knowledge for one controller identity: its HID
descriptor and USB identity, how to encode `GamepadState` into an input report,
and how to decode an output report into `Rumble`.

```rust
pub trait GamepadProfile: Sync {
    fn device_config(&self) -> DeviceConfig<'static>;
    /// Encode state into `out`; return the report length. No allocation.
    fn encode_input(&self, state: &GamepadState, out: &mut [u8]) -> usize;
    /// Decode an output report into rumble, or None if it carries nothing we map.
    fn decode_output(&self, report_id: u8, data: &[u8]) -> Option<Rumble>;
}
```

Profiles start hardcoded, one per file, beginning with `XBOX_ONE` (the identity
proven in the #5 spike: VID 045E / PID 02FF, `HID\VID_045E&PID_02FF&IG_00`).
Following HIDMaestro, later profiles may become data (descriptor + field map in
a table or JSON) so new controllers add data, not code — but that is out of
scope for M1.

## High-level API

```rust
pub struct VirtualGamepad<D: HidDevice> { /* device + profile */ }

impl<D: HidDevice> VirtualGamepad<D> {
    pub fn new<B, F>(backend: &B, profile: &'static dyn GamepadProfile, on_rumble: F)
        -> Result<Self>
    where B: Backend<Device = D>, F: FnMut(Rumble) + Send + 'static;

    /// Encode `state` into a stack buffer and submit it. No heap allocation.
    pub fn set_state(&self, state: &GamepadState) -> Result<()>;
}
```

`new` builds the `DeviceConfig` from the profile, wraps `on_rumble` in an
`OutputSink` that decodes via the profile, and creates the backend device.
`set_state` encodes into a fixed stack array (`[u8; 64]`, larger than any M1
report) and submits the used prefix — no per-frame allocation. The device is
removed when the `VirtualGamepad` drops.

## Error strategy

Core defines `doppelpad::Error` with `thiserror`, wrapping a backend error as an
opaque source so the core stays platform-neutral:

```rust
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("backend failure: {0}")]
    Backend(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("report buffer too small: need {needed}, have {have}")]
    ReportBufferTooSmall { needed: usize, have: usize },
}
```

`doppelpad-windows::Error` (already `std::error::Error`) boxes into
`Error::Backend`. `thiserror` is added to the core crate; it is the one small
dependency the core takes.

## Migration / minimal implementation for #6

1. Core crate gains `state`, `profile`, `backend`, `error`, and `gamepad`
   modules and the `XBOX_ONE` profile.
2. `doppelpad-windows` gains `WindowsBackend` implementing `Backend`, and its
   reader is changed to hand the sink a borrowed slice.
3. The `xbox_gamepad` example is rewritten to use the high-level core API
   (`VirtualGamepad` + `XBOX_ONE`), which is the acceptance check that the API is
   usable and matches the spike.

## Open questions for review

1. **Stick range.** Proposed XInput-style (`i16` sticks, `u8` triggers) so it
   matches what NearZero receives from clients. Alternative: normalized `f32`.
   XInput-style avoids a conversion round-trip; preferred unless NearZero already
   carries `f32`.
2. **Backend selection ergonomics.** Proposed: consumer `cfg`-selects the
   backend explicitly (one line). Alternative: a `doppelpad` facade feature that
   re-exports the platform backend — rejected for M1 because it reintroduces a
   core→backend dependency. OK to revisit if the call-site `cfg` proves annoying.
3. **Rumble motor semantics.** Proposed `left_motor`/`right_motor` as raw
   0..=255. Do we also surface trigger-motor magnitudes (the Xbox One report has
   them) now, or defer until a consumer needs them?
```
