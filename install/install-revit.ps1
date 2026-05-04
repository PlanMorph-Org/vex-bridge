#Requires -Version 5.1
<#
.SYNOPSIS
    Installs the Vex Atlas Revit add-in (vex + vex-bridge + .addin DLLs)
    with NO MSI and NO Authenticode certificate. Machine-wide installs use
    Autodesk's Program Files AddIns folder and may require an elevated shell;
    pass -PerUser for a no-admin fallback.

.DESCRIPTION
     1. Downloads VexBridge-bundle.zip from the latest GitHub Release.
     2. Extracts Revit add-in payloads to each supported AddIns folder:
         C:\Program Files\Autodesk\Revit {year}\AddIns\VexBridge
         (or %APPDATA%\Autodesk\Revit\Addins\{year} if -PerUser is passed).
    3. Registers a per-user Scheduled Task to start vex-bridge at login.
    4. Starts the daemon now and opens the pairing page in the browser.

    If the default Revit AddIns folder is missing, the installer offers a
    per-user fallback and can open a folder picker. Use -Browse to show the
    picker immediately after default-path detection fails, or -InstallRoot to
    point directly at a Revit AddIns folder.

    This is the path for users who don't want to deal with the unsigned-MSI
    SmartScreen warning. It's identical to what the MSI does, minus the
    installer wrapper.

.EXAMPLE
    irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/install-revit.ps1 | iex

.EXAMPLE
    # Install only for the current user (no admin needed at all):
    iex "& { $(irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/install-revit.ps1) } -PerUser"

.EXAMPLE
    # Browse for a custom Revit AddIns folder if needed:
    iex "& { $(irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/install-revit.ps1) } -Browse"

.EXAMPLE
    # Install one explicit Revit AddIns folder:
    iex "& { $(irm https://github.com/PlanMorph-Org/vex-bridge/releases/latest/download/install-revit.ps1) } -RevitVersions 2027 -InstallRoot 'C:\Program Files\Autodesk\Revit 2027\AddIns'"
