# Revit plugin (C#)

A minimal Revit ribbon button + external command. All it does is read the
local access token from disk and POST to the daemon. ~80 LOC of C# total.

## Build

```
dotnet build VexBridgeRevit.csproj -c Release
```

The output `.dll` and the `.addin` manifest go in:

| OS      | Path                                                                             |
| ------- | -------------------------------------------------------------------------------- |
| Windows | `%APPDATA%\Autodesk\Revit\Addins\<year>\VexBridgeRevit.addin`                    |

You can target Revit 2022–2025 by changing the `RevitVersion` property in
the csproj — the API surface we use (`IExternalApplication`,
`IExternalCommand`, ribbon panel) has been stable for many versions.
