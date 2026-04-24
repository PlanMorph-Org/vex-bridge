# Deploying vex-bridge to the Autodesk App Store

This guide covers submitting **vex-bridge** to the Autodesk App Store
(formerly "Autodesk Exchange Apps") for both **Revit** and **AutoCAD**.
It reflects current store policy and the project's actual build outputs
as of v0.2.7.

> A separate listing is required per Autodesk product (one for Revit,
> one for AutoCAD), but both listings can ship the **exact same MSI** —
> our bundle includes add-ins for both products in a single installer.

---

## 1. Eligibility & store requirements

| Requirement | vex-bridge status | Notes |
|---|---|---|
| Bundle format `.bundle` with `PackageContents.xml` | ✅ `installer/PackageContents.xml` | Multi-product bundle (Revit + AutoCAD). |
| One `<Components>` block per supported product-year | ✅ R2022–R2027 + ACAD 2024–2027 | Required because Revit/AutoCAD ignore `SeriesMax`. |
| MSI installer (single-click, idempotent) | ✅ `installer/wix/Product.wxs` | WiX 4. Per-user install. |
| EULA shown / accepted on first run | ✅ `Eula.cs` (Revit + AutoCAD) | Marker file at `%APPDATA%\vex-bridge\eula-accepted`. |
| No internet required for install | ✅ | All binaries shipped inside the bundle. |
| No admin privileges required | ✅ | Per-user MSI; per-user Scheduled Task. |
| No console window flashes | ✅ | `wscript.exe vex-bridge-launch.vbs` (SW_HIDE). |
| Code signing | ⚠ Recommended, **not required** | Standard OV cert is fine; **EV cert NOT required**. |
| Help / docs URL | ✅ `OnlineDocumentation` attribute | https://studio.planmorph.software/docs |
| Icon (32×32 PNG) inside bundle | ✅ `Contents/Resources/icon.png` | Plus separate store-listing artwork (see §4). |
| Uninstall leaves no orphans | ✅ MSI removes Scheduled Task in `Product.wxs` |  |

### Code signing

The Autodesk App Store **does NOT require** an EV certificate. A
standard OV ("Organization Validated") code-signing cert from any major
CA (DigiCert, SSL.com, Sectigo, etc.) is sufficient and removes the
SmartScreen "Unknown publisher" warning after enough installs.

```powershell
# Sign the MSI before uploading:
signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 `
    /n "Architur" path\to\VexBridgeSetup.msi
```

Sign the inner `.exe` files **before** they get harvested into the MSI:

```powershell
signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 `
    /n "Architur" target\release\vex-bridge.exe target\release\vex.exe
```

---

## 2. Build the submission artifact

Both store listings share **one MSI**. From a clean Windows checkout:

```powershell
# 1. Build the Rust binaries (release).
cargo build --release -p vex-bridge
# Build the vex CLI from its own workspace (or use a downloaded release).

# 2. Build the multi-CAD bundle (Revit 2022–2027 + AutoCAD 2024–2027).
cd plugins\revit-csharp\installer
.\build-bundle.ps1 `
    -VexBin       ..\..\..\..\vex\target\release\vex.exe `
    -VexBridgeBin ..\..\..\target\release\vex-bridge.exe

# 3. Wrap the bundle in the WiX MSI.
cd wix
dotnet build Product.wxs   # or: wix build Product.wxs -o VexBridgeSetup.msi
```

Verify the MSI on a clean Windows VM (no Visual Studio / no .NET SDK
installed) — see §6.

---

## 3. Submit to the store

The submission portal lives at <https://apps.autodesk.com>. You need a
free Autodesk Developer account.

### Per-product listing

Submit **two** listings. They can share the same MSI, screenshots, and
description, but each lives in its own product category.

**Revit listing**

- Product: Revit
- Versions supported: **2022, 2023, 2024, 2025, 2026, 2027**
  (must match `<Components>` blocks in `PackageContents.xml`)
- Languages: English (Enu)
- Operating system: Windows 64-bit

**AutoCAD listing**

- Product: AutoCAD (also covers AutoCAD verticals via `Platform="AutoCAD*"`)
- Versions supported: **2024, 2025, 2026, 2027**
- Languages: English (Enu)
- Operating system: Windows 64-bit

### Required submission fields

| Field | Recommended content |
|---|---|
| App name | `vex-bridge for Revit` / `vex-bridge for AutoCAD` |
| Short description | "Push Revit models / AutoCAD drawings to architur with one click." |
| Long description | Markdown allowed; describe pairing flow, the architur web app, EULA, privacy. |
| Price | Free |
| Help URL | https://studio.planmorph.software/docs |
| Privacy policy URL | https://studio.planmorph.software/privacy |
| EULA | Apache-2.0 (paste full text or link); **also bundled in-app** via `Eula.cs`. |
| Support email | support@planmorph.software |
| Categories | "Collaboration" + "Cloud" |

