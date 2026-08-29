<#
.SYNOPSIS
    Removes the DoppelPad virtual HID driver and, optionally, the signing cert.

.DESCRIPTION
    Idempotent. Removes any DoppelPad device nodes, unstages the driver package
    from the driver store, and (with -RemoveCert) removes the signing certificate
    from the machine stores. Requires an elevated session. Leaves no residue.

.PARAMETER RemoveCert
    Also remove the doppelpad signing certificate from the LocalMachine My, Root,
    and TrustedPublisher stores.
#>
[CmdletBinding()]
param(
    [switch] $RemoveCert
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Common.ps1')

Assert-Admin

Write-Host 'Removing device nodes...'
foreach ($instance in Get-DoppelPadDeviceInstances) {
    Write-Host "  $instance"
    & pnputil.exe /remove-device $instance | Out-Null
    if ($LASTEXITCODE -notin 0, 3010) {
        Write-Warning "pnputil /remove-device '$instance' returned $LASTEXITCODE"
    }
}

Write-Host 'Unstaging driver package...'
foreach ($name in Get-PublishedDriverNames) {
    Write-Host "  $name"
    & pnputil.exe /delete-driver $name /uninstall | Out-Null
    if ($LASTEXITCODE -notin 0, 3010) {
        Write-Warning "pnputil /delete-driver '$name' returned $LASTEXITCODE"
    }
}

if ($RemoveCert) {
    Write-Host 'Removing signing certificate...'
    Remove-CertFromStore -StoreName 'My'
    Remove-CertFromStore -StoreName 'Root'
    Remove-CertFromStore -StoreName 'TrustedPublisher'
}

$remaining = Get-DoppelPadDeviceInstances
if ($remaining) {
    Write-Warning "Devices still present: $($remaining -join ', ')"
} else {
    Write-Host 'Done. No DoppelPad devices remain.'
}
