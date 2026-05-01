#Requires -Version 5.1
<#
.SYNOPSIS
    Installs the Vex Atlas Revit add-in (vex + vex-bridge + .addin DLLs)
    with NO MSI, NO Authenticode certificate, NO admin, and NO SmartScreen
    prompts.

.DESCRIPTION
    1. Downloads VexBridge-bundle.zip from the latest GitHub Release.
    2. Extracts it to %ProgramData%\Autodesk\ApplicationPlugins\VexBridge.bundle
       (or %APPDATA%\Autodesk\ApplicationPlugins if -PerUser is passed).
    3. Registers a per-user Scheduled Task to start vex-bridge at login.
    4. Starts the daemon now and opens the pairing page in the browser.

    If the default Autodesk folder is missing, the installer offers a
    per-user fallback and can open a folder picker. Use -Browse to show the
    picker immediately after default-path detection fails, or -InstallRoot to
    point directly at an Autodesk ApplicationPlugins folder.

    This is the path for users who don't want to deal with the unsigned-MSI
    SmartScreen warning. It's identical to what the MSI does, minus the
    installer wrapper.

.EXAMPLE
    irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/install-revit.ps1 | iex

.EXAMPLE
    # Install only for the current user (no admin needed at all):
    iex "& { $(irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/install-revit.ps1) } -PerUser"

.EXAMPLE
    # Browse for a custom Autodesk ApplicationPlugins folder if needed:
    iex "& { $(irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/install-revit.ps1) } -Browse"

.EXAMPLE
    # Install into an explicit ApplicationPlugins folder:
    iex "& { $(irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/install-revit.ps1) } -InstallRoot 'D:\Autodesk\ApplicationPlugins'"
#>
[CmdletBinding()]
param(
    [switch] $PerUser,
    [switch] $Browse,
    [string] $InstallRoot,
    [string] $Repo = $(if ($env:VEX_BRIDGE_GITHUB_REPO) { $env:VEX_BRIDGE_GITHUB_REPO } else { 'PlanMorph-Org/vex-bridge' })
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$TaskName = 'vex-bridge'
$ZipName  = 'VexBridge-bundle.zip'

function Step($m) { Write-Host "==> $m" -ForegroundColor Cyan }
function Ok($m)   { Write-Host "    $([char]0x2713) $m" -ForegroundColor Green }
function Fail($m) { Write-Error "    $([char]0x2717) $m"; exit 1 }

function Normalize-InstallRoot([string] $Path) {
    $expanded = [Environment]::ExpandEnvironmentVariables($Path).TrimEnd('\')
    if ([IO.Path]::GetFileName($expanded) -ieq 'VexBridge.bundle') {
        return Split-Path -Parent $expanded
    }
    if ([IO.Path]::GetFileName($expanded) -ieq 'Autodesk') {
        return Join-Path $expanded 'ApplicationPlugins'
    }
    return $expanded
}

function Select-InstallRoot([string] $InitialPath) {
    if (-not [Environment]::UserInteractive) { return $null }
    try {
        Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
        $dlg = New-Object System.Windows.Forms.FolderBrowserDialog
        $dlg.Description = 'Choose the Autodesk ApplicationPlugins folder. The installer will create VexBridge.bundle inside it.'
        $dlg.SelectedPath = $InitialPath
        $dlg.ShowNewFolderButton = $true
        if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
            return Normalize-InstallRoot $dlg.SelectedPath
        }
    } catch {
        Write-Warning "Folder picker was not available: $($_.Exception.Message)"
    }
    return $null
}

function Resolve-InstallRoot {
    $machineRoot = Join-Path $env:ProgramData 'Autodesk\ApplicationPlugins'
    $userRoot    = Join-Path $env:APPDATA     'Autodesk\ApplicationPlugins'

    if ($InstallRoot) { return Normalize-InstallRoot $InstallRoot }
    if ($PerUser)     { return $userRoot }

    $machineAutodesk = Join-Path $env:ProgramData 'Autodesk'
    if ((Test-Path $machineAutodesk) -or (Test-Path $machineRoot)) {
        return $machineRoot
    }

    Write-Warning "Default Autodesk folder was not found at $machineAutodesk."
    if ($Browse) {
        $picked = Select-InstallRoot $machineRoot
        if ($picked) { return $picked }
    }

    if ([Environment]::UserInteractive) {
        $choice = Read-Host "Press Enter to install per-user at $userRoot, or type B to browse"
        if ($choice -match '^[Bb]') {
            $picked = Select-InstallRoot $machineRoot
            if ($picked) { return $picked }
        }
    }

    return $userRoot
}

if ($env:OS -ne 'Windows_NT') { Fail 'Windows-only.' }

$InstallRoot = Resolve-InstallRoot
$BundleDir   = Join-Path $InstallRoot 'VexBridge.bundle'
$BinDir      = Join-Path $BundleDir   'Contents\bin'

# ── Download the bundle ───────────────────────────────────────────────────────
Step "Fetching latest release from $Repo ..."
$rel = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ 'User-Agent' = 'vex-installer' }
$asset = $rel.assets | Where-Object { $_.name -eq $ZipName } | Select-Object -First 1
if (-not $asset) { Fail "Release $($rel.tag_name) has no asset named $ZipName." }
$tmpZip = Join-Path $env:TEMP $ZipName
Step "Downloading $($asset.name) ($([math]::Round($asset.size / 1MB, 1)) MiB) ..."
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $tmpZip -UseBasicParsing
Ok "Downloaded $tmpZip"

