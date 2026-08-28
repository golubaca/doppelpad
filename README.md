# doppelpad

Cross-platform virtual game controller library: byte-exact doubles of real
controllers, created entirely from user mode.

> **Status:** early development. APIs are not stable and nothing is usable yet.

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
| `doppelpad-linux` | Linux backend (`uhid`/`uinput`) |
| `doppelpad-macos` | macOS backend (planned) |
| `doppelpad-ffi` | C API bindings |

## Acknowledgements

The Windows driver approach builds on [WinUHid](https://github.com/cgutman/WinUHid)
by Cameron Gutman (MIT) and on techniques documented by
[HIDMaestro](https://github.com/hifihedgehog/HIDMaestro) (MIT).

## License

MIT. See [LICENSE](LICENSE).
