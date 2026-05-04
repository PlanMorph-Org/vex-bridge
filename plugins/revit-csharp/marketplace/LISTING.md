# Autodesk App Store listing — vex-bridge for Revit

Copy this verbatim into the App Store submission form at
<https://apps.autodesk.com/MyUploads>.

---

## Title
vex-bridge for Revit

## Tagline (max 100 chars)
One-click semantic version control for Revit models — push to architur, see meaningful diffs.

## Short description (max 500 chars)
Push the model you have open to architur, the cloud platform for semantic version control of BIM. The plugin is a tiny ribbon button that talks to a local daemon (vex-bridge) over loopback — no cloud calls from Revit itself. Pair the machine once, then every Push commits the current model and ships it. On studio.planmorph.software your team sees meaningful diffs ("wall moved 200 mm", "fire-rating added") instead of byte changes, and reviews them in a browser-based 2D/3D viewer. No Navisworks license required.

## Long description
Architects, engineers, and owners have lived without real version control for BIM long enough.
**vex-bridge for Revit** brings the GitHub workflow to Revit: open a model, click *Push to architur*, get a meaning-level diff in the browser.

### What it does
- Adds two buttons to a *vex-bridge* ribbon panel: **Push to architur** and **Pair this device**.
- *Push* prompts for an architur project ID and branch (defaults to `main`), then hands off to the local vex-bridge daemon. The daemon does the real work: commit the model with the [vex](https://github.com/PlanMorph-Org/vex) CLI, push it over SSH to your team's architur instance.
- *Pair* opens the daemon's pairing flow so a one-time approval on studio.planmorph.software binds this Windows account to your architur identity.

### What it does **not** do
- Talk to the internet from inside Revit. The Revit process only touches `http://127.0.0.1:7878` — the local daemon, on loopback. Network egress is the daemon's job and it uses an SSH key that never leaves your machine in plaintext.
- Modify your project files. Push is a read-only Revit transaction; geometry only ever flows out via Revit's own IFC export to the daemon's working directory.
- Phone home with telemetry.

### Why architur
Architur stores Revit/IFC models as a property graph and produces semantic diffs — *"wall W-203 moved 200 mm east, slab gained a fire-rating pset"* — instead of byte deltas. Reviewers see the change in a side-by-side 3D viewer in the browser, no plugin needed on their end. Permitting officials, owners, insurers, and consultants can finally participate in BIM coordination without buying a desktop license.

### Compatibility
Revit 2022, 2023, 2024 (Win64). Revit 2025 support coming soon.

### Required separate install
The vex-bridge desktop daemon (free, open source, Apache-2.0).
Download from <https://studio.planmorph.software/install>.

## Category
Collaboration / Project Management

## Price
Free

## Trial available
No (free)

## Languages
English

## Help URL
https://studio.planmorph.software/docs/revit

## Support email
support@planmorph.software

## Privacy policy URL
https://studio.planmorph.software/legal/privacy

## Screenshots required
1. **Ribbon panel** — Revit ribbon showing the two vex-bridge buttons. (1280×800 PNG)
2. **Push dialog** — the project picker. (1280×800 PNG)
3. **Compare in browser** — architur compare page with side-by-side 3D + the visual diff list. (1920×1080 PNG)
4. **Project page on architur** — repo overview with track list. (1920×1080 PNG)

Place final PNGs in `marketplace/screenshots/` before submission.

## Icon
`marketplace/icon.png` — 512×512 with transparency, no shadow.

## Submission checklist
- [ ] MSI signed with Authenticode certificate (`signtool /a`).
- [ ] MSI installs cleanly into a clean Windows VM with Revit 2024 only.
- [ ] After install, Revit shows the vex-bridge ribbon panel on the *Add-Ins* tab.
- [ ] Clicking *Push* with no daemon running shows the documented error, not a crash.
- [ ] Uninstalling via Add/Remove Programs leaves no files behind under `C:\Program Files\Autodesk\Revit {year}\AddIns\VexBridge` or `%ProgramData%\Autodesk\ApplicationPlugins\VexBridge.bundle`.
- [ ] `ReadMe.html` opens in the default browser when the App Store reviewer clicks Help.
- [ ] App passes Autodesk's automated AVA (App Validation App) tooling.
