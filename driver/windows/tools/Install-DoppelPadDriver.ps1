<#
.SYNOPSIS
    Installs the DoppelPad virtual HID driver using a self-signed certificate.

.DESCRIPTION
    Idempotent. Ensures a local code-signing certificate exists and is trusted,
    signs the driver catalog with it, stages the driver package, and creates the
    persistent root device node. Requires an elevated session. No reboot, no
    test-signing mode.

    The self-signed-certificate approach installs a certificate into the machine
    Root and TrustedPublisher stores. This is appropriate for development and for
    a controlled first-party install, but note it establishes a new trusted root
    on the machine; see tools/README.md.

.PARAMETER DriverDir
    Path to the built driver package directory. Defaults to
    build\<Configuration>\<Platform>\DoppelPadDriver relative to the driver root.
#>
[CmdletBinding()]
param(
    [string] $DriverDir,
    [string] $Configuration = 'Release',
    [string] $Platform = 'x64'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Common.ps1')

Assert-Admin
$pkgDir = Resolve-DriverDir -DriverDir $DriverDir -Configuration $Configuration -Platform $Platform
$inf = Join-Path $pkgDir $script:DoppelPadInfName
$cat = Join-Path $pkgDir 'doppelpaddriver.cat'
if (-not (Test-Path $cat)) { throw "Catalog not found: $cat" }

Write-Host 'Ensuring signing certificate...'
$cert = New-DoppelPadCert
Write-Host "  Using certificate $($cert.Thumbprint)"

Write-Host 'Trusting certificate (Root, TrustedPublisher)...'
Add-CertToStore -Cert $cert -StoreName 'Root'
Add-CertToStore -Cert $cert -StoreName 'TrustedPublisher'

Write-Host 'Signing driver catalog...'
$signtool = Get-KitTool -Name 'signtool.exe' -Root 'bin'
& $signtool sign /sm /s My /sha1 $cert.Thumbprint /fd SHA256 $cat
if ($LASTEXITCODE -ne 0) { throw "signtool failed with exit code $LASTEXITCODE" }

# Create the root device node before installing the driver: pnputil /install only
# binds to devices already present, so the node must exist first.
$existing = Get-DoppelPadDeviceInstances
if ($existing) {
    Write-Host "Device already present: $($existing -join ', ')"
} else {
    Write-Host 'Creating root device node...'
    $devgen = Get-KitTool -Name 'devgen.exe' -Root 'Tools'
    & $devgen /add /bus ROOT /hardwareid $script:DoppelPadHardwareId
    if ($LASTEXITCODE -ne 0) { throw "devgen failed with exit code $LASTEXITCODE" }
}

Write-Host 'Staging driver and binding it to the device...'
& pnputil.exe /add-driver $inf /install
# 0 = success, 3010 = success but reboot requested (not expected here).
if ($LASTEXITCODE -notin 0, 3010) { throw "pnputil /add-driver failed with exit code $LASTEXITCODE" }

Write-Host 'Done. Installed devices:'
foreach ($d in Get-DoppelPadDeviceInstances) {
    $status = (Get-PnpDevice -InstanceId $d -ErrorAction SilentlyContinue).Status
    Write-Host "  $d [$status]"
}
