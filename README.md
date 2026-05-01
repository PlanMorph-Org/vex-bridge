# vex-bridge

> The local agent that lets **any** CAD tool push to architur.

Architects don't know what SSH is, and they shouldn't have to. `vex-bridge`
is a small daemon that runs on the user's machine, holds their key in the OS
keychain, and exposes a tiny localhost HTTP API so a 50-line plugin in any CAD
language can do `vex push` without ever touching a terminal.

```
┌──────────────────────┐    HTTP (loopback)        ┌──────────────────┐
│  Revit / Rhino /     │ ─────────────────────────▶│   vex-bridge     │
│  ArchiCAD / SketchUp │   POST /v1/repo/push      │   daemon         │
│  (per-CAD plugin)    │ ◀─────────────────────────│   (this repo)    │
└──────────────────────┘                            └────────┬─────────┘
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

So we have **one daemon (this repo)** and **many tiny plugin shells**, in
three tiers:

| Tier | How it works                                                          | Coverage                                |
| ---- | --------------------------------------------------------------------- | --------------------------------------- |
| 1    | Native plugin button → `POST 127.0.0.1:7878/v1/repo/push`            | Revit, Rhino, ArchiCAD, SketchUp, AutoCAD |
| 2    | One-line macro that calls the host's `Export IFC` then hits the API   | Vectorworks, Bentley, Allplan, Tekla    |
| 3    | Daemon's filesystem watcher: user picks `Export IFC` to a synced folder | **Every CAD that exports IFC.**         |

Tier 3 is the universal fallback. Every BIM/CAD product on the market that
matters can export IFC. So even on day zero, before we have shipped any
plugins, every architect can use the system: install `vex-bridge`, point it
at a folder, export IFC there from their CAD of choice.

## Layout

```
crates/
  vex-bridge/            ← the daemon binary + library
  vex-bridge-protocol/   ← request/response types (serde) for plugins
plugins/
  revit-csharp/          ← Tier 1 example: Revit external command
  rhino-python/          ← Tier 1 example: Rhino plugin (single .py file)
  generic-watch-folder/  ← Tier 3 instructions
docs/
  adding-a-cad.md        ← how to add a new CAD adapter
  early-access-distribution.md ← direct GitHub Release distribution plan
```

## Local API (v1)

All endpoints under `http://127.0.0.1:7878/v1/`. Every authenticated request
must carry the `X-Vex-Bridge-Token` header — its value lives at
`<config_dir>/access-token` (mode 0600). This stops a malicious webpage in
the user's browser from talking to the daemon (browsers cannot read disk).

| Method | Path                | Auth | Purpose                                  |
| ------ | ------------------- | ---- | ---------------------------------------- |
| GET    | `/v1/health`        | no   | Daemon liveness + version                |
| GET    | `/v1/pair/status`   | yes  | Is the daemon paired with an account?    |
| POST   | `/v1/pair/start`    | yes  | Get a pairing code + URL                 |

`/v1/repo/{init,commit,push}` are reserved in [`vex-bridge-protocol`] and
will land alongside the corresponding `vex` CLI subcommands.

## Build & run

```
cargo build --release
./target/release/vex-bridge pair --device-label "Revit on this Mac"
./target/release/vex-bridge start            # leaves it in foreground
```

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
