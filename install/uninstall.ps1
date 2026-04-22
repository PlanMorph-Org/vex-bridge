#Requires -Version 5.1
<#
.SYNOPSIS
    Uninstalls Vex Atlas (vex CLI + vex-bridge daemon + Revit add-in) from
    Windows. Cleanly removes binaries, scheduled task, registry entries,
    PATH entry, and (optionally) all local configuration + paired keys.

.DESCRIPTION
    Reverses everything that install.ps1 and install-revit.ps1 set up:

      - Stops + unregisters the 'vex-bridge' Scheduled Task
      - Calls 'vex-bridge unpair' to revoke the device key with the server
      - Removes the Revit bundle from %ProgramData%\Autodesk\ApplicationPlugins
        and %APPDATA%\Autodesk\ApplicationPlugins
      - Removes %LOCALAPPDATA%\vex-bridge (binaries + logs)
      - Removes %APPDATA%\vex-bridge (config + state + token) when -Purge
      - Removes %LOCALAPPDATA%\vex-bridge\bin from the user PATH

    Per-user. No admin required (unless the Revit bundle was installed to
    %ProgramData%, in which case admin is needed only to remove that one
    folder — the script will tell you and skip it gracefully if not).

.PARAMETER Purge
    Also delete %APPDATA%\vex-bridge (config, state, access token,
    pairing key). Without this flag the daemon binaries are removed but
    your pairing identity stays — so reinstalling will pick up where you
    left off.

.PARAMETER Yes
    Skip the confirmation prompt. Useful for scripted uninstalls.

.EXAMPLE
    irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/uninstall.ps1 | iex

.EXAMPLE
    # Wipe everything, no questions asked:
    iex "& { $(irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/uninstall.ps1) } -Purge -Yes"
#>
[CmdletBinding()]
param(
    [switch] $Purge,
    [switch] $Yes
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$TaskName       = 'vex-bridge'
$CliInstallDir  = Join-Path $env:LOCALAPPDATA 'vex-bridge'
$CliBinDir      = Join-Path $CliInstallDir    'bin'
$DataDir        = Join-Path $env:APPDATA      'vex-bridge'
$BundleSystem   = Join-Path $env:ProgramData  'Autodesk\ApplicationPlugins\VexBridge.bundle'
$BundleUser     = Join-Path $env:APPDATA      'Autodesk\ApplicationPlugins\VexBridge.bundle'

# ── UI helpers ────────────────────────────────────────────────────────────────
function Banner {
    Write-Host ''
    Write-Host '  ┌─────────────────────────────────────────┐' -ForegroundColor DarkCyan
    Write-Host '  │           Uninstall Vex Atlas           │' -ForegroundColor Cyan
    Write-Host '  └─────────────────────────────────────────┘' -ForegroundColor DarkCyan
    Write-Host ''
}
function Step($m)  { Write-Host "  → $m" -ForegroundColor Cyan }
function Ok($m)    { Write-Host "    $([char]0x2713) $m" -ForegroundColor Green }
function Skip($m)  { Write-Host "    $([char]0x2022) $m" -ForegroundColor DarkGray }
function Warn($m)  { Write-Host "    ! $m" -ForegroundColor Yellow }

if ($env:OS -ne 'Windows_NT') {
    Write-Error 'Windows-only. On macOS/Linux: rm -rf the install dir + plist/systemd unit.'
    exit 1
}

Banner

# ── What we found ─────────────────────────────────────────────────────────────
$found = @()
if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) { $found += "Scheduled Task '$TaskName'" }
if (Test-Path $CliBinDir)    { $found += "CLI            $CliBinDir" }
if (Test-Path $BundleSystem) { $found += "Revit bundle   $BundleSystem" }
if (Test-Path $BundleUser)   { $found += "Revit bundle   $BundleUser" }
if (Test-Path $DataDir)      { $found += "Config + keys  $DataDir" }

if (-not $found) {
    Write-Host '  Nothing to remove. Vex Atlas does not appear to be installed.' -ForegroundColor Green
    Write-Host ''
    return
}

Write-Host '  The following will be removed:' -ForegroundColor White
foreach ($f in $found) { Write-Host "    • $f" -ForegroundColor Gray }
if (-not $Purge -and (Test-Path $DataDir)) {
    Write-Host ''
    Write-Host "  Your config + pairing key in $DataDir will be KEPT." -ForegroundColor DarkGray
    Write-Host '  Pass -Purge to delete them too.' -ForegroundColor DarkGray
}
Write-Host ''

