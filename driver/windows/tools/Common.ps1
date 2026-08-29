# Shared helpers for the DoppelPad driver install tooling.
# Dot-source this file; do not run it directly.

Set-StrictMode -Version Latest

# Identity of the virtual HID enumerator, must match DoppelPadDriver.inf.
$script:DoppelPadHardwareId = 'Root\DoppelPad'
$script:DoppelPadInfName = 'DoppelPadDriver.inf'

# Subject of the self-signed certificate used to sign the driver catalog.
$script:DoppelPadCertSubject = 'CN=doppelpad Test Signing Certificate'

function Assert-Admin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'This script must run in an elevated (Administrator) PowerShell session.'
    }
}

# Newest matching tool under the Windows Kits, for the current architecture.
function Get-KitTool {
    param(
        [Parameter(Mandatory)] [string] $Name,
        [ValidateSet('bin', 'Tools')] [string] $Root = 'bin'
    )
    $arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
    $base = "${env:ProgramFiles(x86)}\Windows Kits\10\$Root"
    $hit = Get-ChildItem "$base\*\$arch\$Name" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $hit) { throw "Could not find $Name under $base. Is the Windows SDK/WDK installed?" }
    return $hit.FullName
}

# Locate the built driver package directory (containing the DLL, INF, and CAT).
function Resolve-DriverDir {
    param([string] $DriverDir, [string] $Configuration = 'Release', [string] $Platform = 'x64')
    if ($DriverDir) {
        $resolved = (Resolve-Path $DriverDir -ErrorAction Stop).Path
    } else {
        $root = Split-Path $PSScriptRoot -Parent
        $resolved = Join-Path $root "build\$Configuration\$Platform\DoppelPadDriver"
    }
    $inf = Join-Path $resolved $script:DoppelPadInfName
    if (-not (Test-Path $inf)) {
        throw "Driver package not found at $resolved (missing $script:DoppelPadInfName). Build the driver first."
    }
    return $resolved
}

# The published driver-store name(s) (oemNN.inf) for our INF, or empty if none.
function Get-PublishedDriverNames {
    $out = & pnputil.exe /enum-drivers 2>$null
    $names = @()
    $current = $null
    foreach ($line in $out) {
        if ($line -match 'Published Name:\s*(oem\d+\.inf)') { $current = $Matches[1] }
        elseif ($line -match 'Original Name:\s*(.+)') {
            if ($current -and $Matches[1].Trim() -ieq $script:DoppelPadInfName) { $names += $current }
            $current = $null
        }
    }
    return $names
}

# Instance IDs of present devices carrying our hardware ID, or empty if none.
# Matches by hardware ID rather than instance path, since a devgen-created node
# is enumerated under ROOT\DEVGEN\{guid} with Root\DoppelPad as a hardware ID.
# Uses pnputil's device-id filter for a single fast query.
function Get-DoppelPadDeviceInstances {
    $out = & pnputil.exe /enum-devices /deviceid $script:DoppelPadHardwareId 2>$null
    $instances = foreach ($line in $out) {
        if ($line -match '^\s*Instance ID:\s*(.+?)\s*$') { $Matches[1] }
    }
    return @($instances | Select-Object -Unique)
}

# The signing certificate (with private key) from LocalMachine\My, or $null.
function Get-DoppelPadCert {
    Get-ChildItem Cert:\LocalMachine\My -ErrorAction SilentlyContinue |
        Where-Object { $_.Subject -eq $script:DoppelPadCertSubject -and $_.HasPrivateKey } |
        Sort-Object NotAfter -Descending | Select-Object -First 1
}

# Create the self-signed code-signing certificate in LocalMachine\My (idempotent).
function New-DoppelPadCert {
    $existing = Get-DoppelPadCert
    if ($existing) { return $existing }
    New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject $script:DoppelPadCertSubject `
        -KeyUsage DigitalSignature `
        -KeyExportPolicy NonExportable `
        -HashAlgorithm SHA256 `
        -CertStoreLocation Cert:\LocalMachine\My `
        -NotAfter (Get-Date).AddYears(10) `
        -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3')
}

# Ensure the cert's public part is trusted in the given LocalMachine store (idempotent).
function Add-CertToStore {
    param([Parameter(Mandatory)] $Cert, [Parameter(Mandatory)] [string] $StoreName)
    $store = [Security.Cryptography.X509Certificates.X509Store]::new(
        $StoreName, [Security.Cryptography.X509Certificates.StoreLocation]::LocalMachine)
    $store.Open('ReadWrite')
    try {
        if (-not $store.Certificates.Find('FindByThumbprint', $Cert.Thumbprint, $false).Count) {
            $store.Add($Cert)
        }
    } finally { $store.Close() }
}

# Remove any doppelpad signing certs (by subject) from the given LocalMachine store.
function Remove-CertFromStore {
    param([Parameter(Mandatory)] [string] $StoreName)
    $store = [Security.Cryptography.X509Certificates.X509Store]::new(
        $StoreName, [Security.Cryptography.X509Certificates.StoreLocation]::LocalMachine)
    $store.Open('ReadWrite')
    try {
        foreach ($c in @($store.Certificates)) {
            if ($c.Subject -eq $script:DoppelPadCertSubject) { $store.Remove($c) }
        }
    } finally { $store.Close() }
}
