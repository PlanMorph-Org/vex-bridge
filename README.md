# vex-bridge

> The standalone desktop agent that versions IFC exports from **any** CAD tool.

Architects don't know what SSH is, and they shouldn't have to. `vex-bridge`
is a small daemon that runs on the user's machine, holds their key in the OS
keychain, watches an IFC inbox folder, and shells out to the `vex` CLI without
ever asking the user to touch a terminal.

```
┌──────────────────────┐    Export IFC             ┌──────────────────┐
│  Revit / Rhino /     │ ─────────────────────────▶│   Vex Inbox      │
│  ArchiCAD / AutoCAD  │                            │   watched folder │
│  / any IFC tool      │                            └────────┬─────────┘
└──────────────────────┘                                     │ fs event
                                                             ▼
                                                    ┌──────────────────┐
                                                    │   vex-bridge     │
                                                    │   daemon         │
                                                    └────────┬─────────┘
                                                             │ subprocess
                                                             ▼
                                                       ┌────────────┐
                                                       │  vex CLI   │
                                                       └────┬───────┘
                                                            │ SSH ed25519
                                                            ▼
                                                  ┌──────────────────────┐
                                                  │  vex-serve on        │
                                                  │  vex.planmorph.software │
                                                  └──────────────────────┘
```

## Why a daemon, not "AI rewrites the binary for each CAD"?

You raised the idea of an autonomous SDK that rewrites itself per CAD. We
deliberately did **not** go that route, and here is the honest engineering
case:

1. **Code-signing breaks self-modifying binaries.** macOS Gatekeeper, Windows
   SmartScreen, and antivirus all hash a signed binary at install time. If the
   binary edits itself, the signature breaks and the OS quarantines it on
   next launch. End-user IT will block the install entirely.
2. **Firms audit static artefacts.** Architecture and engineering firms run
   procurement reviews. They will not approve a tool whose code mutates after
   install — there is nothing for security review to inspect.
3. **Plugin marketplaces reject mutable code.** Autodesk App Store, McNeel
   Food4Rhino, Graphisoft Plugins — all forbid downloading + executing new
   code at runtime.
4. **You don't actually need it.** The variation between CADs is in their
   *plugin host*, not in what we want to do (push files). Move the variable
   part into a tiny shell, keep the heavy lifting in one well-tested daemon.

So we have **one daemon (this repo)** and optional tiny plugin shells later, in
three tiers:

| Tier | How it works                                                          | Coverage                                |
| ---- | --------------------------------------------------------------------- | --------------------------------------- |
| 1    | Daemon's filesystem watcher: user exports IFC to an inbox folder      | **Every CAD that exports IFC.**         |
| 2    | One-line macro that calls the host's `Export IFC` into that inbox     | Vectorworks, Bentley, Allplan, Tekla    |
| 3    | Native plugin button that feeds the same inbox/API path               | Revit, Rhino, ArchiCAD, SketchUp, AutoCAD |

Tier 1 is the MVP. Every BIM/CAD product on the market that
matters can export IFC. So even on day zero, before we have shipped any
plugins, every architect can use the system: install `vex-bridge`, choose an
inbox folder, export IFC there from their CAD of choice.

## Layout

```
crates/
  vex-bridge/            ← the daemon binary + library
  vex-bridge-protocol/   ← request/response types (serde) for plugins
plugins/
  generic-watch-folder/  ← standalone IFC inbox instructions
  revit-csharp/          ← optional future accelerator: Revit external command
  rhino-python/          ← optional future accelerator: Rhino plugin (single .py file)
docs/
  adding-a-cad.md        ← how to add a new CAD adapter
  early-access-distribution.md ← direct GitHub Release distribution plan
```

## Local API (v1)

All endpoints under `http://127.0.0.1:7878/v1/`. Every authenticated request
must carry the `X-Vex-Bridge-Token` header — its value lives at
`<config_dir>/access-token` (mode 0600). This stops a malicious webpage in
the user's browser from talking to the daemon (browsers cannot read disk).

The local dashboard is served at `http://127.0.0.1:7878/ui` and injects the
per-user token into that same-origin page. Open it with `vex-bridge dashboard`
or from the native `vex-tray` menu.

| Method | Path                | Auth | Purpose                                  |
| ------ | ------------------- | ---- | ---------------------------------------- |
| GET    | `/v1/health`        | no   | Daemon liveness + version                |
| GET    | `/v1/pair/status`   | yes  | Is the daemon paired with an account?    |
| POST   | `/v1/pair/start`    | yes  | Get a pairing code + URL                 |
| GET    | `/v1/setup/status`  | yes  | First-run state for desktop setup        |
| POST   | `/v1/setup/inbox`   | yes  | Create/update the first watched inbox    |
| GET    | `/v1/watch/status`  | yes  | Active watcher + project status          |
| GET    | `/v1/activity/recent` | yes | Recent processing/commit/error events    |
| GET    | `/v1/projects`      | yes  | Local project rows for desktop UI        |
| GET    | `/v1/projects/:id/history` | yes | Commit list for one project       |
| GET    | `/v1/projects/:id/changes?from=&to=` | yes | Selected visual diff for 2D/3D views |
| POST   | `/v1/repo/register` | yes  | Map an Architur project to a local inbox |
| POST   | `/v1/repo/push`     | yes  | Manually import/commit/push latest IFC   |

The filesystem watcher uses the same import/commit/push path automatically when
an IFC export settles in a configured inbox. `vex-bridge` handles the desktop
agent work; the `vex` binary owns IFC metadata extraction, import, semantic
diffing, and history. New inbox registrations are activated immediately in the
running daemon, so the first-run UI does not need to ask users to restart.

## Build & run

```
cargo build --release
./target/release/vex-bridge pair --device-label "Vex on this machine"
./target/release/vex-bridge start            # leaves it in foreground
./target/release/vex-bridge dashboard        # opens http://127.0.0.1:7878/ui
cargo build --release -p vex-bridge --features tray
./target/release/vex-tray                    # native tray/menu bar entry point
```

Release bundles are self-contained: `vex-bridge`, `vex-tray`, and the matching
`vex` engine binary live in the same extracted folder. Unless `vex_bin` is set
explicitly in `config.toml`, the bridge prefers that co-located `vex` binary
before falling back to `vex` on `PATH`.

For production the daemon should be supervised by `launchd` (macOS),
`systemd --user` (Linux), or `nssm` (Windows). Sample units land here in a
later milestone.

## Security notes

- The daemon binds **only** to `127.0.0.1`. It is not reachable from another
  machine on the LAN.
- The SSH private key never touches disk: it's generated in-process and the
  32-byte seed is stored in the OS keychain.
- The access token is a 256-bit random value written with mode `0600`.
- All plugin requests are validated against the token in constant time.
- The daemon never executes shell strings — all `vex` invocations are
  argument vectors.

## License

Apache-2.0 — same as the parent vex project.
