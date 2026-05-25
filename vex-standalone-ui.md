# Vex Standalone UI — Design & File Handling Specification

## Context & Pivot

Vex was originally designed with a Revit add-in as the primary ingestion path.
We are replacing that with a **standalone desktop agent** — `vex-bridge` — that
any BIM professional can use regardless of which CAD tool they work in, without
requiring plugin installation, IT approval, or Autodesk credentials.

The Revit add-in is not deleted. It becomes an *optional accelerator* for firms
that want deeper integration. But it is no longer the MVP path.

---

## Core Principle

> Vex does not track saves. Vex tracks **design decisions**.

Architects save in native formats (`.rvt`, `.pln`, `.3dm`). IFC is a deliberate
export — a checkpoint. We lean into that. A `vex commit` is intentional, like a
Git commit, and carries meaning: *"structural walls done," "MEP coordination
round 1," "client review model."*

What we solve is making that checkpoint **as frictionless as possible** — one
folder drop, zero terminal commands, instant feedback.

---

## System Architecture (Standalone)

```
┌─────────────────────────────────────────────────────┐
│                   CAD Tool                          │
│  (Revit / ArchiCAD / Rhino / AutoCAD / Any IFC)    │
│                                                     │
│   File > Export > IFC  ──────────────────────────┐ │
└──────────────────────────────────────────────────│─┘
                                                   │ drop .ifc anywhere
                                                   ▼
                                        ┌──────────────────┐
                                        │   Vex Inbox      │
                                        │   (watched dir)  │
                                        └────────┬─────────┘
                                                 │ fs event
                                                 ▼
                                        ┌──────────────────┐
                                        │  vex-bridge      │
                                        │  daemon          │
                                        │                  │
                                        │ 1. Hash file     │
                                        │ 2. Ask vex for   │
                                        │    IFC metadata  │
                                        │ 3. Match project │
                                        │ 4. Auto-commit   │
                                        └────────┬─────────┘
                                                 │
                              ┌──────────────────┼──────────────────┐
                              ▼                  ▼                  ▼
                       ┌──────────┐      ┌──────────────┐   ┌──────────────┐
                       │ vex CLI  │      │ System Tray  │   │  vex-serve   │
                       │ parser + │      │ App (UI)     │   │  (cloud)     │
                       │ diff     │      │              │   │              │
                       └──────────┘      └──────────────┘   └──────────────┘
```

`vex` owns the IFC parser, import pipeline, semantic diff, and commit history.
`vex-bridge` owns the desktop-agent concerns: inbox watching, pairing, local
status, notifications, settings, and calling `vex` through stable JSON commands.
The bridge keeps a tiny bounded metadata fallback only so older bundled `vex`
binaries do not strand users during upgrades.

---

## The Inbox Folder Model

### What It Is

A single, Vex-controlled directory the user designates as their **IFC drop zone**:

```
~/VexInbox/
```

This folder can be:
- A local folder on the desktop
- A Dropbox / OneDrive / Google Drive synced folder (for team workflows)
- A network share on a firm's file server

### What the User Does

1. Export IFC from their CAD tool as they normally would
2. Save or copy the `.ifc` file into the Vex Inbox folder
3. That's it — vex-bridge handles everything from there

The filename **does not matter**. `MyHouse_FINAL_v3_REVISED_USE_THIS_ONE.ifc`
is handled identically to `model.ifc`.

### What vex-bridge Does (Automatically)

```
New .ifc detected in inbox
         │
         ▼
1. Compute BLAKE3 hash of file
         │
         ├── Hash already seen? → SKIP (duplicate export, no-op)
         │
         ▼
2. Ask `vex ifc-intake --json` for fast IFC metadata
   Extract:
   - IfcProject GUID        → identifies which project this belongs to
   - FILE_NAME.author       → who exported it
   - FILE_DESCRIPTION       → any description the CAD tool wrote
   - Originating application (Revit, ArchiCAD, etc.)
         │
         ▼
3. Match to existing Vex project by IfcProject GUID
         │
         ├── No match found? → Prompt user: "New project detected. Name it?"
         │
         ▼
4. Run semantic diff against HEAD commit of that project through `vex`
         │
         ▼
5. Auto-commit with:
   - Timestamp
   - Author (from IFC header or OS user)
   - Auto-generated summary: "12 walls added, 3 doors modified, 1 slab removed"
         │
         ▼
6. Notify user via system tray
   "Committed: Structural model — 12 walls added"
         │
         ▼
7. Move processed file to .vex/archive/ (keeps inbox clean)
```

