#Requires -Version 5.1
<#
.SYNOPSIS
    Installs vex + vex-bridge on Windows, then opens the Architur setup
    screen so you can finish pairing in one go — no terminal commands needed.

.DESCRIPTION
    1. Downloads the latest vex and vex-bridge binaries from GitHub Releases.
    2. Installs both to %LOCALAPPDATA%\vex-bridge\bin and adds that to PATH.
    3. Registers vex-bridge as a Task Scheduler task (auto-start at every login).
    4. Starts the daemon immediately.
    5. Opens your browser to the Architur setup/pairing page.
       Click Approve and this machine is ready.

.EXAMPLE
    irm https://studio.planmorph.software/api/install/script.ps1 | iex
.EXAMPLE
    irm https://github.com/Planmorph-Org/vex-bridge/releases/latest/download/install.ps1 | iex
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RepoVex    = if ($env:VEX_GITHUB_REPO)        { $env:VEX_GITHUB_REPO }        else { 'Planmorph-Org/vex' }
$RepoBridge = if ($env:VEX_BRIDGE_GITHUB_REPO) { $env:VEX_BRIDGE_GITHUB_REPO } else { 'Planmorph-Org/vex-bridge' }
$Suffix     = 'windows-x86_64'
$InstallDir = Join-Path $env:LOCALAPPDATA 'vex-bridge\bin'
$TaskName   = 'vex-bridge'

function Step($msg)  { Write-Host "==> $msg" -ForegroundColor Cyan }
function Ok($msg)    { Write-Host "    $([char]0x2713) $msg" -ForegroundColor Green }
function Fail($msg)  { Write-Error "    $([char]0x2717) $msg"; exit 1 }

# ── Platform check ────────────────────────────────────────────────────────────
if ($env:OS -ne 'Windows_NT') { Fail 'This script is for Windows. Use install.sh on macOS.' }

# ── Install directory ─────────────────────────────────────────────────────────
Step "Creating install directory: $InstallDir"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

# Persist to user PATH if not already present.
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($userPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable('PATH', "$InstallDir;$userPath", 'User')
    $env:PATH = "$InstallDir;$env:PATH"
    Ok "Added $InstallDir to user PATH (takes effect in new shells)"
}

# ── Helper: get latest release tag via GitHub API ─────────────────────────────
function Get-LatestTag($repo) {
    $url  = "https://api.github.com/repos/$repo/releases/latest"
    $resp = Invoke-RestMethod -Uri $url -Headers @{ 'User-Agent' = 'vex-installer' }
    return $resp.tag_name
}

