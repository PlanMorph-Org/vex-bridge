# Build a fully-formed Autodesk Bundle (.bundle) for vex-bridge.
#
# This script produces a SELF-CONTAINED bundle that ships the vex CLI, the
# vex-bridge daemon, AND the Revit + AutoCAD add-in DLLs together. After
# the MSI installs, the user does NOT need to download or install anything
# else, and does NOT need to touch a terminal — the daemon is auto-started
# by a per-user Scheduled Task (registered by the MSI in Product.wxs) and
# the in-CAD Pair button drives the whole pairing flow in-process.
#
#   PowerShell:   .\build-bundle.ps1
#                 .\build-bundle.ps1 -VexBin C:\path\to\vex.exe -VexBridgeBin C:\path\to\vex-bridge.exe
#
# Output:
#   dist/VexBridge.bundle/
#     PackageContents.xml
#     Contents/
#       Resources/icon.png  ReadMe.html  LICENSE.txt
#       bin/vex.exe          vex-bridge.exe   vex-bridge-launch.vbs
#       Revit/2022/ VexBridgeRevit.dll  VexBridgeRevit.addin
#       Revit/2023/ ...
#       Revit/2027/ ...    (Revit 2027 = .NET 10 — see csproj TFM mapping)
#       AutoCAD/2024/ VexBridgeAutoCAD.dll
#       AutoCAD/2025/ ...
#       AutoCAD/2027/ ...
#
# The wrapping MSI lives in installer/wix/Product.wxs.

[CmdletBinding()]
param(
    # Every Revit major version we ship a per-year add-in DLL for. Both
    # Revit and AutoCAD only honor SeriesMin (exact match) in
    # PackageContents.xml — SeriesMax is ignored — so each year MUST have
    # its own per-year folder AND its own <Components> block in
    # PackageContents.xml or it simply won't show up in that release.
    # See: https://blog.autodesk.io/revit-api-understanding-the-role-of-seriesmin-and-seriesmax-in-plugin-deployment/
    # When Autodesk ships a new year, add it here AND in
    # installer/PackageContents.xml. The csproj TFM mappings
    # (net48 < 2025, net8.0-windows 2025–2026, net10.0-windows 2027+)
    # auto-select the right framework via -p:RevitVersion / -p:AcadVersion.
    [string[]] $RevitVersions = @('2022', '2023', '2024', '2025', '2026', '2027'),
    [string[]] $AcadVersions  = @('2024', '2025', '2026', '2027'),
    [string]   $Configuration  = 'Release',
    [string]   $OutDir         = 'dist',
    # Paths to the prebuilt CLI / daemon binaries. Defaults look in the
    # workspace target/release dirs so a clean checkout + cargo build
    # at the workspace root produces a working bundle.
    [string]   $VexBin         = '',
    [string]   $VexBridgeBin   = ''
)

$ErrorActionPreference = 'Stop'
$root      = Split-Path -Parent $PSScriptRoot
$revitProj = Join-Path $root 'VexBridgeRevit.csproj'
$acadProj  = Join-Path (Split-Path -Parent $root) 'autocad-csharp\VexBridgeAutoCAD.csproj'
$bundle    = Join-Path $root "$OutDir/VexBridge.bundle"
$contents  = Join-Path $bundle 'Contents'
$resources = Join-Path $contents 'Resources'
$binDir    = Join-Path $contents 'bin'
$revitDir  = Join-Path $contents 'Revit'
$acadDir   = Join-Path $contents 'AutoCAD'

# Resolve binary paths. If not provided, try standard cargo output locations.
if (-not $VexBin) {
    $candidate = Join-Path $root '..\..\..\vex\target\release\vex.exe'
    if (Test-Path $candidate) { $VexBin = (Resolve-Path $candidate).Path }
}
if (-not $VexBridgeBin) {
    $candidate = Join-Path $root '..\..\..\target\release\vex-bridge.exe'
    if (Test-Path $candidate) { $VexBridgeBin = (Resolve-Path $candidate).Path }
}
if (-not (Test-Path $VexBin))       { throw "vex.exe not found. Pass -VexBin <path> or build it first (cargo build --release in the vex workspace)." }
if (-not (Test-Path $VexBridgeBin)) { throw "vex-bridge.exe not found. Pass -VexBridgeBin <path> or run 'cargo build --release -p vex-bridge'." }

if (Test-Path $bundle) { Remove-Item -Recurse -Force $bundle }
New-Item -ItemType Directory -Path $bundle, $contents, $resources, $binDir | Out-Null

