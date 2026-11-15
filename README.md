# doppelpad

Cross-platform virtual game controller library: byte-exact doubles of real
controllers, created entirely from user mode.

> **Status:** the Windows backend and the Xbox One profile work: games see the
> virtual pad through XInput, with rumble, via Windows' inbox `xinputhid`
> driver. They ship in NearZero. The Linux, macOS and FFI crates only reserve
> their names so far. APIs are not stable yet.

## Goals

- Virtual gamepads that are indistinguishable from real hardware across input
  APIs (DirectInput, XInput, Windows.Gaming.Input, SDL, browser Gamepad).
- No kernel-mode driver of our own. On Windows this means a small UMDF2 driver
  on Microsoft's Virtual HID Framework; on Linux, plain `uhid`/`uinput`.
- Rust core with an optional C API for non-Rust consumers.
- Permissive licensing (MIT) throughout, including the driver component.

Windows support comes first, Linux next, macOS later.

## Workspace

| Crate | Purpose |
|-------|---------|
| `doppelpad` | Core library: device profiles, report handling, public API |
| `doppelpad-windows` | Windows backend (UMDF2/VHF driver client) |
| `doppelpad-linux` | Linux backend (`uhid`/`uinput`) (planned) |
| `doppelpad-macos` | macOS backend (planned) |
| `doppelpad-ffi` | C API bindings (planned) |

## Getting started (Windows)

1. Build the driver: see [driver/windows/README.md](driver/windows/README.md).
2. Install it from an elevated PowerShell:
   `driver\windows\tools\Install-DoppelPadDriver.ps1`. The tooling creates a
   self-signed, non-exportable signing key on each machine and trusts it in the
   machine `Root` store, so no key is ever shared between machines. See
   [driver/windows/tools/README.md](driver/windows/tools/README.md).
3. Run the example, also elevated:
   `cargo run -p doppelpad-windows --example xbox_gamepad`.

## Acknowledgements

The Windows driver approach builds on [WinUHid](https://github.com/cgutman/WinUHid)
by Cameron Gutman (MIT) and on techniques documented by
[HIDMaestro](https://github.com/hifihedgehog/HIDMaestro) (MIT).

## License

MIT. See [LICENSE](LICENSE).
