# Adding a new CAD

Any CAD with a plugin runtime that can make HTTP calls is a one-afternoon
job. The contract is fixed; only the host language varies.

## The shape of every plugin

```
1. Read the access token from disk:
     - macOS / Linux: ~/.config/vex-bridge/access-token
     - Windows:       %APPDATA%\vex-bridge\access-token
2. POST http://127.0.0.1:7878/v1/repo/push
       Headers: Content-Type: application/json
                X-Vex-Bridge-Token: <token>
       Body:    {"project_id": "...", "branch": "main"}
3. Stream the NDJSON response, surface progress + errors in the host UI.
```

That's it. Everything CAD-specific (how to get the open document on disk,
how to add a button to a ribbon, how to pop a dialog) is host-side.

## Recommended approach

| Task                              | Where it should live                        |
| --------------------------------- | ------------------------------------------- |
| Detect "save" / export-to-IFC     | Plugin (host SDK)                           |
| Author identity, pairing, push    | Daemon (already done)                       |
| Surface progress to user          | Plugin (host UI toolkit)                    |

If your CAD has *no* plugin SDK, you don't need a plugin at all — point
`vex-bridge` at a folder and use the host's `File → Export → IFC...` menu.
The daemon's filesystem watcher will do the rest.

## Examples in this repo

- `plugins/revit-csharp/` — `IExternalApplication` + `IExternalCommand`,
  ribbon button.
- `plugins/rhino-python/` — single `.py` file dropped into Rhino's
  Scripts folder.
