# vex-bridge for AutoCAD

Tier-1 AutoCAD plug-in. Loads on AutoCAD startup via the Autodesk Bundle
mechanism and exposes three commands at the AutoCAD command line:

| Command   | What it does                                                       |
| --------- | ------------------------------------------------------------------ |
| `VEXPUSH` | Commit + push the active drawing to architur.                      |
| `VEXPAIR` | Pair this device with an architur account (browser-based, no SSH). |
| `VEXEULA` | Re-display the End User License Agreement.                         |

The plug-in itself is a thin shell — all real work (key management,
SSH, network) happens in the bundled `vex-bridge.exe` daemon. Same daemon
the Revit plug-in uses; only one runs per machine.

## Building

```powershell
# From plugins/autocad-csharp/
dotnet build -c Release                  # Defaults to AutoCAD 2025 (net8.0)
dotnet build -c Release -p:AcadVersion=2024  # net48
dotnet build -c Release -p:AcadVersion=2026  # net8.0
dotnet build -c Release -p:AcadVersion=2027  # net10.0
```

The MSI / bundle build (`plugins/revit-csharp/installer/build-bundle.ps1`)
invokes this csproj for every supported AutoCAD year and drops the DLLs
into `Contents/AutoCAD/<year>/` inside the shared `VexBridge.bundle`.

## AutoCAD year → release-number mapping

Used in `PackageContents.xml` because AutoCAD's bundle loader compares
on the internal release number, not the marketing year:

| Year | SeriesMin |
| ---- | --------- |
| 2024 | R24.3     |
| 2025 | R25.0     |
| 2026 | R25.1     |
| 2027 | R26.0     |
