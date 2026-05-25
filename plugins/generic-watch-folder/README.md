# Generic watch-folder mode (Tier 3)

This works with **every** CAD on the market — no plugin required.

## Setup

1. Install and pair `vex-bridge`:

   ```
   vex-bridge pair --device-label "My MacBook"
   ```

2. Pick or create a folder you'll always export IFC into, e.g.
   `~/VexInbox/<project_name>/`.

3. Register it through the daemon API. The desktop setup UI will do this for
   users; curl is shown here for adapter authors and manual testing:

   ```sh
   TOKEN="$(cat ~/.config/vex-bridge/access-token)"
   curl -sS -X POST http://127.0.0.1:7878/v1/setup/inbox \
     -H "X-Vex-Bridge-Token: $TOKEN" \
     -H 'Content-Type: application/json' \
     -d '{
       "project_id": "prj_01HXYZ...",
       "project_name": "Commercial Tower",
       "local_path": "/Users/me/VexInbox/Commercial-Tower",
       "include": ["*.ifc"],
       "ifc_project_guid": "2HnQxDrSH5sBbC4NkVOGR8"
     }'
   ```

   This persists the equivalent config and starts watching immediately:

   ```toml
   [[watch]]
   project_id = "prj_01HXYZ..."          # from architur web UI
   path       = "/Users/me/VexInbox/Commercial-Tower"
   include    = ["*.ifc"]                # default
   ifc_project_guid = "2HnQxDrSH5sBbC4NkVOGR8" # optional IfcProject.GlobalId route
   project_name = "Commercial Tower"     # optional display name
   ```

4. It will now commit and push every IFC file you save into that folder, no
   matter which CAD wrote it.

## In your CAD

Whatever your CAD is — Vectorworks, Bentley OpenBuildings, Allplan, Tekla,
Civil 3D, FreeCAD, BricsCAD — use:

```
File → Export → IFC...
```

Save into the watched folder. Done. The daemon hashes the IFC with BLAKE3,
skips exact duplicate exports, asks `vex ifc-intake --json` for the IFC header
and `IfcProject` routing metadata, imports the model with `vex import`, commits
and pushes it, then moves the processed IFC into `.vex/archive/`.

## Caveats

- The watcher debounces by 2 seconds; on very large IFC exports your CAD
  must finish writing before the daemon pushes.
- Only `.ifc` files matching `include` trigger a commit.
- If `ifc_project_guid` is set and a dropped IFC has a different project GUID,
   the file is left in place and the daemon logs the mismatch instead of routing
   it to the wrong Vex project.
