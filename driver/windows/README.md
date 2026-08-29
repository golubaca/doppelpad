# DoppelPadDriver

UMDF2 driver that creates virtual HID devices through Microsoft's Virtual HID
Framework (VHF). This is the only non-Rust component of doppelpad; all device
logic lives in the userland crates, which talk to this driver over the ioctl
interface defined in `Public.h`.

## Provenance

Derived from the driver component of [WinUHid](https://github.com/cgutman/WinUHid)
by Cameron Gutman, MIT licensed (see the repository NOTICE file). Vendored as a
plain copy from upstream commit `d6cebbef5c7909168d1f881185be8f607d6aefd4`
(2025-05-28). Upstream is dormant; we maintain this copy and expect to diverge.

Source files keep their upstream names and internal symbols to stay diffable
against WinUHid. Externally visible identifiers are renamed so a doppelpad
install can never collide with a real WinUHid install:

| Item | Upstream | Here |
|------|----------|------|
| Control device | `\\.\WinUHid` | `\\.\DoppelPad` |
| Root enumerator | `Root\WinUHid` | `Root\DoppelPad` |
| UMDF service / binary | `WinUHidDriver` | `DoppelPadDriver` |
| INF / catalog | `WinUHidDriver.inf/.cat` | `DoppelPadDriver.inf/.cat` |
| WPP trace GUID | `0e97c6bb-...` | `c6614e05-005f-4c6b-b8c3-fcdc20d31313` |

`WinUHidApi.h` is a verbatim copy of upstream's public API header
(`WinUHid/WinUHid.h`); the driver needs its shared types (`WINUHID_EVENT`,
`WINUHID_EVENT_TYPE`, `WINUHID_REQUEST_ID`). The Rust client mirrors these
types; this header and `Public.h` are the wire-format source of truth.

Deliberate deviations from upstream:

- Win32 (x86) configurations dropped; targets are x64 and ARM64.
- Output directories are project-relative (`build/`), no solution file.

## Building

Verified toolchain (2026-08-29). Headless driver builds require the VS 2026
generation — VS 2022 Build Tools cannot host the WDK component:

- Visual Studio 2026 Build Tools (`winget install Microsoft.VisualStudio.BuildTools`)
  with the C++ workload, ARM64 tools, Spectre-mitigated runtimes, and the
  `Component.Microsoft.Windows.DriverKit.BuildTools` component (note the
  `.BuildTools` suffix; the plain component only exists for full VS).
- Windows SDK and WDK 10.0.28000 MSIs (`winget install
  Microsoft.WindowsSDK.10.0.28000 Microsoft.WindowsWDK.10.0.28000`).
  msbuild 18 requires the 28000-line kits; WDK 26100 only ships 17.0 build
  task DLLs.

```
& "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\MSBuild\Current\Bin\MSBuild.exe" DoppelPadDriver.vcxproj /p:Configuration=Release /p:Platform=x64
```

Output: `build/Release/x64/DoppelPadDriver/` containing the test-signed driver
DLL, stamped INF, and signed catalog, ready for the install tooling.

Known toolchain quirks (do not fail the build output):

- The integrated INF verification step errors with `Unable to load DLL
  'x86\InfVerif.dll'` (28000 kit packaging gap). Verify manually:
  `& "C:\Program Files (x86)\Windows Kits\10\Tools\10.0.26100.0\x64\infverif.exe" /v /u build\Release\x64\DoppelPadDriver\DoppelPadDriver.inf`
- A post-build packaging step wants `pwsh.exe` (PowerShell 7); without it the
  step is skipped after the package directory is already produced.

## Security notes

The INF restricts access to the control device to administrators, SYSTEM, and
UMDF drivers (`DoppelPad_AddReg` security descriptor). This is inherited from
upstream and must be preserved: an unrestricted virtual HID device would let
unprivileged processes inject input and bypass UIPI.
