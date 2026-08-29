# Spike: virtual Xbox controller via the `xinputhid` route

Decision record for issue #5. Answers whether a pure-HID virtual gamepad can
deliver XInput (with rumble) without an XUSB companion device.

## Result: the pure-HID `xinputhid` route works. No XUSB companion needed for M1.

A virtual HID gamepad with the Xbox One identity is recognized by XInput, reads
back correct input, and receives rumble, using only our existing UMDF/VHF driver
plus Windows' inbox `xinputhid.sys`. Verified end to end by
`examples/xbox_gamepad.rs` on Windows 11 (build 26100).

## How it works

1. The device advertises VID `045E` / PID `02FF` and the extra hardware ID
   `HID\VID_045E&PID_02FF&IG_00` (set via `hardware_ids`).
2. `C:\Windows\INF\xinputhid.inf` `[GIP_Hid]` matches that hardware ID (its
   `%GIP_Hid.DeviceDesc%` line is literally `HID\VID_045E&PID_02FF&IG_00`).
   Windows binds `xinputhid.sys` as an upper filter on the HID child.
3. `xinputhid.sys` synthesizes the XInput/GameInput view from the HID reports:
   input reports become `XInputGetState` reads; `XInputSetState` rumble becomes
   HID output reports delivered back to our `on_output` callback.

The full list of PIDs `xinputhid.inf` matches (all `VID_045E ... &IG_00`):
`02D1 02DD 02E3 02EA 0B00 0B0A 0B13 02FF`.

## What was verified

| Check | Result |
|-------|--------|
| XInput enumeration (`XInputGetState`) | Device appears in slot 0 |
| Input round-trip | Button bit 0 → `XINPUT_GAMEPAD_A` (0x1000) read back |
| Rumble round-trip | `XInputSetState(40000,40000)` → output report with left/right motor magnitude `0x3D` (61 = 40000/65535·100) reached our callback |

## Findings and their consequences

- **No ContainerID slot-skip.** HIDMaestro documents a bug where root-enumerated
  devices get a null-sentinel ContainerID that makes `xinput1_4` skip slot 0.
  It did **not** occur here: the device landed in slot 0 directly. We did not
  need to set a non-null container ID. (The builder exposes `container_id(...)`
  if a future case needs it.)

- **`xinputhid` streams continuous keep-alive output reports.** Even with no
  active rumble, `xinputhid` writes an all-zero-magnitude output report several
  times per second (`[0F,00,00,00,00,FF,00,EB]`: all motors "enabled", zero
  magnitude). Only the byte-3/4 motor magnitudes carry real rumble. **Consumers
  (NearZero) must dedupe output reports and forward only on change**, or they
  will spam the network with no-op rumble. The example demonstrates this by
  printing only on change and tracking peak magnitude.

- **Output report layout** (8 bytes, ported from WinUHid's Xbox One profile):
  `[MotorsEnabled, LeftTrigger, RightTrigger, LeftMotor, RightMotor, Duration,
  Delay, Repeat]`. Rumble magnitudes are the 0..100 range from the PID output
  collection's logical maximum.

- **The control handle must use overlapped I/O.** The output-report reader keeps
  a `GET_NEXT_EVENT` request pending indefinitely. On a synchronous handle,
  Windows serializes all I/O, so that pending read blocks `submit_input_report`
  (a `WriteFile` on the same handle) until the next output report arrives —
  which, once no XInput client is polling, may be never. The first draft did
  this and hung. The client now opens the handle with `FILE_FLAG_OVERLAPPED` and
  drives every call through overlapped completion, so input submission and event
  reading are independent, and the reader stops cleanly via an event object.

## Decision

For NearZero's needs (a virtual Xbox controller usable by games through XInput,
with rumble forwarded to the remote client), the `xinputhid` route is
sufficient. An XUSB companion device (for the Xbox 360 wired identity, PID
`028E`) is **not** required for M1 and is deferred. Reconsider it only if a
consumer specifically needs the 360-wired identity or DirectInput trigger
behavior that the `xinputhid` path does not provide.

## Not yet verified (follow-ups)

The spike proved the XInput path, which is the acceptance bar. These remain to
confirm, expected to work per HIDMaestro's documentation of the same route:

- Visibility in `joy.cpl`, SDL3/HIDAPI, the browser Gamepad API, and
  Windows.Gaming.Input.
- Multiple simultaneous controllers and slot ordering.
- Same-boot create/remove cycles (HIDMaestro documents a sticky-PnP fast-path
  needing a per-process instance-id suffix; our devgen-based creation may or may
  not hit it — untested here).