Copy-Item (Join-Path $PSScriptRoot 'PackageContents.xml') $bundle
Copy-Item (Join-Path $PSScriptRoot '../marketplace/ReadMe.html') $resources -ErrorAction SilentlyContinue
Copy-Item (Join-Path $PSScriptRoot '../marketplace/LICENSE.txt') $resources -ErrorAction SilentlyContinue
Copy-Item (Join-Path $PSScriptRoot '../marketplace/icon.png')    $resources -ErrorAction SilentlyContinue

# Ship the actual binaries the daemon + plugin shell out to. These end up
# at: %ProgramData%\Autodesk\ApplicationPlugins\VexBridge.bundle\Contents\bin\
Write-Host ">> Bundling vex CLI:    $VexBin"
Copy-Item $VexBin       (Join-Path $binDir 'vex.exe')        -Force
Write-Host ">> Bundling vex-bridge: $VexBridgeBin"
Copy-Item $VexBridgeBin (Join-Path $binDir 'vex-bridge.exe') -Force

# Ship a tiny VBScript launcher alongside the binaries. The MSI registers a
# Scheduled Task that invokes `wscript.exe vex-bridge-launch.vbs`, which uses
# WshShell.Run with windowStyle = 0 (SW_HIDE) to start the daemon at every
# user logon WITHOUT flashing a console window. Doing this via
# `powershell.exe -WindowStyle Hidden -Command "& 'vex-bridge.exe' start"`
# (the previous approach) still pops a conhost window every login on Windows
# 10/11 because the child console-subsystem app gets its own conhost. The
# daemon writes its logs to %APPDATA%\vex-bridge\vex-bridge.log so nothing
# useful is hidden by suppressing the window.
$launchVbs = Join-Path $binDir 'vex-bridge-launch.vbs'
@'
' Launches vex-bridge.exe (located next to this .vbs) with the daemon
' subcommand and ZERO console window. Invoked at user logon by the
' Scheduled Task registered by the MSI. Path-independent so the bundle
' can live under either %ProgramData% or %APPDATA%.
Set fso = CreateObject("Scripting.FileSystemObject")
Set sh  = CreateObject("WScript.Shell")
exe = fso.BuildPath(fso.GetParentFolderName(WScript.ScriptFullName), "vex-bridge.exe")
sh.Run """" & exe & """ start", 0, False
'@ | Out-File -FilePath $launchVbs -Encoding ASCII -Force
Write-Host ">> Bundling launcher:   $launchVbs"

foreach ($v in $RevitVersions) {
    Write-Host ">> Building add-in for Revit $v ..."
    $verDir = Join-Path $revitDir $v
    New-Item -ItemType Directory -Force -Path $verDir | Out-Null

    & dotnet build $revitProj `
        -c $Configuration `
        -p:RevitVersion=$v `
        -p:OutputPath="$verDir/" `
        --nologo
    if ($LASTEXITCODE -ne 0) { throw "build failed for Revit $v" }

    Copy-Item (Join-Path $root 'VexBridgeRevit.addin') $verDir -Force
}

foreach ($v in $AcadVersions) {
    Write-Host ">> Building add-in for AutoCAD $v ..."
    $verDir = Join-Path $acadDir $v
    New-Item -ItemType Directory -Force -Path $verDir | Out-Null

    & dotnet build $acadProj `
        -c $Configuration `
        -p:AcadVersion=$v `
        -p:OutputPath="$verDir/" `
        --nologo
    if ($LASTEXITCODE -ne 0) { throw "build failed for AutoCAD $v" }
}

Write-Host ""
Write-Host "Bundle ready at $bundle"
Write-Host "Bundled binaries:"
Get-ChildItem $binDir | Format-Table Name, Length -AutoSize

# ── Emit a ZIP of the bundle for cert-free direct distribution. ───────────
# Users who don't want the MSI (no SmartScreen, no Authenticode, no admin)
# can extract this straight into:
#   %ProgramData%\Autodesk\ApplicationPlugins\
# Revit picks it up on next launch. No installer, no signing required.
$zipName = "VexBridge-bundle.zip"
$zipPath = Join-Path (Split-Path -Parent $bundle) $zipName
if (Test-Path $zipPath) { Remove-Item -Force $zipPath }
Write-Host ">> Packing bundle ZIP: $zipPath"
Compress-Archive -Path $bundle -DestinationPath $zipPath -CompressionLevel Optimal
Write-Host "   ZIP ready ($([math]::Round((Get-Item $zipPath).Length / 1MB, 1)) MiB)"

Write-Host ""
Write-Host "To install for the current user, copy it to:"
Write-Host "    %APPDATA%\Autodesk\ApplicationPlugins\"
Write-Host ""
Write-Host "To produce the App Store MSI:"
Write-Host "    cd installer\wix"
Write-Host "    wix build -arch x64 Product.wxs -out ..\..\dist\VexBridgeRevit-0.2.0.msi"