---

## File Identity & Deduplication

IFC filenames are meaningless to Vex. Identity is determined by two things:

### 1. Content Hash (BLAKE3)

Every imported file is hashed before anything else. If the hash matches a
previously seen commit, the file is silently skipped. This handles the common
case of an architect exporting the same unchanged model twice.

### 2. IfcProject GUID (Project Routing)

The IFC STEP format always contains an `IfcProject` entity in the first ~100
lines of the file. This entity has a `GlobalId` — a 22-character base64-encoded
UUID that is stable across all exports of the same project from the same tool.

```
#1 = IFCPROJECT('2HnQxDrSH5sBbC4NkVOGR8', $, 'My Building', ...);
```

Vex uses this GUID to route incoming files to the correct project automatically,
regardless of filename, export path, or which machine the export came from.

### Handling GUID Inconsistencies

Some tools regenerate the `IfcProject` GUID on every export (a known bug in
certain Revit configurations). For these cases, vex-bridge falls back to a
**structural fingerprint**:

```
fingerprint = hash(project_name + building_location + primary_author + approximate_element_count)
```

This is fuzzy enough to survive GUID regeneration but specific enough to avoid
false matches between genuinely different projects.

---

## UI: System Tray Application

The primary UI is a **system tray / menu bar app** — lightweight, always
running, never in the way.

### Tray Icon States

| Icon | Meaning |
|------|---------|
| ◉ Green | vex-bridge running, watching inbox |
| ◉ Yellow | Processing a file |
| ◉ Blue | New commit just landed |
| ◉ Red | Error — click to see details |
| ◯ Grey | Paused / not connected |

### Tray Menu (Right-click)

```
Vex — planmorph.software
─────────────────────────
● Watching: ~/VexInbox
  Last commit: 4 minutes ago

  Projects
  ├── Commercial Tower (12 commits)
  ├── Residential Block A (7 commits)
  └── Add new project...

─────────────────────────
  Open Dashboard
  Pause Watching
  Settings
─────────────────────────
  Sign out
  Quit Vex
```

### Commit Notification (Toast)

When a new file is processed, a system notification fires:

```
┌─────────────────────────────────────┐
│ 🏗 Vex — New Commit                  │
│                                     │
│ Commercial Tower                    │
│ 12 walls added · 3 doors modified   │
│ 1 slab removed                      │
│                                     │
│ [View Diff]          [Dismiss]      │
└─────────────────────────────────────┘
```

---

## UI: Dashboard (Main Window)

Opened from the tray or a desktop shortcut. A simple three-panel layout.

```
┌─────────────────────────────────────────────────────────────────┐
│  Vex                                          [Settings] [Sync] │
├──────────────┬──────────────────────────┬───────────────────────┤
│  Projects    │  Commit History          │  Diff View            │
│              │                          │                       │
│ ▶ Tower A    │  ● Today                 │  Commit #12           │
│   Block B    │  12:34  Commit #12       │  "Structural update"  │
│   Warehouse  │  "Structural update"     │                       │
│              │  Lawrence · Revit 2024   │  + 12 IfcWall         │
│  + New       │                          │  ~ 3 IfcDoor          │
│              │  ● Yesterday             │  - 1 IfcSlab          │
│              │  16:02  Commit #11       │                       │
│              │  "MEP coordination"      │  [View full diff]     │
│              │  Oscar · ArchiCAD 27     │  [Export report]      │
│              │                          │                       │
└──────────────┴──────────────────────────┴───────────────────────┘
```

### Projects Panel

- Lists all Vex projects on this machine
- Shows last commit time and total commit count
- `+ New` creates a new project and optionally links it to an IfcProject GUID

### Commit History Panel

- Chronological list of commits for the selected project
- Shows: timestamp, auto-generated summary, author, originating CAD tool
- Clicking a commit loads it in the Diff View
- User can **edit the commit message** inline (the auto-summary is a starting point, not final)

### Diff View Panel

Shows a human-readable semantic diff for the selected commit. The local desktop
dashboard is served by `vex-bridge` at `/ui`, opens via `vex-bridge dashboard`,
and uses the same `vex.visual-diff/1` contract as ConstructIQ so the later
xeokit viewer can mount into the 3D pane without changing the daemon API:

