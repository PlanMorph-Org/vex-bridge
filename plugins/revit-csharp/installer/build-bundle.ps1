# Build a fully-formed Autodesk Bundle (.bundle) for vex-bridge for Revit.
#
# This script produces a SELF-CONTAINED bundle that ships the vex CLI, the
# vex-bridge daemon, and the Revit add-in DLL together. After the MSI
# installs, the user does NOT need to download or install anything else,
# and does NOT need to touch a terminal — the daemon is auto-started by a
# per-user Scheduled Task (registered by the MSI in Product.wxs) and the
# Pair button drives the whole pairing flow in-process.
#
#   PowerShell:   .\build-bundle.ps1
#                 .\build-bundle.ps1 -VexBin C:\path\to\vex.exe -VexBridgeBin C:\path\to\vex-bridge.exe
#
# Output:
#   dist/VexBridge.bundle/
#     PackageContents.xml
#     Contents/
#       Resources/icon.png  ReadMe.html  LICENSE.txt
#       bin/vex.exe          vex-bridge.exe         <-- shipped, not downloaded
#       2022/ VexBridgeRevit.dll  VexBridgeRevit.addin
#       2023/ ...
#       2024/ ...
#       2025/ ...
#
# The wrapping MSI lives in installer/wix/Product.wxs.

[CmdletBinding()]
param(
    [string[]] $Versions      = @('2022', '2023', '2024', '2025'),
    [string]   $Configuration = 'Release',
    [string]   $OutDir        = 'dist',
    # Paths to the prebuilt CLI / daemon binaries. Defaults look in the
    # workspace target/release dirs so a clean checkout + cargo build
    # at the workspace root produces a working bundle.
    [string]   $VexBin        = '',
    [string]   $VexBridgeBin  = ''
)

$ErrorActionPreference = 'Stop'
$root      = Split-Path -Parent $PSScriptRoot
$projFile  = Join-Path $root 'VexBridgeRevit.csproj'
$bundle    = Join-Path $root "$OutDir/VexBridge.bundle"
$contents  = Join-Path $bundle 'Contents'
$resources = Join-Path $contents 'Resources'
$binDir    = Join-Path $contents 'bin'

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

foreach ($v in $Versions) {
    Write-Host ">> Building add-in for Revit $v ..."
    $verDir = Join-Path $contents $v
    New-Item -ItemType Directory -Path $verDir | Out-Null

    & dotnet build $projFile `
        -c $Configuration `
        -p:RevitVersion=$v `
        -p:OutputPath="$verDir/" `
        --nologo
    if ($LASTEXITCODE -ne 0) { throw "build failed for Revit $v" }

    Copy-Item (Join-Path $root 'VexBridgeRevit.addin') $verDir -Force
}

Write-Host ""
Write-Host "Bundle ready at $bundle"
Write-Host "Bundled binaries:"
Get-ChildItem $binDir | Format-Table Name, Length -AutoSize
Write-Host "To install for the current user, copy it to:"
Write-Host "    %APPDATA%\Autodesk\ApplicationPlugins\"
Write-Host ""
Write-Host "To produce the App Store MSI:"
Write-Host "    cd installer\wix"
Write-Host "    wix build -arch x64 Product.wxs -out ..\..\dist\VexBridgeRevit-0.2.0.msi"
