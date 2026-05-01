# Early Access Distribution

Use direct distribution until there is enough customer pull to justify the
Autodesk App Store review cycle.

## What to ship now

1. Build a tagged GitHub Release from this repository.
2. Send firms the Windows installer asset from the release:
   `VexAtlasSetup-<tag>-windows-x86_64.exe`.
3. For Revit-heavy beta users, send the cert-free bundle installer instead:
   `install-revit.ps1`, which downloads `VexBridge-bundle.zip` from the latest
   release and installs it under Autodesk's ApplicationPlugins folder.
4. Include this SmartScreen note in outreach:

   > This is early access software. Windows may show a SmartScreen warning
   > because we have not purchased a code signing certificate yet. Click
   > **More info**, then **Run anyway**.

## Revit install behavior

The supported Revit install unit is the Autodesk bundle:

```text
C:\ProgramData\Autodesk\ApplicationPlugins\VexBridge.bundle\
```

If the machine-wide Autodesk folder is missing or not writable,
`install-revit.ps1` lets the user browse for an Autodesk ApplicationPlugins
folder, then falls back to the per-user location:

```text
%APPDATA%\Autodesk\ApplicationPlugins\VexBridge.bundle\
```

The bundle includes:

- `vex.exe`
- `vex-bridge.exe`
- Revit add-ins for 2022 through 2027
- AutoCAD add-ins for 2024 through 2027
- `EarlyAccessInstall.html`, opened after install

## Account connection

Users connect the installed add-in to Architur by clicking **Pair this device**
in Revit. The browser opens to:

```text
https://studio.planmorph.software/pair?code=<code>
```

If they are not signed in, they can sign in at `/login` or create an account at
`/register`; both flows return to the pairing approval page.

## App Store timing

Keep `docs/autodesk-app-store.md` as the future submission checklist. Do not
block early customer conversations on Autodesk review, code signing, or listing
metadata.