### Required artwork

| Asset | Spec |
|---|---|
| App icon | 256×256 PNG, transparent background |
| Logo for store header | 660×255 PNG |
| Screenshots | At least 3, 1280×720 PNG/JPG (ribbon button, pairing dialog, push success) |
| Optional: short demo video | MP4 ≤100 MB or YouTube link |

Place the in-bundle 32×32 icon at
`plugins/revit-csharp/marketplace/icon.png` so `build-bundle.ps1`
includes it under `Contents/Resources/`.

---

## 4. Versioning checklist (every release)

The MSI relies on `MajorUpgrade`, so **the version must increase every
submission** or the upgrade will be rejected by Windows Installer.

Bump in lock-step:

- [ ] `Cargo.toml` → `workspace.package.version`
- [ ] `plugins/revit-csharp/VexBridgeRevit.csproj` → `<Version>`
- [ ] `plugins/autocad-csharp/VexBridgeAutoCAD.csproj` → `<Version>`
- [ ] `plugins/revit-csharp/installer/PackageContents.xml` →
      `AppVersion`, `FriendlyVersion`, every `<ComponentEntry Version=...>`
- [ ] `plugins/revit-csharp/installer/wix/Product.wxs` → `Version=`
- [ ] Tag the repo: `git tag vX.Y.Z && git push --tags`

The store also requires a human-readable changelog per submission.

---

## 5. EULA requirement (App Store policy)

The store requires apps to display an EULA **before** any functional
use. vex-bridge satisfies this two ways:

1. **At submission time**: paste the EULA text into the "EULA" field of
   each listing (Apache-2.0 by default — see `LICENSE`).
2. **At runtime**: `Eula.cs` shows a modal `WinForms` dialog the first
   time the user invokes any vex-bridge command. On accept, a marker
   file is written to `%APPDATA%\vex-bridge\eula-accepted`. Push and
   Pair commands return `Cancelled` if the user declines.

To force a re-prompt (e.g. for testing), delete the marker file or run
the `VEXEULA` command in AutoCAD.

---

## 6. Pre-submission test checklist

Run on a **clean** Windows 10 + Windows 11 VM with no developer tooling:

- [ ] MSI installs without admin elevation prompt
- [ ] Scheduled Task `VexBridgeStartup` exists for the current user
- [ ] Logging out and back in starts `vex-bridge.exe` with **no
      console window flash** (verify via Task Manager)
- [ ] Open Revit 2022/2024/2025/2026/2027 — `vex-bridge` ribbon tab
      appears in each
- [ ] Open AutoCAD 2024/2025/2026/2027 — `VEXPUSH` / `VEXPAIR`
      commands resolve
- [ ] First `Push` or `Pair` shows the EULA modal; declining cancels
      the command
- [ ] After accepting, pairing flow completes against a staging
      architur instance
- [ ] Push uploads a small `.rvt` and `.dwg` end-to-end
- [ ] Uninstalling the MSI removes:
      - `%ProgramData%\Autodesk\ApplicationPlugins\VexBridge.bundle\`
      - The `VexBridgeStartup` Scheduled Task
      - `%APPDATA%\vex-bridge\` is **preserved** (per-user state, by design)

---

## 7. Review timeline & common rejection reasons

Autodesk's review typically takes **5–10 business days** for the first
submission and **2–5 business days** for updates. Common rejections:

| Reason | How vex-bridge avoids it |
|---|---|
| Add-in doesn't load in one of the declared years | We ship a per-year DLL **and** a per-year `<Components>` block. |
| Console window appears at logon | wscript+VBS launcher (SW_HIDE). |
| EULA not shown to end user | Runtime EULA modal + marker file. |
| MSI requires admin / asks for reboot | Per-user install; no reboot triggers. |
| Uninstall leaves Scheduled Task / files | `Product.wxs` `RegisterTask`/`UnregisterTask` custom actions. |
| Crash if Revit/AutoCAD started while daemon already running | `BundledBin.IsDaemonRunning()` guard makes startup idempotent. |
| Outbound network call without consent | Daemon only contacts architur **after** pairing (user-initiated). |

---

## 8. Post-submission

Once approved, the listing is live at:

- `https://apps.autodesk.com/RVT/en/Detail/Index?id=<listing-id>` (Revit)
- `https://apps.autodesk.com/ACD/en/Detail/Index?id=<listing-id>` (AutoCAD)

The store also exposes a JSON API for download counts, ratings, and
review text — see <https://apps.autodesk.com/Publisher/Reports>.

For updates, repeat §2 with bumped versions, then upload the new MSI to
each listing's "Update" tab.
