# vex-bridge for AutoCAD

Optional Tier 1 AutoCAD plug-in source. The standalone Vex product does not
ship or require this plug-in; the MVP workflow is exporting IFC into a Vex
inbox folder watched by `vex-bridge`.

When built for local development, the plug-in exposes three commands at the
AutoCAD command line:

| Command   | What it does                                                       |
| --------- | ------------------------------------------------------------------ |
| `VEXPUSH` | Commit + push the active drawing to architur.                      |
| `VEXPAIR` | Pair this device with an architur account (browser-based, no SSH). |
| `VEXEULA` | Re-display the End User License Agreement.                         |

The plug-in itself is a thin shell. All real work (key management, IFC import,
commits, pushes, SSH, network) happens in the `vex-bridge.exe` daemon.

## Building

```powershell
# From plugins/autocad-csharp/
dotnet build -c Release                  # Defaults to AutoCAD 2025 (net8.0)
dotnet build -c Release -p:AcadVersion=2024  # net48
dotnet build -c Release -p:AcadVersion=2026  # net8.0
dotnet build -c Release -p:AcadVersion=2027  # net10.0
```

The standalone release workflow does not package AutoCAD DLLs. Build this
project directly when testing the optional plug-in path.

## AutoCAD year → release-number mapping

Used when building/testing the optional AutoCAD plug-in because Autodesk APIs
and package references use the internal release number, not the marketing year:

| Year | SeriesMin |
| ---- | --------- |
| 2024 | R24.3     |
| 2025 | R25.0     |
| 2026 | R25.1     |
| 2027 | R26.0     |
