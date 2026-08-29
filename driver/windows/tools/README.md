# Driver install tooling

PowerShell scripts that install and remove the DoppelPad virtual HID driver on a
development machine using a self-signed certificate. No reboot, no test-signing
mode. All scripts require an elevated (Administrator) session and are idempotent.

## Scripts

| Script | Purpose |
|--------|---------|
| `Install-DoppelPadDriver.ps1` | Ensure the signing cert, trust it, sign the catalog, create the root device node, stage and bind the driver. |
| `Uninstall-DoppelPadDriver.ps1` | Remove the device node(s) and driver package; with `-RemoveCert`, also remove the certificate. |
| `Common.ps1` | Shared helpers (dot-sourced by the other scripts; not run directly). |

## Usage

Build the driver first (see [../README.md](../README.md)), then from an elevated
PowerShell prompt:

```powershell
# install (uses build\Release\x64\DoppelPadDriver by default)
.\Install-DoppelPadDriver.ps1

# install a specific build output
.\Install-DoppelPadDriver.ps1 -DriverDir C:\path\to\DoppelPadDriver

# remove everything including the certificate
.\Uninstall-DoppelPadDriver.ps1 -RemoveCert
```

A successful install ends with the enumerator device reported as `[OK]`:

```
Done. Installed devices:
  ROOT\DEVGEN\{...} [OK]
```

The device is the control enumerator that hosts the UMDF driver. Virtual HID
controllers appear once a client opens the control device and requests them; that
is the job of the `doppelpad-windows` crate, not this tooling.

## How it works

1. **Certificate.** A self-signed code-signing certificate
   (`CN=doppelpad Test Signing Certificate`) is created in `LocalMachine\My` and
   its public part is copied into `Root` and `TrustedPublisher`.
2. **Sign.** The driver catalog (`.cat`) is signed with that certificate. The
   catalog is the trust anchor for the package; the file hashes it contains are
   produced at build time and are not modified here.
3. **Device.** A persistent root device node carrying the hardware ID
   `Root\DoppelPad` is created with `devgen /bus ROOT`. It must exist before the
   driver is installed, because `pnputil /install` only binds to devices that are
   already present.
4. **Driver.** `pnputil /add-driver ... /install` stages the package in the
   driver store and binds it to the device, which starts the UMDF driver.

## Security note

The self-signed-certificate approach adds a new trusted root certificate to the
machine's `Root` store so Windows accepts the catalog signature without a prompt
or test-signing mode. This is appropriate for development and for a controlled
first-party install, but a certificate in the machine `Root` store can vouch for
any code signed by its private key. The key is generated locally and marked
non-exportable. `Uninstall-DoppelPadDriver.ps1 -RemoveCert` removes it. A
production distribution should revisit this in favor of an attestation-signed
package; see the project notes.
