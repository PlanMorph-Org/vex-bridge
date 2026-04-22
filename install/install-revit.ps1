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

    This is the path for users who don't want to deal with the unsigned-MSI
    SmartScreen warning. It's identical to what the MSI does, minus the
    installer wrapper.

.EXAMPLE
    irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/install-revit.ps1 | iex

.EXAMPLE
    # Install only for the current user (no admin needed at all):
    iex "& { $(irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/install-revit.ps1) } -PerUser"
#>
[CmdletBinding()]
param(
    [switch] $PerUser,
    [string] $Repo = $(if ($env:VEX_BRIDGE_GITHUB_REPO) { $env:VEX_BRIDGE_GITHUB_REPO } else { 'PlanMorph-Org/vex-bridge' })
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$TaskName  = 'vex-bridge'
$ZipName   = 'VexBridge-bundle.zip'

if ($PerUser) {
    $InstallRoot = Join-Path $env:APPDATA      'Autodesk\ApplicationPlugins'
} else {
    $InstallRoot = Join-Path $env:ProgramData  'Autodesk\ApplicationPlugins'
}
$BundleDir = Join-Path $InstallRoot 'VexBridge.bundle'
$BinDir    = Join-Path $BundleDir   'Contents\bin'

function Step($m) { Write-Host "==> $m" -ForegroundColor Cyan }
function Ok($m)   { Write-Host "    $([char]0x2713) $m" -ForegroundColor Green }
function Fail($m) { Write-Error "    $([char]0x2717) $m"; exit 1 }

if ($env:OS -ne 'Windows_NT') { Fail 'Windows-only.' }

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
New-Item -ItemType Directory -Force -Path $InstallRoot | Out-Null
Expand-Archive -Path $tmpZip -DestinationPath $InstallRoot -Force
Remove-Item -Force $tmpZip
if (-not (Test-Path (Join-Path $BinDir 'vex-bridge.exe'))) {
    Fail "Extracted bundle is missing vex-bridge.exe (looked in $BinDir)."
}
Ok "Bundle extracted."

# ── Scheduled Task — start vex-bridge at every login ──────────────────────────
Step "Registering Scheduled Task '$TaskName' (start at login)..."
Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue
$action  = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-WindowStyle Hidden -NoProfile -Command `"& '$(Join-Path $BinDir 'vex-bridge.exe')' start`"" -WorkingDirectory $BinDir
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

Write-Host ""
Write-Host "==> Done. Launch Revit — the Vex Atlas ribbon will appear under Add-Ins." -ForegroundColor Green
Write-Host ""
