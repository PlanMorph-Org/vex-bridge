# Early Access Distribution

Use direct distribution for the standalone Vex desktop agent. The MVP path is
the IFC inbox workflow; no Revit, AutoCAD, Autodesk account, or plug-in install
is required.

## What to ship now

1. Build a tagged GitHub Release from this repository.
2. Send firms the matching raw binary bundle from the release:
   `vex-bridge-<tag>-windows-x86_64.tar.gz`,
   `vex-bridge-<tag>-macos-arm64.tar.gz`, or
   `vex-bridge-<tag>-macos-x86_64.tar.gz`.
3. Include this early-access note in outreach:

   > This is early access software. The release ships raw desktop-agent
   > binaries, not an installer. Extract the bundle, keep `vex`, `vex-bridge`,
   > and `vex-tray` together, and launch the daemon/tray from your normal
   > startup mechanism while we validate the standalone workflow.

The release workflow downloads the matching platform `vex` bundle from the
latest `PlanMorph-Org/vex` GitHub Release. Set the repository variable
`VEX_RELEASE_TAG` when a bridge release must pin a specific engine version. If
the engine repository is private to Actions, set `VEX_RELEASE_TOKEN` with read
access to that release.

## Standalone install behavior

The supported install unit is the desktop agent:

```text
vex-bridge(.exe)
vex-tray(.exe)
vex(.exe)
```

The bundle:

- includes the daemon, tray, and matching `vex` engine binaries,
- includes `SHA256SUMS` for the bundled files,
- does not register login items, scheduled tasks, launchd agents, or services,
- leaves startup, pairing, and project/inbox registration to the user, the
   Architur setup UI, or
  `/v1/repo/register`.

Keep the three binaries in the same directory. If `config.toml` does not set
`vex_bin`, `vex-bridge` automatically uses the bundled `vex` binary next to the
running daemon/tray and only falls back to `vex` on `PATH` when no bundled copy
exists.

## Account connection

Users connect the installed daemon to Architur through the browser pairing flow:

```text
https://studio.planmorph.software/pair?code=<code>
```

After pairing, users choose or create an IFC inbox folder in the setup UI. From
there, they export IFC from any CAD tool into that folder and `vex-bridge`
imports, commits, pushes, and archives the export automatically.

## Revit and AutoCAD

Revit and AutoCAD plug-ins remain source-code examples for future Tier 1
accelerators, but they are not built, published, or installed by the standalone
release workflow.

Do not block early customer conversations on Autodesk review, code signing, or
listing metadata.