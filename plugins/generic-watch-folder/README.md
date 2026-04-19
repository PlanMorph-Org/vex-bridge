# Generic watch-folder mode (Tier 3)

This works with **every** CAD on the market — no plugin required.

## Setup

1. Install and pair `vex-bridge`:

   ```
   vex-bridge pair --device-label "My MacBook"
   ```

2. Pick or create a folder you'll always export IFC into, e.g.
   `~/Architur/Watched/<project_name>/`.

3. Edit your config (`~/.config/vex-bridge/config.toml` on Linux/macOS,
   `%APPDATA%\vex-bridge\config.toml` on Windows):

   ```toml
   [[watch]]
   project_id = "prj_01HXYZ..."          # from architur web UI
   path       = "/Users/me/Architur/Watched/Tower"
   include    = ["*.ifc"]                # default
   ```

4. Restart the daemon. It will now commit and push every IFC file you save
   into that folder, no matter which CAD wrote it.

## In your CAD

Whatever your CAD is — Vectorworks, Bentley OpenBuildings, Allplan, Tekla,
Civil 3D, FreeCAD, BricsCAD — use:

```
File → Export → IFC...
```

Save into the watched folder. Done.

## Caveats

- The watcher debounces by 2 seconds; on very large IFC exports your CAD
  must finish writing before the daemon pushes.
- Only files matching `include` globs trigger a commit.
- The daemon never deletes anything in the watched folder. To remove a file
  from the repo, do it from the architur web UI.