```
Commit #12 — "Structural update"
Lawrence Musyoka · Revit 2024 · 24 May 2026 12:34

ADDED (12)
  + IfcWall  "W-101"  Level 3, Grid A-B   length: 4200mm
  + IfcWall  "W-102"  Level 3, Grid C     length: 3600mm
  ... (10 more)

MODIFIED (3)
  ~ IfcDoor  "D-045"  width: 900mm → 1200mm
  ~ IfcDoor  "D-046"  fire_rating: "" → "60 min"
  ~ IfcDoor  "D-047"  location moved 200mm north

REMOVED (1)
  - IfcSlab  "S-201"  Level 2, Area 42m²
```

---

## UI: First-Run Setup Flow

When vex-bridge is installed for the first time:

```
Step 1 of 3 — Welcome
─────────────────────
Welcome to Vex.
Version control for your BIM models.

[Get Started]


Step 2 of 3 — Set Your Inbox
─────────────────────────────
Vex watches a folder for IFC exports.
Drop any IFC file here and Vex will version it automatically.

Inbox folder:  [~/VexInbox]  [Browse...]

This can be a local folder, Dropbox, or a network share.

[Back]  [Continue]


Step 3 of 3 — Connect Your Account
────────────────────────────────────
Sign in to sync your commits to the cloud and
collaborate with your team.

[Sign in with planmorph.software]

Or use locally only for now →

[Back]  [Start Using Vex]
```

The desktop shell drives this flow through the local daemon API:
- `GET /v1/setup/status` returns whether an inbox is needed, the suggested
  `~/VexInbox/...` path, pairing state, and current watcher status.
- `POST /v1/setup/inbox` creates the inbox directory, persists the `[[watch]]`
  entry, and activates the watcher immediately in the running daemon.
- `GET /v1/watch/status` and `GET /v1/projects` feed the tray menu and dashboard
  without scraping TOML, state JSON, or logs.
- `GET /v1/projects/:project_id/history` feeds the commit-history column.
- `GET /v1/projects/:project_id/changes` feeds the 2D plan and 3D change panes,
  including the local `caught_at_unix` timestamp from bridge state.

---

## Handling the Revit Add-in (Existing Work)

The Revit add-in is not discarded. It becomes **Tier 1 integration** — an
optional enhancement for firms already using Vex that want a tighter workflow.

With the add-in installed, the Revit ribbon gets a **Vex panel**:

```
┌──────────────────────────────┐
│  VEX                         │
│  [Commit]  [Log]  [Diff]    │
└──────────────────────────────┘
```

Clicking **Commit** in Revit:
1. Triggers an IFC export silently in the background (to the Vex inbox)
2. Lets the user type a commit message before exporting
3. vex-bridge picks it up from the inbox as normal — same pipeline

The add-in is just a nicer trigger for the same underlying system. It doesn't
bypass the inbox model — it feeds into it. This means the standalone path and
the add-in path are architecturally identical, which keeps the codebase simple.

---

## Team Workflow (Shared Inbox via Cloud Sync)

For teams, the inbox folder is a shared cloud-synced directory:

```
Dropbox/VexInbox/Commercial-Tower/
  ├── (architect exports here from their machine)
  ├── (structural engineer exports here from theirs)
  └── (vex-bridge on each machine watches the same folder)
```

Each team member runs vex-bridge locally. When anyone drops an IFC into the
shared inbox:
- Every team member's vex-bridge sees the file
- The IfcProject GUID routes it to the correct project
- The author field (from the IFC header or OS user) attributes the commit
- Duplicate processing is prevented by the content hash check

This gives teams a shared commit history without needing a central server — the
cloud sync folder *is* the transport layer. The vex-serve cloud backend (for
firms that want it) is an upgrade, not a requirement.

---

## Summary of Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Inbox folder, not filesystem watcher on native files | Native files (.rvt, .pln) can't be headlessly exported; IFC export is the natural checkpoint |
| Identity by content hash + IfcProject GUID, not filename | Filenames are unreliable; content and project identity are stable |
| System tray as primary UI | Stays out of the way; architects are not developers |
| Auto-commit with editable message | Zero friction for the common case; control available when needed |
| Revit add-in feeds the same inbox pipeline | One code path to maintain; add-in is a UX accelerator, not a separate system |
| Shared cloud folder for teams | No server required for collaboration at small scale |
