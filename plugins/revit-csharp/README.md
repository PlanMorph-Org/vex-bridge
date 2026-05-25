# vex-bridge for Revit

Optional Revit ribbon plug-in source. The standalone Vex product does not ship
or require this plug-in; the MVP workflow is exporting IFC into a Vex inbox
folder watched by `vex-bridge`.

This project remains as a Tier 1 accelerator for a future workflow where Revit
can trigger the same underlying daemon pipeline from a ribbon button.

## Layout

```text
plugins/revit-csharp/
├── Application.cs          # ribbon panel + ensures daemon is running on Revit start
├── PushCommand.cs          # opens picker, hits POST /v1/repo/push
├── PairCommand.cs          # in-process pair flow over the local daemon
├── BridgeClient.cs         # tiny HTTP client over loopback
├── BundledBin.cs           # locates bundled binaries for developer/plugin builds
├── ProjectPickerDialog.cs  # WinForms modal: project id + branch
├── VexBridgeRevit.csproj
└── VexBridgeRevit.addin    # Revit add-in manifest
```

## Developer Build

```powershell
dotnet build VexBridgeRevit.csproj -c Release -p:RevitVersion=2024
```

For local testing, copy the built `.dll` and `.addin` into the per-user Revit
add-in folder for the target year:

```text
%APPDATA%\Autodesk\Revit\Addins\2024\
```

Open Revit, then use the Vex panel from the Add-Ins tab. The plug-in expects a
running `vex-bridge` daemon and calls the same localhost API used by the
standalone workflow.

## Runtime Shape

```text
Revit
 │
 │ (UI thread, sync HttpClient)
 ▼
PushCommand ──► BridgeClient ──► http://127.0.0.1:7878/v1/repo/push
                                                     │
                                                     ▼
                                                vex-bridge
                                                     │ subprocess
                                                     ▼
                                                    vex CLI
```

The Revit process should stay a thin shell. Pairing, key management, IFC import,
commits, pushes, dedupe, and archiving belong in the daemon.