# ── Extract ───────────────────────────────────────────────────────────────────
Step "Installing to $BundleDir ..."
if (Test-Path $BundleDir) {
    # Stop the existing daemon so we can replace the binaries.
    Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
    Remove-Item -Recurse -Force $BundleDir
}
try {
    New-Item -ItemType Directory -Force -Path $InstallRoot | Out-Null
} catch {
    if ($PerUser -or $InstallRoot.StartsWith($env:APPDATA, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw
    }
    Write-Warning "Could not write to $InstallRoot ($($_.Exception.Message)). Falling back to per-user install."
    $InstallRoot = Join-Path $env:APPDATA 'Autodesk\ApplicationPlugins'
    $BundleDir   = Join-Path $InstallRoot 'VexBridge.bundle'
    $BinDir      = Join-Path $BundleDir   'Contents\bin'
    New-Item -ItemType Directory -Force -Path $InstallRoot | Out-Null
}
Expand-Archive -Path $tmpZip -DestinationPath $InstallRoot -Force
Remove-Item -Force $tmpZip
if (-not (Test-Path (Join-Path $BinDir 'vex-bridge.exe'))) {
    Fail "Extracted bundle is missing vex-bridge.exe (looked in $BinDir)."
}
Ok "Bundle extracted."

# ── Scheduled Task — start vex-bridge at every login ──────────────────────────
Step "Registering Scheduled Task '$TaskName' (start at login)..."
Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue

# Hidden launcher — see install.ps1 for the rationale. Dropping a .vbs into
# the bundle bin dir and invoking it via wscript.exe is the only Windows
# pattern that NEVER flashes a console window at logon. Powershell with
# -WindowStyle Hidden does NOT cover the child console app's own conhost.
$BridgeExe = Join-Path $BinDir 'vex-bridge.exe'
$LaunchVbs = Join-Path $BinDir 'vex-bridge-launch.vbs'
@"
Set WshShell = CreateObject("WScript.Shell")
WshShell.Run """$BridgeExe""" & " start", 0, False
"@ | Out-File -FilePath $LaunchVbs -Encoding ASCII -Force

$action  = New-ScheduledTaskAction -Execute 'wscript.exe' -Argument "`"$LaunchVbs`"" -WorkingDirectory $BinDir
$trigger = New-ScheduledTaskTrigger -AtLogOn -User "$env:USERDOMAIN\$env:USERNAME"
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -RestartCount 5 -RestartInterval (New-TimeSpan -Minutes 1) -StartWhenAvailable
$principal = New-ScheduledTaskPrincipal -UserId "$env:USERDOMAIN\$env:USERNAME" -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null
Ok "Task registered."

Step "Starting daemon ..."
Start-ScheduledTask -TaskName $TaskName
Start-Sleep -Seconds 3
Ok "Daemon started."

# ── Pair this device — opens the browser ──────────────────────────────────────
Step "Opening pairing page in your browser ..."
& (Join-Path $BinDir 'vex-bridge.exe') pair --device-label $env:COMPUTERNAME --open-browser

$Guide = Join-Path $BundleDir 'Contents\Resources\EarlyAccessInstall.html'
if (Test-Path $Guide) {
    Step "Opening install guide ..."
    Start-Process $Guide
}

Write-Host ""
Write-Host "==> Done. Launch Revit — the Vex Atlas ribbon will appear under Add-Ins." -ForegroundColor Green
Write-Host "    Installed bundle: $BundleDir"
Write-Host "    Account setup:    https://studio.planmorph.software/register"
Write-Host "    Sign in:          https://studio.planmorph.software/login"
Write-Host ""