if (-not $Yes) {
    $reply = Read-Host '  Continue? [y/N]'
    if ($reply -notmatch '^(y|yes)$') {
        Write-Host '  Aborted.' -ForegroundColor Yellow
        Write-Host ''
        return
    }
    Write-Host ''
}

# ── 1. Unpair (best effort) ───────────────────────────────────────────────────
Step 'Unpairing this device from architur'
$bridgeExe = $null
foreach ($candidate in @(
    (Join-Path $CliBinDir   'vex-bridge.exe'),
    (Join-Path $BundleSystem 'Contents\bin\vex-bridge.exe'),
    (Join-Path $BundleUser   'Contents\bin\vex-bridge.exe')
)) {
    if (Test-Path $candidate) { $bridgeExe = $candidate; break }
}
if ($bridgeExe) {
    try {
        & $bridgeExe unpair 2>&1 | Out-Null
        Ok 'Device key revoked'
    } catch {
        Warn "Could not reach server (already offline?). Continuing."
    }
} else {
    Skip 'vex-bridge.exe not found — nothing to unpair'
}

# ── 2. Stop + remove the scheduled task ───────────────────────────────────────
Step "Removing Scheduled Task '$TaskName'"
if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
    Stop-ScheduledTask    -TaskName $TaskName -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
    Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue
    Ok 'Task removed'
} else {
    Skip 'Task not registered'
}

# ── 3. Stop any lingering daemon process ──────────────────────────────────────
Step 'Stopping any running vex-bridge process'
$procs = Get-Process -Name 'vex-bridge' -ErrorAction SilentlyContinue
if ($procs) {
    $procs | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
    Ok "Stopped $($procs.Count) process(es)"
} else {
    Skip 'No daemon running'
}

# ── 4. Remove CLI install dir ─────────────────────────────────────────────────
Step "Removing CLI install dir"
if (Test-Path $CliInstallDir) {
    Remove-Item -Recurse -Force $CliInstallDir -ErrorAction SilentlyContinue
    if (Test-Path $CliInstallDir) {
        Warn "Some files locked: $CliInstallDir (will be cleaned on next reboot)"
    } else {
        Ok "Removed $CliInstallDir"
    }
} else {
    Skip 'Not installed'
}

# ── 5. Remove Revit bundle(s) ─────────────────────────────────────────────────
Step 'Removing Revit add-in bundle(s)'
$removed = 0
foreach ($b in @($BundleSystem, $BundleUser)) {
    if (Test-Path $b) {
        try {
            Remove-Item -Recurse -Force $b -ErrorAction Stop
            Ok "Removed $b"
            $removed++
        } catch {
            Warn "Could not remove $b — needs admin. Run: Remove-Item -Recurse -Force '$b'"
        }
    }
}
if ($removed -eq 0) { Skip 'No Revit bundle installed' }

# ── 6. Remove from user PATH ──────────────────────────────────────────────────
Step 'Cleaning user PATH'
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($userPath -and ($userPath -like "*$CliBinDir*")) {
    $cleaned = ($userPath -split ';' | Where-Object { $_ -and ($_ -ne $CliBinDir) }) -join ';'
    [Environment]::SetEnvironmentVariable('PATH', $cleaned, 'User')
    Ok 'Removed from PATH (takes effect in new shells)'
} else {
    Skip 'Not on PATH'
}

# ── 7. Purge data (opt-in) ────────────────────────────────────────────────────
if ($Purge) {
    Step "Purging config + pairing key"
    if (Test-Path $DataDir) {
        Remove-Item -Recurse -Force $DataDir -ErrorAction SilentlyContinue
        Ok "Removed $DataDir"
    } else {
        Skip 'No data to purge'
    }
} elseif (Test-Path $DataDir) {
    Step "Keeping config + pairing key"
    Skip "$DataDir (use -Purge to remove)"
}

# ── Done ──────────────────────────────────────────────────────────────────────
Write-Host ''
Write-Host '  ✓ Vex Atlas has been uninstalled.' -ForegroundColor Green
if (-not $Purge -and (Test-Path $DataDir)) {
    Write-Host '    (Your pairing identity is preserved — reinstall to keep using it.)' -ForegroundColor DarkGray
}
Write-Host ''
