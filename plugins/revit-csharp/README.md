# vex-bridge for Revit

Revit ribbon plug-in. The MSI is **completely self-contained** — it
ships the `vex.exe` CLI and the `vex-bridge.exe` daemon inside the
bundle, registers a per-user Scheduled Task that auto-starts the daemon
at every login, and the *Pair* button drives the whole pairing flow
in-process (no console window, no terminal, no separate downloads).
An architect can install from the Autodesk App Store and click *Pair*
without ever opening a shell.

```
plugins/revit-csharp/
├── Application.cs          # ribbon panel + ensures daemon is running on Revit start
├── PushCommand.cs          # opens picker, hits POST /v1/repo/push
├── PairCommand.cs          # in-process pair flow — opens browser, polls /pair/status
├── BridgeClient.cs         # tiny HTTP client over the loopback daemon
├── BundledBin.cs           # locates bundled vex.exe + vex-bridge.exe; starts daemon hidden
├── ProjectPickerDialog.cs  # WinForms modal: project id + branch
├── VexBridgeRevit.csproj
├── VexBridgeRevit.addin    # Revit add-in manifest
├── installer/
│   ├── PackageContents.xml # Autodesk Bundle manifest
│   ├── build-bundle.ps1    # composes dist/VexBridge.bundle/ incl. bin/vex.exe + vex-bridge.exe
│   └── wix/Product.wxs     # MSI for Autodesk App Store submission + ScheduledTask registration
└── marketplace/
    ├── LISTING.md          # App Store listing copy
    ├── ReadMe.html         # required Help file inside the bundle
    ├── LICENSE.txt
    ├── icon.png            # 512×512, no shadow
    └── screenshots/        # 1280×800 + 1920×1080 PNGs
```

## Build (developer install)

```powershell
dotnet build VexBridgeRevit.csproj -c Release -p:RevitVersion=2024
```

Then drop the `.dll` and `.addin` into:
`%APPDATA%\Autodesk\Revit\Addins\2024\`

Open Revit → *Add-Ins* tab → vex-bridge panel → *Push to architur*.

## Build (Autodesk Bundle, what real users install)

`build-bundle.ps1` requires prebuilt `vex.exe` and `vex-bridge.exe` to
stuff into `Contents\bin\`. Build them first with `cargo build --release`
in each workspace:

```powershell
# In the vex repo
cargo build --release           # → ..\vex\target\release\vex.exe

# In the vex-bridge repo
cargo build --release -p vex-bridge   # → ..\target\release\vex-bridge.exe

# Then in this directory
cd installer
.\build-bundle.ps1 -Versions 2022,2023,2024
# → dist/VexBridge.bundle/
#       Contents\bin\vex.exe
#       Contents\bin\vex-bridge.exe
#       Contents\{2022,2023,2024}\VexBridgeRevit.dll
```

Copy the bundle to either of these locations and Revit auto-discovers it:

| Scope        | Path                                                         |
|--------------|--------------------------------------------------------------|
| Per-user     | `%APPDATA%\Autodesk\ApplicationPlugins\VexBridge.bundle\`    |
| Machine-wide | `%ProgramData%\Autodesk\ApplicationPlugins\VexBridge.bundle\`|

## Build (signed MSI for Autodesk App Store)

```powershell
cd installer
.\build-bundle.ps1
wix build -arch x64 wix/Product.wxs -out dist/VexBridgeRevit-0.2.0.msi
signtool sign /tr http://timestamp.digicert.com /td sha256 /fd sha256 /a `
              dist/VexBridgeRevit-0.2.0.msi
```

Then upload the signed MSI at <https://apps.autodesk.com/MyUploads> and
paste the listing copy from [marketplace/LISTING.md](marketplace/LISTING.md).
The reviewer team installs the MSI on a clean Revit VM and exercises the
ribbon button against the *Sample Architectural Project*.

## What the user sees, end-to-end

1. Install the MSI from the Autodesk App Store. Per-user Scheduled Task
   `vex-bridge` is registered and the daemon is launched immediately.
2. Open Revit. Even if the Scheduled Task hasn't fired yet (e.g. they
   didn't log out / log back in), `Application.OnStartup` fires off
   `BundledBin.EnsureDaemonRunning` which starts the bundled
   `vex-bridge.exe` *hidden* (`CreateNoWindow=true`).
3. Click *Pair this device*. The plug-in calls
   `POST /v1/pair/start`, opens the verification URL in the user's
   default browser, and shows a TaskDialog with the confirmation code.
   No console window, ever.
4. Click *Push to architur*, pick the project, click OK. The plug-in
   calls `POST /v1/repo/push`, which runs the same `vex add → commit →
   push` pipeline as the file watcher.

## Architecture

```
Revit
 │
 │ (UI thread, sync HttpClient)
 ▼
PushCommand ──► BridgeClient ──► http://127.0.0.1:7878/v1/repo/push
                                                     │
                                                     ▼
                              %ProgramData%\Autodesk\ApplicationPlugins\
                                  VexBridge.bundle\Contents\bin\vex-bridge.exe
                                                     │ subprocess
                                                     ▼
                              %ProgramData%\Autodesk\ApplicationPlugins\
                                  VexBridge.bundle\Contents\bin\vex.exe ──SSH──► architur
```

The Revit process never touches the public internet. The daemon does.
That single design choice is what makes the plug-in eligible for the
Autodesk App Store: Autodesk reviewers explicitly forbid plug-ins that
phone home directly from the Revit process.

## Versioning

Bump the version in *three* places when releasing:

1. `VexBridgeRevit.csproj` → `<Version>`
2. `installer/PackageContents.xml` → `AppVersion`, `FriendlyVersion`, all `ComponentEntry/@Version`
3. `installer/wix/Product.wxs` → `Version`

A future commit will replace this with a single `Directory.Build.props`.
