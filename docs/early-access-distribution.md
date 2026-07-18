# Early Access Distribution

Use direct distribution for the standalone Vex desktop agent. The MVP path is
the IFC inbox workflow; no Revit, AutoCAD, Autodesk account, or plug-in install
is required.

## What to ship now

1. Build a tagged GitHub Release from this repository.
2. Send Windows firms the one-click setup executable:
   `VexAtlasSetup-<tag>-windows-x86_64.exe`.
3. Send macOS firms the matching raw binary bundle from the release:
   `vex-bridge-<tag>-macos-arm64.tar.gz` or
   `vex-bridge-<tag>-macos-x86_64.tar.gz`.
4. Include this early-access note in outreach:

   > This is early access software. The release ships raw desktop-agent
   > binaries on macOS and a per-user setup executable on Windows. Windows users
   > should run the setup executable; macOS users should extract the bundle, keep
   > `vex`, `vex-bridge`, `vex-tray`, and `vex-desktop` together, and launch the
   > desktop app/tray from their normal startup mechanism while we validate the
   > standalone workflow.

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
vex-desktop(.exe)
vex(.exe)
```

The Windows setup executable:

- installs `vex`, `vex-bridge`, `vex-tray`, `vex-desktop`, and `SHA256SUMS`
   together under the current user's local application folder,
- adds that install folder to the user's PATH,
- registers `vex-tray` to start at sign-in,
- starts `vex-tray` after install, and
- launches the Vex Atlas desktop app so first-run pairing, inbox setup, sync,
   and change review happen from the UI.

The macOS bundle:

- includes the daemon, tray, desktop app, and matching `vex` engine binaries,
- includes `SHA256SUMS` for the bundled files,
- does not register login items, scheduled tasks, launchd agents, or services,
- leaves startup, pairing, and project/inbox registration to the user, the
   Architur setup UI, or
  `/v1/repo/register`.

Keep the four binaries in the same directory. If `config.toml` does not set
`vex_bin`, `vex-bridge` automatically uses the bundled `vex` binary next to the
running daemon/tray and only falls back to `vex` on `PATH` when no bundled copy
exists.

## WebView2 Runtime provisioning (Windows)

The Vex Atlas desktop app is a WebView2-hosted UI, so a usable Microsoft Edge
WebView2 Runtime must be present. The Windows setup executable checks the same
per-machine and per-user registry locations Microsoft's own installers use
(`HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}`,
the non-WOW6432Node HKLM equivalent, and the HKCU equivalent) for a usable
`pv` version value. Most Windows 10/11 machines already have the Evergreen
Runtime from Edge/Windows Update and need no action.

When no usable runtime is detected, and the release pipeline staged Microsoft's
official WebView2 Evergreen Bootstrapper (`MicrosoftEdgeWebView2Setup.exe`)
into the release's `SourceDir`, the setup silently runs that
Microsoft-signed bootstrapper (`/silent /install`) during install — including
unattended/silent installs, since it is a required dependency rather than an
optional app launch. The bootstrapper itself fetches the actual runtime from
Microsoft's own CDN; Vex Atlas never downloads or executes anything at install
time beyond running this pre-staged, official stub.

`release.yml` downloads that bootstrapper from Microsoft's documented
redistribution fwlink and verifies its Authenticode signature is a valid
Microsoft signature before staging it. If the bootstrapper is missing (older
builds, offline packaging, or a failed/unsigned download), `VexAtlasSetup.iss`
detects its absence at compile time and simply omits WebView2 provisioning
from that build (a `#pragma message` warning is emitted); the app relies on
whatever runtime is already on the machine, and the WebView2 team's own
just-in-time install prompt as a last resort. No undocumented or unsigned
binaries are ever bundled.

## Native crash reporting

Every shipped native binary (`vex-bridge`, `vex-tray`, `vex-desktop`) installs
a panic hook at startup (`vex_bridge::crash_report::install`). On panic, it
writes a single redacted JSON crash report to
`<app data dir>\vex-bridge\crash-reports\` (the same app-data root used by
`config.toml`/logs; see `Paths::discover()`), alongside continuing to print to
stderr/the existing log sink as before. Each report includes a UTC timestamp,
the executable name and version, the panicking thread name, the panic
message/location, and a backtrace when `RUST_BACKTRACE` capture is available.
Report contents are passed through the same redaction convention used
elsewhere (home directory collapsed to `~`, known access tokens replaced with
`[redacted-token]`, and long token-shaped strings heuristically replaced with
`[redacted]`) so reports are safe to attach to a support request.

`vex-desktop` and `vex-tray` additionally wrap their startup/run logic in
`catch_unwind`, so instead of the process silently disappearing on a panic,
the desktop app shows a native error dialog and the tray posts a native OS
notification, both naming the crash report's file path so a user can find and
share it.

## Account connection

Users connect the installed daemon to Architur through the browser pairing flow:

```text
https://studio.planmorph.software/pair?code=<code>
```

After pairing, users choose or create an IFC inbox folder in the setup UI. From
there, they export IFC from any CAD tool into that folder and `vex-bridge`
imports, commits, and archives the export automatically. Pushing those commits
to the cloud is user-determined: the desktop app shows how many commits are
ready to push, and the user presses **Push** when they want to sync.

## Revit and AutoCAD

Revit and AutoCAD plug-ins remain source-code examples for future Tier 1
accelerators, but they are not built, published, or installed by the standalone
release workflow.

Do not block early customer conversations on Autodesk review, code signing, or
listing metadata.