# ── Helper: download + extract a release archive ──────────────────────────────
# Releases ship as .tar.gz (one format across all platforms). PowerShell's
# Expand-Archive only handles .zip, so we shell out to bsdtar (`tar.exe`),
# which is built into Windows 10 1803+ and Windows 11. Using Expand-Archive
# here used to silently produce garbage files and was the source of the
# "binary isn't in the correct format" error users were seeing.
function Install-Release($repo, $tag, $archiveName, $binNames) {
    $url  = "https://github.com/$repo/releases/download/$tag/$archiveName"
    $tmp  = Join-Path $env:TEMP $archiveName
    Step "Downloading $archiveName ..."
    Invoke-WebRequest -Uri $url -OutFile $tmp -UseBasicParsing

    if (-not (Get-Command tar.exe -ErrorAction SilentlyContinue)) {
        Fail 'tar.exe not found. Windows 10 build 1803 or Windows 11 is required.'
    }

    $extractDir = Join-Path $env:TEMP ("vex-extract-" + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
    & tar.exe -xzf $tmp -C $extractDir
    if ($LASTEXITCODE -ne 0) { Fail "Failed to extract $archiveName (tar exit $LASTEXITCODE)." }

    # The archive unpacks to a single sub-folder.
    $inner = Get-ChildItem $extractDir -Directory | Select-Object -First 1
    if (-not $inner) { Fail "Archive $archiveName had no inner folder." }
    foreach ($bin in $binNames) {
        $src  = Join-Path $inner.FullName $bin
        if (-not (Test-Path $src)) { Fail "Expected binary '$bin' not found in archive." }
        $dest = Join-Path $InstallDir $bin
        Copy-Item -Path $src -Destination $dest -Force
        Ok "Installed $bin -> $dest"
    }
    Remove-Item $tmp -Force -ErrorAction SilentlyContinue
    Remove-Item $extractDir -Recurse -Force -ErrorAction SilentlyContinue
}

# ── Fetch versions ────────────────────────────────────────────────────────────
Step 'Fetching latest release versions...'
$VexTag    = Get-LatestTag $RepoVex
$BridgeTag = Get-LatestTag $RepoBridge
if (-not $VexTag)    { Fail 'Could not determine latest vex version.' }
if (-not $BridgeTag) { Fail 'Could not determine latest vex-bridge version.' }
Ok "vex        $VexTag"
Ok "vex-bridge $BridgeTag"

# ── Install vex ───────────────────────────────────────────────────────────────
Install-Release $RepoVex    $VexTag    "vex-$VexTag-$Suffix.tar.gz"        @('vex.exe')
# ── Install vex-bridge ────────────────────────────────────────────────────────
Install-Release $RepoBridge $BridgeTag "vex-bridge-$BridgeTag-$Suffix.tar.gz" @('vex-bridge.exe')

# ── Task Scheduler — run vex-bridge at login ──────────────────────────────────
Step 'Registering Task Scheduler task (auto-start at login)...'

$BridgeExe = Join-Path $InstallDir 'vex-bridge.exe'
$LogDir    = Join-Path $env:LOCALAPPDATA 'vex-bridge\logs'
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

# Remove any stale task silently.
Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue

$action  = New-ScheduledTaskAction `
    -Execute $BridgeExe `
    -Argument 'start' `
    -WorkingDirectory $InstallDir

# LogonType = Interactive means "when this user logs in", no elevation required.
$trigger = New-ScheduledTaskTrigger -AtLogOn -User "$env:USERDOMAIN\$env:USERNAME"

$settings = New-ScheduledTaskSettingsSet `
    -ExecutionTimeLimit ([TimeSpan]::Zero) `
    -RestartCount 5 `
    -RestartInterval (New-TimeSpan -Minutes 1) `
    -StartWhenAvailable

$principal = New-ScheduledTaskPrincipal `
    -UserId "$env:USERDOMAIN\$env:USERNAME" `
    -LogonType Interactive `
    -RunLevel Limited   # No UAC elevation needed.

Register-ScheduledTask `
    -TaskName  $TaskName `
    -Action    $action `
    -Trigger   $trigger `
    -Settings  $settings `
    -Principal $principal `
    -Force | Out-Null

Ok "Task '$TaskName' registered."

# ── Start the daemon now ───────────────────────────────────────────────────────
Step 'Starting vex-bridge...'
Start-ScheduledTask -TaskName $TaskName
Start-Sleep -Seconds 2   # Give it a moment to bind the port.

$status = (Get-ScheduledTask -TaskName $TaskName).State
if ($status -eq 'Running') {
    Ok 'vex-bridge is running.'
} else {
    Write-Host "    Note: task state is '$status' — it may still be starting." -ForegroundColor Yellow
}

# ── Open setup screen in the browser ──────────────────────────────────────────
# vex-bridge pair --open-browser:
#   - registers an Ed25519 key with Architur
#   - opens the Architur approval page in the default browser automatically
#   - polls quietly until the user clicks Approve
#   - on success: this machine is paired and ready to push
Step 'Opening Architur setup screen...'
$BridgeExe = Join-Path $InstallDir 'vex-bridge.exe'
& $BridgeExe pair --device-label $env:COMPUTERNAME --open-browser

# ── Done ───────────────────────────────────────────────────────────────────────
Write-Host ''
Write-Host '==> All done! This machine is paired with your Architur account.' -ForegroundColor Green
Write-Host ''
Write-Host 'Your CAD plugins can now push models by hitting:'
Write-Host '  http://127.0.0.1:7878/v1/repo/push' -ForegroundColor White
Write-Host ''
Write-Host 'Run  vex --help  to use the CLI directly.'
Write-Host ''