#>
[CmdletBinding()]
param(
    [switch] $PerUser,
    [switch] $Browse,
    [string] $InstallRoot,
    [string[]] $RevitVersions = @('2022', '2023', '2024', '2025', '2026', '2027'),
    [string] $Repo = $(if ($env:VEX_BRIDGE_GITHUB_REPO) { $env:VEX_BRIDGE_GITHUB_REPO } else { 'PlanMorph-Org/vex-bridge' })
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$TaskName = 'vex-bridge'
$ZipName  = 'VexBridge-bundle.zip'

function Step($m) { Write-Host "==> $m" -ForegroundColor Cyan }
function Ok($m)   { Write-Host "    $([char]0x2713) $m" -ForegroundColor Green }
function Fail($m) { Write-Error "    $([char]0x2717) $m"; exit 1 }

function Normalize-InstallRoot([string] $Path, [string] $Version) {
    $expanded = [Environment]::ExpandEnvironmentVariables($Path).TrimEnd('\')
    if ([IO.Path]::GetFileName($expanded) -ieq 'VexBridge') {
        return Split-Path -Parent $expanded
    }
    if ([IO.Path]::GetFileName($expanded) -ieq "Revit $Version") {
        return Join-Path $expanded 'AddIns'
    }
    if ([IO.Path]::GetFileName($expanded) -ieq 'Autodesk') {
        return Join-Path $expanded "Revit $Version\AddIns"
    }
    return $expanded
}

function Select-InstallRoot([string] $InitialPath, [string] $Version) {
    if (-not [Environment]::UserInteractive) { return $null }
    try {
        Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
        $dlg = New-Object System.Windows.Forms.FolderBrowserDialog
        $dlg.Description = "Choose the Autodesk Revit $Version AddIns folder. The installer will create VexBridge inside it."
        $dlg.SelectedPath = $InitialPath
        $dlg.ShowNewFolderButton = $true
        if ($dlg.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
            return Normalize-InstallRoot $dlg.SelectedPath $Version
        }
    } catch {
        Write-Warning "Folder picker was not available: $($_.Exception.Message)"
    }
    return $null
}

function Resolve-InstallRoot([string] $Version) {
    $machineRoot = Join-Path $env:ProgramFiles "Autodesk\Revit $Version\AddIns"
    $userRoot    = Join-Path $env:APPDATA     "Autodesk\Revit\Addins\$Version"

    if ($InstallRoot) { return Normalize-InstallRoot $InstallRoot $Version }
    if ($PerUser)     { return $userRoot }

    $machineRevit = Split-Path -Parent $machineRoot
    if ((Test-Path $machineRevit) -or (Test-Path $machineRoot)) {
        return $machineRoot
    }

    Write-Warning "Default Revit $Version folder was not found at $machineRevit."
    if ($Browse) {
        $picked = Select-InstallRoot $machineRoot $Version
        if ($picked) { return $picked }
    }

    if ([Environment]::UserInteractive) {
        $choice = Read-Host "Press Enter to install per-user at $userRoot, or type B to browse"
        if ($choice -match '^[Bb]') {
            $picked = Select-InstallRoot $machineRoot $Version
            if ($picked) { return $picked }
        }
    }

    return $userRoot
}

if ($env:OS -ne 'Windows_NT') { Fail 'Windows-only.' }

if ($InstallRoot -and $RevitVersions.Count -ne 1) {
    Fail 'Use -InstallRoot together with exactly one -RevitVersions value.'
}

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
$extractDir = Join-Path $env:TEMP ("vex-bridge-bundle-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
Expand-Archive -Path $tmpZip -DestinationPath $extractDir -Force
Remove-Item -Force $tmpZip

$BundleDir = Join-Path $extractDir 'VexBridge.bundle'
if (-not (Test-Path $BundleDir)) {
    $BundleDir = Get-ChildItem $extractDir -Directory -Filter 'VexBridge.bundle' -Recurse | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $BundleDir -or -not (Test-Path $BundleDir)) { Fail 'Extracted ZIP is missing VexBridge.bundle.' }

$BundleBin = Join-Path $BundleDir 'Contents\bin'
if (-not (Test-Path (Join-Path $BundleBin 'vex-bridge.exe'))) {
    Fail "Extracted bundle is missing vex-bridge.exe (looked in $BundleBin)."
}

$InstalledAddIns = @()
foreach ($version in $RevitVersions) {
    $InstallRoot = Resolve-InstallRoot $version
    $AddInDir    = Join-Path $InstallRoot 'VexBridge'
    $BinDir      = Join-Path $AddInDir    'bin'
    $RootAddIn   = Join-Path $InstallRoot 'VexBridgeRevit.addin'
    $RevitPayload = Join-Path $BundleDir "Contents\Revit\$version"

    Step "Installing Revit $version add-in to $AddInDir ..."
    if (-not (Test-Path $RevitPayload)) { Fail "Bundle is missing Revit $version payload (looked in $RevitPayload)." }
    if (Test-Path $AddInDir) {
        Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 1
        Remove-Item -Recurse -Force $AddInDir
    }

    try {
        New-Item -ItemType Directory -Force -Path $InstallRoot | Out-Null
    } catch {
        if ($PerUser -or $InstallRoot.StartsWith($env:APPDATA, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw
        }
        Write-Warning "Could not write to $InstallRoot ($($_.Exception.Message)). Falling back to per-user Revit Addins install."
        $InstallRoot = Join-Path $env:APPDATA "Autodesk\Revit\Addins\$version"
        $AddInDir    = Join-Path $InstallRoot 'VexBridge'
        $BinDir      = Join-Path $AddInDir    'bin'
        $RootAddIn   = Join-Path $InstallRoot 'VexBridgeRevit.addin'
        New-Item -ItemType Directory -Force -Path $InstallRoot | Out-Null
    }

    New-Item -ItemType Directory -Force -Path $AddInDir, $BinDir | Out-Null
    Copy-Item (Join-Path $RevitPayload '*') $AddInDir -Recurse -Force
    Copy-Item (Join-Path $BundleBin '*') $BinDir -Recurse -Force
    $resources = Join-Path $BundleDir 'Contents\Resources'
    if (Test-Path $resources) {
        Copy-Item $resources $AddInDir -Recurse -Force
    }

    $manifest = Get-Content (Join-Path $RevitPayload 'VexBridgeRevit.addin') -Raw
    $manifest = $manifest -replace '<Assembly>.*?</Assembly>', '<Assembly>VexBridge\VexBridgeRevit.dll</Assembly>'
    $manifest | Out-File -FilePath $RootAddIn -Encoding UTF8 -Force
    $InstalledAddIns += [pscustomobject]@{ Version = $version; RootAddIn = $RootAddIn; AddInDir = $AddInDir; BinDir = $BinDir }
    Ok "Revit $version add-in extracted."
}

Remove-Item -Recurse -Force $extractDir -ErrorAction SilentlyContinue
$FirstInstall = $InstalledAddIns | Select-Object -First 1
if (-not $FirstInstall) { Fail 'No Revit add-ins were installed.' }
$BinDir = $FirstInstall.BinDir

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

$Guide = Join-Path $AddInDir 'Resources\EarlyAccessInstall.html'
if (Test-Path $Guide) {
    Step "Opening install guide ..."
    Start-Process $Guide
}

Write-Host ""
Write-Host "==> Done. Launch Revit — the Vex Atlas ribbon will appear under Add-Ins." -ForegroundColor Green
foreach ($install in $InstalledAddIns) {
    Write-Host "    Revit $($install.Version) manifest: $($install.RootAddIn)"
    Write-Host "    Revit $($install.Version) folder:   $($install.AddInDir)"
}
Write-Host "    Account setup:    https://studio.planmorph.software/register"
Write-Host "    Sign in:          https://studio.planmorph.software/login"
Write-Host ""
