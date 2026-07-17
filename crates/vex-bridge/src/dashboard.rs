pub fn render(token: &str) -> String {
    let token_json = serde_json::to_string(token).unwrap_or_else(|_| "\"\"".to_string());
    DASHBOARD_HTML.replace("__VEX_TOKEN__", &token_json)
}

const DASHBOARD_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Vex</title>
<style>
:root {
  color-scheme: dark;
  --bg: #101112;
  --panel: #181a1b;
  --panel-2: #202325;
  --line: #34383b;
  --text: #f2f1ec;
  --muted: #a8aaa7;
  --subtle: #747873;
  --green: #43c26b;
  --red: #e05a47;
  --amber: #d99a2b;
  --blue: #4b8fe3;
  --violet: #9b6bd3;
}
* { box-sizing: border-box; }
html, body { height: 100%; }
body {
  margin: 0;
  background: var(--bg);
  color: var(--text);
  font: 13px/1.45 system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
button, input, select {
  font: inherit;
}
button {
  border: 1px solid var(--line);
  background: var(--panel-2);
  color: var(--text);
  border-radius: 6px;
  padding: 7px 10px;
  cursor: pointer;
}
button:hover { border-color: #596066; }
button.primary { background: #f2f1ec; color: #111; border-color: #f2f1ec; }
button:disabled { opacity: .45; cursor: default; }
.app {
  height: 100%;
  display: grid;
  grid-template-rows: 48px 1fr 26px;
}
.topbar {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 0 16px;
  border-bottom: 1px solid var(--line);
  background: #141617;
}
.brand { font-weight: 700; letter-spacing: 0; }
.status-dot { width: 9px; height: 9px; border-radius: 50%; background: var(--subtle); }
.status-dot.ok { background: var(--green); }
.status-dot.warn { background: var(--amber); }
.toolbar-spacer { flex: 1; }
.update-banner {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 9px 16px;
  font-size: 13px;
  border-bottom: 1px solid var(--line);
}
.update-banner.info { background: #14233a; color: #cfe3ff; border-bottom-color: #1f3b63; }
.update-banner.warn { background: #3a2c10; color: #ffe6b0; border-bottom-color: #5c4516; }
.update-banner .ub-text { flex: 1; min-width: 0; }
.update-banner .ub-text strong { font-weight: 650; }
.update-banner .ub-sub { opacity: 0.8; margin-left: 8px; font-size: 12px; }
.update-banner button {
  background: transparent;
  border: 1px solid currentColor;
  color: inherit;
  padding: 4px 12px;
  border-radius: 6px;
  font-size: 12px;
  cursor: pointer;
  opacity: 0.9;
}
.update-banner button:hover { opacity: 1; }
.update-banner button.ub-apply { background: #cfe3ff; color: #0b3a66; border-color: #cfe3ff; font-weight: 600; }
.update-banner button.ub-apply:disabled { opacity: 0.6; cursor: default; }
.update-banner button.ub-dismiss { border-color: transparent; opacity: 0.65; }
.update-banner button.ub-dismiss:hover { opacity: 1; }
.main {
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(180px, 260px) minmax(210px, 320px) minmax(360px, 1fr);
}
.sidebar, .history, .viewer {
  min-height: 0;
  border-right: 1px solid var(--line);
  background: var(--panel);
}
.viewer { border-right: 0; display: grid; grid-template-rows: auto minmax(0, 1fr) auto; }
.panel-head {
  height: 46px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 0 14px;
  border-bottom: 1px solid var(--line);
}
.panel-title { font-weight: 650; }
.list { overflow: auto; height: calc(100% - 46px); }
.row {
  width: 100%;
  text-align: left;
  border: 0;
  border-bottom: 1px solid rgba(255,255,255,0.05);
  border-radius: 0;
  background: transparent;
  padding: 11px 14px;
  display: grid;
  gap: 4px;
}
.row:hover, .row.active { background: var(--panel-2); }
.row-title { font-weight: 620; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.row-meta { color: var(--muted); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.project-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 34px;
  align-items: stretch;
  border-bottom: 1px solid rgba(255,255,255,0.05);
}
.project-row .row { border-bottom: 0; }
.icon-button {
  align-self: center;
  justify-self: center;
  width: 28px;
  height: 28px;
  padding: 0;
  display: grid;
  place-items: center;
  color: var(--muted);
}
.icon-button.danger:hover { color: #f09a8e; border-color: rgba(224,90,71,.65); }
.badges { display: flex; flex-wrap: wrap; gap: 6px; }
.badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  min-height: 22px;
  padding: 2px 7px;
  border: 1px solid var(--line);
  border-radius: 999px;
  color: var(--muted);
  background: rgba(255,255,255,0.03);
  font-size: 12px;
}
.badge.added { border-color: rgba(67,194,107,.45); color: #8fe5a7; }
.badge.removed { border-color: rgba(224,90,71,.45); color: #f09a8e; }
.badge.modified { border-color: rgba(217,154,43,.45); color: #f0c06a; }
.badge.moved { border-color: rgba(75,143,227,.45); color: #93baf0; }
.badge.renamed { border-color: rgba(155,107,211,.45); color: #c5a8e7; }
.badge.internal { border-color: rgba(255,255,255,.12); color: var(--muted); }
.viewer-head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 12px;
  padding: 12px 14px;
  border-bottom: 1px solid var(--line);
}
.commit-line { font-weight: 650; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.time-line { color: var(--muted); margin-top: 3px; }
.view-toggle {
  display: inline-flex;
  align-self: start;
  border: 1px solid var(--line);
  border-radius: 7px;
  overflow: hidden;
}
.view-toggle button {
  border: 0;
  border-radius: 0;
  background: transparent;
}
.view-toggle button.active { background: #f2f1ec; color: #111; }
.view-grid {
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 1px;
  background: var(--line);
}
.view-grid.dim-3d #planPane { display: none; }
.view-grid.dim-2d #modelPane { display: none; }
#planCanvas { cursor: grab; }
#planCanvas.panning { cursor: grabbing; }
.view-pane {
  min-width: 0;
  min-height: 0;
  background: #111313;
  display: grid;
  grid-template-rows: 38px 1fr;
  position: relative;
}
.view-pane header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  border-bottom: 1px solid rgba(255,255,255,0.07);
  color: var(--muted);
}
canvas { width: 100%; height: 100%; display: block; }
.plan-tools { display: flex; align-items: center; gap: 6px; }
.plan-tools select {
  background: #121414;
  color: var(--text);
  border: 1px solid var(--line);
  border-radius: 5px;
  padding: 3px 6px;
  font-size: 12px;
  max-width: 150px;
}
.plan-tools .tool-btn { padding: 3px 8px; }
.view-status {
  position: absolute;
  inset: 38px 0 0 0;
  display: grid;
  place-items: center;
  padding: 18px;
  color: var(--muted);
  text-align: center;
  pointer-events: none;
}
.view-status:empty { display: none; }
.orbit-hint {
  position: absolute;
  left: 50%;
  bottom: 14px;
  transform: translateX(-50%);
  padding: 6px 12px;
  border: 1px solid var(--line);
  border-radius: 999px;
  background: rgba(17, 19, 19, 0.82);
  color: var(--muted);
  font-size: 12px;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.4s ease;
  z-index: 4;
}
.orbit-hint.show { opacity: 1; }
.empty {
  padding: 18px;
  color: var(--muted);
}
.change-table {
  overflow: auto;
  border-top: 1px solid var(--line);
  min-height: 0;
  max-height: 190px;
}
table { width: 100%; border-collapse: collapse; }
th, td { text-align: left; border-bottom: 1px solid rgba(255,255,255,0.06); padding: 8px 10px; }
th { color: var(--muted); font-weight: 600; position: sticky; top: 0; background: var(--panel); }
.kind { font-weight: 700; }
.kind.added { color: var(--green); }
.kind.removed { color: var(--red); }
.kind.modified { color: var(--amber); }
.kind.moved { color: var(--blue); }
.kind.renamed { color: var(--violet); }
.kind.unchanged { color: var(--muted); }
.layer-chip {
  display: inline-block;
  margin-left: 6px;
  padding: 1px 6px;
  border-radius: 9px;
  border: 1px solid rgba(255,255,255,.14);
  font-size: 10px;
  letter-spacing: .04em;
  text-transform: uppercase;
  color: var(--muted);
  vertical-align: 1px;
}
.layer-chip.shape { border-color: rgba(75,143,227,.5); color: #93baf0; }
.layer-chip.property { border-color: rgba(245,191,68,.5); color: #f0cf93; }
.change-row.expandable { cursor: pointer; }
.change-row.expandable:hover { background: rgba(255,255,255,.035); }
.caret { display: inline-block; width: 12px; color: var(--muted); transition: transform .12s ease; }
.change-row.open .caret { transform: rotate(90deg); }
.change-detail > td { padding: 2px 10px 8px 30px; background: rgba(255,255,255,.02); }
.delta-table { width: 100%; border-collapse: collapse; }
.delta-table td { border-bottom: 1px solid rgba(255,255,255,.05); padding: 4px 8px; font-size: 12px; }
.delta-table tr:last-child td { border-bottom: none; }
.delta-table .dk { color: var(--muted); width: 32%; white-space: nowrap; }
.delta-table .dv { font-family: var(--mono, ui-monospace, monospace); word-break: break-word; }
.delta-table .dv.before { color: #f0a9a0; }
.delta-table .dv.arrow { color: var(--muted); width: 16px; text-align: center; }
.delta-table .dv.after { color: #8fd0a0; }
.setup {
  display: none;
  position: fixed;
  inset: 64px auto auto 50%;
  width: min(560px, calc(100vw - 32px));
  transform: translateX(-50%);
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel);
  box-shadow: 0 24px 80px rgba(0,0,0,.45);
  z-index: 4;
}
.setup.open { display: block; }
.setup form { display: grid; gap: 10px; padding: 14px; }
.modal-actions { display: flex; justify-content: flex-end; gap: 8px; }
.radio-group { display: grid; gap: 8px; }
.radio-option {
  display: grid;
  grid-template-columns: 18px minmax(0, 1fr);
  gap: 8px;
  align-items: start;
  color: var(--text);
}
.radio-option input { width: auto; margin-top: 2px; }
.radio-option span { color: var(--muted); font-size: 12px; }
.danger-text { color: #f09a8e; }
.field { display: grid; gap: 5px; }
.field label { color: var(--muted); font-size: 12px; }
.field input {
  width: 100%;
  border: 1px solid var(--line);
  background: #121414;
  color: var(--text);
  border-radius: 6px;
  padding: 8px 9px;
}
.field select {
  width: 100%;
  border: 1px solid var(--line);
  background: #121414;
  color: var(--text);
  border-radius: 6px;
  padding: 8px 9px;
}
.push-preview { display: grid; gap: 10px; padding: 0 14px 14px; }
.preview-grid { display: grid; grid-template-columns: 110px minmax(0, 1fr); gap: 7px 12px; }
.preview-grid .label { color: var(--muted); }
.preview-grid .value { min-width: 0; overflow-wrap: anywhere; }
.preview-warning { padding: 8px 10px; border: 1px solid rgba(217,154,43,.45); color: #f0c06a; background: rgba(217,154,43,.08); border-radius: 6px; }
.viewer-toolbar {
  position: absolute;
  top: 44px;
  left: 10px;
  z-index: 3;
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  max-width: calc(100% - 20px);
}
.tool-btn {
  padding: 4px 8px;
  font-size: 12px;
  background: rgba(20,22,23,.82);
  border: 1px solid var(--line);
  border-radius: 5px;
  color: var(--muted);
}
.tool-btn:hover { color: var(--text); border-color: #596066; }
.tool-btn.active { background: #f2f1ec; color: #111; border-color: #f2f1ec; }
.tool-select {
  font-size: 12px;
  background: rgba(20,22,23,.82);
  border: 1px solid var(--line);
  border-radius: 5px;
  color: var(--muted);
  padding: 3px 6px;
  max-width: 150px;
}
.tool-select:hover { color: var(--text); border-color: #596066; }
.viewer-toolbar .sep { width: 1px; align-self: stretch; background: var(--line); margin: 2px 1px; }
.gizmo {
  position: absolute;
  right: 10px;
  bottom: 10px;
  width: 86px;
  height: 86px;
  z-index: 3;
  pointer-events: none;
}
.props-panel {
  position: absolute;
  top: 44px;
  right: 10px;
  width: 244px;
  max-height: calc(100% - 64px);
  overflow: auto;
  z-index: 4;
  background: rgba(18,20,21,.94);
  border: 1px solid var(--line);
  border-radius: 7px;
  padding: 10px 12px 12px;
  display: none;
}
.props-panel.open { display: block; }
.props-panel h4 { margin: 0 26px 8px 0; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.props-panel .prop { display: grid; grid-template-columns: 92px 1fr; gap: 6px; padding: 3px 0; border-bottom: 1px solid rgba(255,255,255,.05); font-size: 12px; }
.props-panel .prop .k { color: var(--subtle); }
.props-panel .prop .v { color: var(--text); word-break: break-word; }
.props-panel .close { position: absolute; top: 7px; right: 8px; padding: 2px 7px; }
.section-row {
  position: absolute;
  left: 10px;
  bottom: 10px;
  z-index: 3;
  display: none;
  align-items: center;
  gap: 8px;
  background: rgba(20,22,23,.85);
  border: 1px solid var(--line);
  border-radius: 5px;
  padding: 5px 9px;
  color: var(--muted);
  font-size: 12px;
}
.section-row.open { display: flex; }
.section-row input[type=range] { width: 130px; accent-color: var(--blue); }
.statusbar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 0 14px;
  border-top: 1px solid var(--line);
  background: #141617;
  color: var(--muted);
  font-size: 12px;
  overflow: hidden;
  white-space: nowrap;
}
.statusbar .sb-item { display: inline-flex; align-items: center; gap: 6px; min-width: 0; }
.statusbar .sb-item strong { color: var(--text); font-weight: 620; }
.statusbar .sb-spacer { flex: 1; }
.statusbar .sb-action { margin-left: 10px; padding: 2px 8px; font: inherit; font-size: 11px; color: var(--text); background: transparent; border: 1px solid var(--subtle); border-radius: 5px; cursor: pointer; flex: none; }
.statusbar .sb-action:hover { border-color: var(--text); }
.statusbar .sb-action:disabled { opacity: 0.55; cursor: default; }
.sb-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--subtle); flex: none; }
.sb-dot.ok { background: var(--green); }
.sb-dot.warn { background: var(--amber); }
@media (max-width: 760px) {
  .main { grid-template-columns: 1fr; grid-template-rows: minmax(160px, 220px) minmax(180px, 240px) minmax(220px, 1fr); }
  .sidebar, .history { border-right: 0; border-bottom: 1px solid var(--line); }
  .view-grid { grid-template-columns: 1fr; grid-template-rows: minmax(0, 1fr); }
}
</style>
</head>
<body>
<div class="app">
  <div class="topbar">
    <div class="status-dot" id="statusDot"></div>
    <div class="brand">Vex Atlas</div>
    <div id="topStatus" class="row-meta">Loading</div>
    <div class="toolbar-spacer"></div>
    <button id="pairButton">Pair Device</button>
    <button id="setupButton">Add Inbox</button>
    <button id="syncButton" title="Push committed changes to the cloud">Push</button>
    <button class="primary" id="refreshButton">Refresh</button>
  </div>
  <div class="update-banner" id="updateBanner" style="display:none"></div>
  <main class="main">
    <section class="sidebar">
      <div class="panel-head"><div class="panel-title">Projects</div><div id="projectCount" class="row-meta"></div></div>
      <div id="projects" class="list"></div>
    </section>
    <section class="history">
      <div class="panel-head"><div class="panel-title">Commit History</div><div id="historyMeta" class="row-meta"></div></div>
      <div id="history" class="list"></div>
    </section>
    <section class="viewer">
      <div class="viewer-head">
        <div>
          <div id="changeTitle" class="commit-line">No project selected</div>
          <div id="changeTime" class="time-line"></div>
        </div>
        <div>
          <div class="view-toggle" id="dimToggle">
            <button type="button" data-dim="3d">3D</button>
            <button type="button" data-dim="2d" class="active">2D</button>
          </div>
          <div class="view-toggle" id="viewToggle">
            <button type="button" data-mode="full" class="active">Full Model</button>
            <button type="button" data-mode="changes">Changes Only</button>
          </div>
          <button class="primary" id="addIfcButton" type="button" disabled title="Add an IFC file to this project's inbox">Add IFC</button>
          <input id="addIfcInput" type="file" accept=".ifc" style="display:none">
          <div class="badges" id="countBadges"></div>
        </div>
      </div>
      <div class="view-grid dim-2d" id="viewGrid">
        <div class="view-pane" id="planPane">
          <header>
            <span>2D Plan</span>
            <span class="plan-tools">
              <select id="planLevel" title="Floor level"><option value="auto">Auto level</option></select>
              <button class="tool-btn" id="planModeBtn" data-act="plan-mode" title="Toggle section plan / top view">Plan</button>
              <span id="planMeta"></span>
            </span>
          </header>
          <canvas id="planCanvas"></canvas>
          <div class="view-status" id="planStatus"></div>
        </div>
        <div class="view-pane" id="modelPane">
          <header><span>3D Model</span><span id="modelMeta"></span></header>
          <canvas id="modelCanvas"></canvas>
          <div class="orbit-hint" id="orbitHint">Drag to orbit · scroll to zoom · right-drag to pan</div>
          <div class="viewer-toolbar" id="viewerToolbar">
            <button class="tool-btn" data-act="fit" title="Fit to model (F)">Fit</button>
            <span class="sep"></span>
            <button class="tool-btn" data-act="view-iso" title="Isometric view">Iso</button>
            <button class="tool-btn" data-act="view-top" title="Top view">Top</button>
            <button class="tool-btn" data-act="view-front" title="Front view">Front</button>
            <button class="tool-btn" data-act="view-right" title="Right view">Right</button>
            <span class="sep"></span>
            <button class="tool-btn" data-act="proj" id="projBtn" title="Toggle perspective / orthographic">Persp</button>
            <button class="tool-btn" data-act="section" id="sectionBtn" title="Section / cut plane">Section</button>
            <span class="sep"></span>
            <select class="tool-select" id="modelLevel" title="Isolate a floor (full structure by default)"><option value="full">Full structure</option></select>
          </div>
          <div class="section-row" id="sectionRow">
            <span>Cut</span>
            <input type="range" id="sectionSlider" min="0" max="100" value="100" title="Section height">
            <button class="tool-btn" data-act="section-off" title="Clear section">Clear</button>
          </div>
          <div class="props-panel" id="propsPanel">
            <button class="tool-btn close" data-act="props-close" type="button">Close</button>
            <h4 id="propsTitle">Element</h4>
            <div id="propsBody"></div>
          </div>
          <canvas class="gizmo" id="gizmoCanvas" width="172" height="172"></canvas>
          <div class="view-status" id="modelStatus"></div>
        </div>
      </div>
      <div class="change-table">
        <table>
          <thead><tr><th>Kind</th><th>Element</th><th>Change</th></tr></thead>
          <tbody id="changeRows"></tbody>
        </table>
      </div>
    </section>
  </main>
  <footer class="statusbar">
    <span class="sb-item"><span class="sb-dot" id="sbDot"></span><span id="sbConn">Connecting…</span></span>
    <span class="sb-item" id="sbAccountItem" style="display:none">Signed in as&nbsp;<strong id="sbAccount"></strong></span>
    <span class="sb-item" id="sbWatch"></span>
    <span class="sb-spacer"></span>
    <span class="sb-item" id="sbActivity"></span>
    <span class="sb-item" id="sbVersions"></span>
    <button class="sb-action" id="sbDiag" type="button" title="Copy a diagnostics report to share with support">Copy diagnostics</button>
    <button class="sb-action" id="sbRepair" type="button" title="Restart the Vex background daemon">Repair</button>
  </footer>
</div>
<div class="setup" id="setupPanel">
  <div class="panel-head"><div class="panel-title">Add Inbox</div><button id="closeSetup" type="button">Close</button></div>
  <form id="setupForm">
    <div class="field"><label for="projectName">Project Name</label><input id="projectName" required placeholder="Commercial Tower"></div>
    <div class="field"><label for="projectId">Project ID (auto)</label><input id="projectId" readonly placeholder="vex-…"></div>
    <div class="field" id="browseField" style="display:none"><button type="button" id="browseFolder">Browse for folder…</button></div>
    <div class="row-meta" id="inboxHint">A managed folder named after the project will be created inside VexInbox.</div>
    <button class="primary" type="submit">Save Inbox</button>
  </form>
</div>
<div class="setup" id="deletePanel">
  <div class="panel-head"><div class="panel-title">Delete Project</div><button id="closeDelete" type="button">Close</button></div>
  <form id="deleteForm">
    <div class="row-meta" id="deleteProjectText"></div>
    <div class="radio-group">
      <label class="radio-option"><input type="radio" name="deletePolicy" value="keep_folder" checked><div>Remove from Vex Desktop<br><span>Keep the project folder and IFC history on disk.</span></div></label>
      <label class="radio-option"><input type="radio" name="deletePolicy" value="archive_folder"><div>Archive folder<br><span>Rename the folder inside VexInbox.</span></div></label>
      <label class="radio-option"><input type="radio" name="deletePolicy" value="delete_folder"><div class="danger-text">Delete folder permanently<br><span>Only folders inside VexInbox are allowed.</span></div></label>
    </div>
    <div class="modal-actions"><button type="button" id="cancelDelete">Cancel</button><button class="primary" type="submit">Delete</button></div>
  </form>
</div>
<div class="setup" id="pushPanel">
  <div class="panel-head"><div class="panel-title">Review Push</div><button id="closePush" type="button">Close</button></div>
  <form id="pushForm">
    <div class="field"><label for="cloudProject">Cloud Project</label><select id="cloudProject" required><option value="">Loading projects…</option></select></div>
    <div class="push-preview" id="pushPreview"><div class="row-meta">Loading preview…</div></div>
    <div class="modal-actions"><button type="button" id="cancelPush">Cancel</button><button class="primary" id="confirmPush" type="submit">Push</button></div>
  </form>
</div>
<script type="importmap">
{
  "imports": {
    "three": "/assets/viewer/three/three.module.js",
    "three/examples/jsm/utils/BufferGeometryUtils": "/assets/viewer/three/examples/jsm/utils/BufferGeometryUtils.js",
    "three/examples/jsm/controls/OrbitControls": "/assets/viewer/three/examples/jsm/controls/OrbitControls.js",
    "three/examples/jsm/loaders/GLTFLoader": "/assets/viewer/three/examples/jsm/loaders/GLTFLoader.js",
    "web-ifc": "/assets/viewer/web-ifc/web-ifc-api.js"
  }
}
</script>
<script type="module">
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader';
import { IFCLoader } from '/assets/viewer/web-ifc-three/IFCLoader.js';

const TOKEN = __VEX_TOKEN__;
const headers = {'X-Vex-Bridge-Token': TOKEN};
const jsonHeaders = {'X-Vex-Bridge-Token': TOKEN, 'Content-Type': 'application/json'};
let selectedProject = null;
let selectedCommit = null;
let projectCommits = [];
let latestChanges = null;
let lastSetup = null;
let currentViewMode = 'full';
let pairPollTimer = null;
let pendingDeleteProject = null;
let cloudProjects = [];
const urlParams = new URLSearchParams(window.location.search);
const requestedProject = urlParams.get('project');
const requestedCommit = urlParams.get('commit');

const els = {
  statusDot: document.getElementById('statusDot'), topStatus: document.getElementById('topStatus'),
  projects: document.getElementById('projects'), history: document.getElementById('history'),
  projectCount: document.getElementById('projectCount'), historyMeta: document.getElementById('historyMeta'),
  changeTitle: document.getElementById('changeTitle'), changeTime: document.getElementById('changeTime'),
  countBadges: document.getElementById('countBadges'), changeRows: document.getElementById('changeRows'),
  planCanvas: document.getElementById('planCanvas'), modelCanvas: document.getElementById('modelCanvas'),
  planMeta: document.getElementById('planMeta'), modelMeta: document.getElementById('modelMeta'),
  planStatus: document.getElementById('planStatus'), modelStatus: document.getElementById('modelStatus'),
  planLevel: document.getElementById('planLevel'), planModeBtn: document.getElementById('planModeBtn'),
  modelLevel: document.getElementById('modelLevel'),
  pairButton: document.getElementById('pairButton'), syncButton: document.getElementById('syncButton'),
  setupPanel: document.getElementById('setupPanel'), setupForm: document.getElementById('setupForm'),
  deletePanel: document.getElementById('deletePanel'), deleteForm: document.getElementById('deleteForm'),
  pushPanel: document.getElementById('pushPanel'), pushForm: document.getElementById('pushForm'),
  cloudProject: document.getElementById('cloudProject'), pushPreview: document.getElementById('pushPreview'),
  confirmPush: document.getElementById('confirmPush'),
  deleteProjectText: document.getElementById('deleteProjectText'),
  inboxHint: document.getElementById('inboxHint'), viewToggle: document.getElementById('viewToggle'),
  dimToggle: document.getElementById('dimToggle'), viewGrid: document.getElementById('viewGrid'),
  projectName: document.getElementById('projectName'), projectId: document.getElementById('projectId'),
  viewerToolbar: document.getElementById('viewerToolbar'), projBtn: document.getElementById('projBtn'),
  sectionBtn: document.getElementById('sectionBtn'), sectionRow: document.getElementById('sectionRow'),
  sectionSlider: document.getElementById('sectionSlider'),
  propsPanel: document.getElementById('propsPanel'), propsTitle: document.getElementById('propsTitle'),
  propsBody: document.getElementById('propsBody'), gizmoCanvas: document.getElementById('gizmoCanvas'),
  modelCanvas2: document.getElementById('modelCanvas'),
  sbDot: document.getElementById('sbDot'), sbConn: document.getElementById('sbConn'),
  sbAccountItem: document.getElementById('sbAccountItem'), sbAccount: document.getElementById('sbAccount'),
  sbWatch: document.getElementById('sbWatch'), sbActivity: document.getElementById('sbActivity'),
  sbVersions: document.getElementById('sbVersions'),
  sbDiag: document.getElementById('sbDiag'), sbRepair: document.getElementById('sbRepair'),
  browseField: document.getElementById('browseField'), browseFolder: document.getElementById('browseFolder'),
  addIfcButton: document.getElementById('addIfcButton'), addIfcInput: document.getElementById('addIfcInput'),
  updateBanner: document.getElementById('updateBanner')
};

// Native desktop bridge (present only inside the vex-desktop window). Falls back
// gracefully to plain browser behaviour when unavailable.
const native = (typeof window !== 'undefined' && window.__vexNative && window.__vexNative.available)
  ? window.__vexNative : null;
let pickedFolderPath = null;
let healthInfo = null;
let updateInfo = null;

let ifcViewer = null;

document.getElementById('refreshButton').addEventListener('click', refresh);
document.getElementById('setupButton').addEventListener('click', () => {
  if (!els.projectId.value.trim()) els.projectId.value = genProjectId();
  els.setupPanel.classList.add('open');
});
els.pairButton.addEventListener('click', startOrPollPairing);
els.syncButton.addEventListener('click', openPushPanel);
els.addIfcButton.addEventListener('click', () => { if (selectedProject) els.addIfcInput.click(); });
if (els.sbDiag) els.sbDiag.addEventListener('click', copyDiagnostics);
if (els.sbRepair) els.sbRepair.addEventListener('click', repairDaemon);els.addIfcInput.addEventListener('change', onAddIfcInput);
document.getElementById('closeSetup').addEventListener('click', () => els.setupPanel.classList.remove('open'));
document.getElementById('closeDelete').addEventListener('click', closeDeletePanel);
document.getElementById('cancelDelete').addEventListener('click', closeDeletePanel);
document.getElementById('closePush').addEventListener('click', closePushPanel);
document.getElementById('cancelPush').addEventListener('click', closePushPanel);
els.setupForm.addEventListener('submit', saveInbox);
els.deleteForm.addEventListener('submit', deleteProject);
els.pushForm.addEventListener('submit', pushSelectedProject);
els.cloudProject.addEventListener('change', renderPushPreview);
els.viewToggle.addEventListener('click', event => {
  const button = event.target.closest('button[data-mode]');
  if (!button) return;
  currentViewMode = button.dataset.mode;
  for (const item of els.viewToggle.querySelectorAll('button')) item.classList.toggle('active', item === button);
  renderChanges(latestChanges);
});
els.dimToggle.addEventListener('click', event => {
  const button = event.target.closest('button[data-dim]');
  if (!button) return;
  setViewDimension(button.dataset.dim);
});
window.addEventListener('resize', () => { if (ifcViewer) ifcViewer.resize(); });
els.viewerToolbar.addEventListener('click', event => {
  const btn = event.target.closest('button[data-act]');
  if (!btn || !ifcViewer) return;
  const act = btn.dataset.act;
  if (act === 'fit') ifcViewer.fit();
  else if (act === 'view-iso') ifcViewer.setView('iso');
  else if (act === 'view-top') ifcViewer.setView('top');
  else if (act === 'view-front') ifcViewer.setView('front');
  else if (act === 'view-right') ifcViewer.setView('right');
  else if (act === 'proj') ifcViewer.toggleProjection();
  else if (act === 'section') ifcViewer.toggleSection();
});
els.sectionRow.addEventListener('click', event => {
  if (event.target.closest('button[data-act="section-off"]') && ifcViewer) ifcViewer.toggleSection(false);
});
els.sectionSlider.addEventListener('input', () => { if (ifcViewer) ifcViewer.setSection(Number(els.sectionSlider.value)); });
if (els.modelLevel) els.modelLevel.addEventListener('change', () => {
  if (!ifcViewer) return;
  const value = els.modelLevel.value;
  ifcViewer.setModelLevel(value === 'full' ? null : Number(value));
});
els.planLevel.addEventListener('change', () => {
  if (!ifcViewer) return;
  const value = els.planLevel.value;
  ifcViewer.setPlanLevel(value === 'auto' ? 0 : Number(value));
});
els.planModeBtn.addEventListener('click', () => {
  if (!ifcViewer) return;
  const next = ifcViewer.planCutMode === 'plan' ? 'top' : 'plan';
  ifcViewer.setPlanCutMode(next);
  els.planModeBtn.textContent = next === 'plan' ? 'Plan' : 'Top';
  els.planModeBtn.classList.toggle('active', next === 'top');
  els.planLevel.disabled = next === 'top';
});
els.propsPanel.addEventListener('click', event => {
  if (event.target.closest('button[data-act="props-close"]') && ifcViewer) ifcViewer.clearSelection();
});
window.addEventListener('keydown', event => {
  if (!ifcViewer) return;
  const tag = (event.target && event.target.tagName) || '';
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
  if (event.key === 'f' || event.key === 'F') ifcViewer.fit();
  else if (event.key === 'Escape') ifcViewer.clearSelection();
});
if (native) {
  els.browseField.style.display = '';
  els.browseFolder.addEventListener('click', pickFolder);
}

async function pickFolder() {
  if (!native) return;
  try {
    const path = await native.pickFolder();
    if (!path) return;
    pickedFolderPath = path;
    const base = path.replace(/[\\/]+$/, '').split(/[\\/]/).pop() || path;
    if (!els.projectName.value.trim()) els.projectName.value = base;
    const root = lastSetup && (lastSetup.inbox_root_path || lastSetup.suggested_inbox_path);
    els.inboxHint.textContent = (root && path.startsWith(root))
      ? `Tracking ${path}`
      : `Will create a managed folder named "${base}" inside ${root || 'VexInbox'}.`;
  } catch (error) {
    els.inboxHint.textContent = `Folder pick failed: ${error.message || error}`;
  }
}

function genProjectId() {
  const uuid = (typeof crypto !== 'undefined' && crypto.randomUUID)
    ? crypto.randomUUID().replace(/-/g, '')
    : (Date.now().toString(16) + Math.random().toString(16).slice(2)).slice(0, 32);
  return `vex-${uuid}`;
}

function setViewDimension(dim) {
  const wanted = dim === '2d' ? '2d' : '3d';
  els.viewGrid.classList.toggle('dim-2d', wanted === '2d');
  els.viewGrid.classList.toggle('dim-3d', wanted === '3d');
  for (const item of els.dimToggle.querySelectorAll('button')) {
    item.classList.toggle('active', item.dataset.dim === wanted);
  }
  if (ifcViewer) requestAnimationFrame(() => ifcViewer.resize());
}

async function api(path, options = {}) {
  const response = await fetch(path, options);
  const raw = await response.text();
  let body = null;
  if (raw) {
    try { body = JSON.parse(raw); } catch (_) { body = null; }
  }
  if (!response.ok) {
    const message = body && body.message ? body.message : `${path} -> ${response.status}`;
    const hint = body && body.hint ? body.hint : null;
    const error = new Error(hint ? `${message} (${hint})` : message);
    error.status = response.status;
    error.code = body && body.code ? body.code : null;
    error.hint = hint;
    error.correlationId = body && body.correlation_id ? body.correlation_id : null;
    throw error;
  }
  if (body === null) {
    throw new Error(`${path} returned an unexpected response.`);
  }
  return body;
}

async function refresh(options = {}) {
  try {
    const setup = await api('/v1/setup/status', {headers});
    lastSetup = setup;
    const paired = setup.pair_status && setup.pair_status.status === 'paired';
    els.statusDot.className = `status-dot ${paired && setup.watch.active_watchers > 0 ? 'ok' : 'warn'}`;
    els.topStatus.textContent = `${pairText(setup.pair_status)} / ${setup.watch.active_watchers}/${setup.watch.configured_projects} watching`;
    updatePairButton(setup.pair_status);
    updatePushButton(setup, paired);
    els.addIfcButton.disabled = !selectedProject;
    if (!pickedFolderPath) {
      els.inboxHint.textContent = `Folders are created inside ${setup.inbox_root_path || setup.suggested_inbox_path || 'VexInbox'}.`;
    }
    renderProjects(setup.watch.projects);
    renderStatusBar(setup);
    if (!selectedProject && setup.watch.projects.length) {
      const requested = setup.watch.projects.find(project => project.project_id === requestedProject);
      await selectProject((requested || setup.watch.projects[0]).project_id);
    } else if (selectedProject && options.reloadSelected !== false) {
      await reloadSelectedProject();
    }
  } catch (error) {
    els.statusDot.className = 'status-dot warn';
    els.topStatus.textContent = error.message;
    els.sbDot.className = 'sb-dot warn';
    els.sbConn.textContent = 'Daemon offline';
  }
}

async function loadHealth() {
  try {
    healthInfo = await api('/v1/health', {headers});
    if (lastSetup) renderStatusBar(lastSetup);
    renderSystemBanner();
  } catch (_) { /* best-effort */ }
}

// Open a URL in the user's real browser. Inside the desktop window this hands
// off to the native shell; in a plain browser it falls back to a new tab.
function openExternalUrl(url) {
  if (native && typeof native.openExternal === 'function') native.openExternal(url);
  else window.open(url, '_blank', 'noopener');
}

// Fetch the (redacted) diagnostics report and copy it to the clipboard so the
// user can paste it into a support thread.
async function copyDiagnostics() {
  const btn = els.sbDiag;
  const original = btn ? btn.textContent : '';
  try {
    const report = await api('/v1/diagnostics', {headers});
    const text = JSON.stringify(report, null, 2);
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(text);
    } else {
      const area = document.createElement('textarea');
      area.value = text;
      document.body.appendChild(area);
      area.select();
      document.execCommand('copy');
      document.body.removeChild(area);
    }
    if (btn) { btn.textContent = 'Copied!'; setTimeout(() => { btn.textContent = original; }, 2000); }
  } catch (error) {
    if (btn) { btn.textContent = 'Copy failed'; setTimeout(() => { btn.textContent = original; }, 2000); }
  }
}

// Ask the daemon to shut down cleanly. The page itself can't respawn a process,
// so we guide the user: the tray/desktop launcher brings a fresh daemon back on
// the next open. For an immediate restart, the tray's "Repair / Restart" item
// does the full retire-and-relaunch.
async function repairDaemon() {
  const btn = els.sbRepair;
  const original = btn ? btn.textContent : '';
  if (!window.confirm('Restart the Vex background daemon? In-flight syncs will be interrupted.')) return;
  if (btn) { btn.textContent = 'Restarting…'; btn.disabled = true; }
  try {
    await api('/v1/daemon/shutdown', {method: 'POST', headers});
  } catch (_) { /* the daemon may drop the connection as it stops */ }
  setTimeout(() => {
    if (btn) { btn.textContent = original; btn.disabled = false; }
    els.sbConn.textContent = 'Daemon restarting — reopen Vex Atlas if it does not reconnect.';
  }, 1500);
}

async function checkUpdates() {
  try {
    updateInfo = await api('/v1/update/check', {headers});
  } catch (_) {
    updateInfo = null;
  }
  renderSystemBanner();
}

// Trigger the daemon's verified in-app installer download + launch. The daemon
// downloads the installer, checks its SHA-256 against the release manifest, and
// launches it; the installer then closes the app and relaunches the new build.
async function applyUpdate(button) {
  if (button) { button.disabled = true; button.textContent = 'Downloading…'; }
  try {
    const result = await api('/v1/update/apply', {method: 'POST', headers});
    if (result && result.launched) {
      if (button) button.textContent = 'Installing…';
      return;
    }
    // Apply not possible (unsupported platform / unverified asset): fall back to
    // opening the release page so the user can install manually.
    if (result && result.release_url) openExternalUrl(result.release_url);
    if (button) { button.disabled = false; button.textContent = 'Download & install'; }
  } catch (_) {
    if (updateInfo && updateInfo.release_url) openExternalUrl(updateInfo.release_url);
    if (button) { button.disabled = false; button.textContent = 'Download & install'; }
  }
}

// One banner drives both the "update available" and "engine/bridge schema
// mismatch" notices, with the safety-critical mismatch taking priority. Keeping
// it as a persistent strip (not transient toast) means a user who steps away
// still sees the state when they return.
function renderSystemBanner() {
  const banner = els.updateBanner;
  if (!banner) return;

  if (healthInfo && healthInfo.vex_bin && !healthInfo.vex_version) {
    banner.className = 'update-banner warn';
    banner.style.display = '';
    banner.innerHTML = '';
    const text = document.createElement('div');
    text.className = 'ub-text';
    text.innerHTML = '<strong>Vex engine not found.</strong>'
      + '<span class="ub-sub">IFC files can\'t be imported until the bundled engine is available. Reinstall Vex Atlas to restore it.</span>';
    banner.appendChild(text);
    return;
  }

  if (healthInfo && healthInfo.vex_schema_compatible === false) {
    banner.className = 'update-banner warn';
    banner.style.display = '';
    banner.innerHTML = '';
    const text = document.createElement('div');
    text.className = 'ub-text';
    text.innerHTML = '<strong>Engine / bridge version mismatch.</strong>'
      + '<span class="ub-sub">Visual diffs may be inaccurate until the components are realigned. Install matching versions.</span>';
    banner.appendChild(text);
    return;
  }

  if (updateInfo && updateInfo.update_available && updateInfo.latest_version) {
    const dismissed = localStorage.getItem('vexDismissedUpdate');
    if (dismissed === updateInfo.latest_version) { banner.style.display = 'none'; return; }
    banner.className = 'update-banner info';
    banner.style.display = '';
    banner.innerHTML = '';
    const text = document.createElement('div');
    text.className = 'ub-text';
    text.innerHTML = `<strong>Vex Atlas ${escapeHtml(updateInfo.latest_version)} is available.</strong>`
      + `<span class="ub-sub">You're on ${escapeHtml(updateInfo.current_version)}.</span>`;
    banner.appendChild(text);
    if (updateInfo.can_apply) {
      const apply = document.createElement('button');
      apply.className = 'ub-apply';
      apply.textContent = 'Download & install';
      apply.addEventListener('click', () => applyUpdate(apply));
      banner.appendChild(apply);
    }
    if (updateInfo.release_url) {
      const view = document.createElement('button');
      view.textContent = 'View release';
      view.addEventListener('click', () => openExternalUrl(updateInfo.release_url));
      banner.appendChild(view);
    }
    const dismiss = document.createElement('button');
    dismiss.className = 'ub-dismiss';
    dismiss.textContent = 'Dismiss';
    dismiss.addEventListener('click', () => {
      localStorage.setItem('vexDismissedUpdate', updateInfo.latest_version);
      banner.style.display = 'none';
    });
    banner.appendChild(dismiss);
    return;
  }

  banner.style.display = 'none';
}

function renderStatusBar(setup) {
  const pair = setup.pair_status || {};
  const paired = pair.status === 'paired';
  const watching = setup.watch && setup.watch.active_watchers > 0;
  els.sbDot.className = `sb-dot ${paired && watching ? 'ok' : (paired || watching ? 'warn' : '')}`;
  els.sbConn.textContent = paired ? 'Connected' : (pair.status === 'pending' ? 'Pairing…' : 'Not paired');
  const account = pair.account_name || pair.account_email || pair.device_label;
  if (paired && account) {
    els.sbAccountItem.style.display = '';
    els.sbAccount.textContent = account;
  } else {
    els.sbAccountItem.style.display = 'none';
  }
  if (setup.watch) {
    els.sbWatch.textContent = `${setup.watch.active_watchers}/${setup.watch.configured_projects} watching`;
  }
  if (selectedProject) {
    const project = (setup.watch && setup.watch.projects || []).find(p => p.project_id === selectedProject);
    if (project) {
      const pending = project.pending_push_count || 0;
      const pushText = pending > 0 ? ` · ${pending} ready to push` : '';
      els.sbActivity.textContent = `${escapeHtml(project.project_name || project.project_id)} · ${project.seen_import_count} imports${pushText}`;
    } else {
      els.sbActivity.textContent = '';
    }
  } else {
    els.sbActivity.textContent = '';
  }
  if (healthInfo) {
    const engine = healthInfo.vex_version ? ` · engine ${healthInfo.vex_version}` : '';
    els.sbVersions.textContent = `bridge ${healthInfo.version}${engine}`;
  }
}

function updatePairButton(status) {
  const kind = status && status.status;
  if (kind === 'paired') {
    const account = status.account_name || status.account_email || status.device_label;
    els.pairButton.textContent = account ? `Sign out (${account})` : 'Sign out';
    els.pairButton.disabled = false;
    els.pairButton.dataset.mode = 'signout';
  } else if (kind === 'pending') {
    els.pairButton.textContent = 'Check Pairing';
    els.pairButton.disabled = false;
    els.pairButton.dataset.mode = 'pair';
    ensurePairPolling();
  } else {
    els.pairButton.textContent = 'Pair Device';
    els.pairButton.disabled = false;
    els.pairButton.dataset.mode = 'pair';
  }
}

function pairText(status) {
  const kind = status && status.status;
  if (kind === 'paired') {
    const account = status.account_name || status.account_email;
    return account ? `Signed in as ${account}` : `Paired as ${status.device_label || 'this workstation'}`;
  }
  if (kind === 'pending') return `Pairing code ${status.code}`;
  return 'Not paired';
}

async function signOut() {
  if (!window.confirm('Sign out of this account? The device key will be removed and you will need to pair again.')) return;
  try {
    const status = await api('/v1/pair/forget', {method: 'POST', headers});
    if (lastSetup) lastSetup.pair_status = status;
    els.topStatus.textContent = 'Signed out';
    await refresh();
  } catch (error) {
    els.topStatus.textContent = error.message;
  }
}

async function startOrPollPairing() {
  const status = lastSetup && lastSetup.pair_status;
  if (els.pairButton.dataset.mode === 'signout') {
    await signOut();
    return;
  }
  if (status && status.status === 'pending') {
    await pollPairing();
    return;
  }
  const label = (lastSetup && lastSetup.default_device_label) || 'Vex Atlas Desktop';
  const response = await api('/v1/pair/start', {
    method: 'POST', headers: jsonHeaders, body: JSON.stringify({device_label: label, open_browser: true})
  });
  els.topStatus.textContent = `Pairing code ${response.code}`;
  ensurePairPolling();
}

function ensurePairPolling() {
  if (pairPollTimer) return;
  pairPollTimer = setInterval(pollPairing, 3000);
}

async function pollPairing() {
  try {
    const status = await api('/v1/pair/poll', {method: 'POST', headers});
    if (lastSetup) lastSetup.pair_status = status;
    updatePairButton(status);
    els.topStatus.textContent = `${pairText(status)} / ${lastSetup ? `${lastSetup.watch.active_watchers}/${lastSetup.watch.configured_projects} watching` : 'watching'}`;
    if (!status || status.status !== 'pending') {
      clearInterval(pairPollTimer);
      pairPollTimer = null;
      await refresh();
    }
  } catch (error) {
    els.topStatus.textContent = error.message;
  }
}

async function openPushPanel() {
  if (!selectedProject) return;
  els.pushPanel.classList.add('open');
  els.confirmPush.disabled = true;
  els.cloudProject.innerHTML = '<option value="">Loading projects…</option>';
  els.pushPreview.innerHTML = '<div class="row-meta">Loading preview…</div>';
  try {
    const [projects, changes] = await Promise.all([
      api('/v1/cloud/projects', {headers}),
      api(`/v1/projects/${encodeURIComponent(selectedProject)}/changes`, {headers})
    ]);
    cloudProjects = projects || [];
    const local = currentLocalProject();
    els.cloudProject.innerHTML = '<option value="">Select a cloud project…</option>';
    for (const project of cloudProjects) {
      const option = document.createElement('option');
      option.value = project.id;
      option.textContent = `${project.full_name}${project.has_commits ? ' (contains commits)' : ''}`;
      els.cloudProject.appendChild(option);
    }
    els.cloudProject.value = local && local.cloud_project_id || '';
    els.pushPanel.dataset.changes = JSON.stringify(changes || {});
    renderPushPreview();
  } catch (error) {
    els.pushPreview.innerHTML = `<div class="preview-warning">${escapeHtml(error.message)}</div>`;
  }
}

function closePushPanel() {
  els.pushPanel.classList.remove('open');
  delete els.pushPanel.dataset.changes;
}

function currentLocalProject() {
  return lastSetup && lastSetup.watch && lastSetup.watch.projects.find(project => project.project_id === selectedProject);
}

function renderPushPreview() {
  const local = currentLocalProject();
  const destination = cloudProjects.find(project => project.id === els.cloudProject.value);
  let changes = {};
  try { changes = JSON.parse(els.pushPanel.dataset.changes || '{}'); } catch (_) {}
  const latest = projectCommits[0];
  const counts = changes.visual_diff && changes.visual_diff.counts || {};
  const changedCount = ['added', 'removed', 'modified', 'moved', 'renamed']
    .reduce((total, kind) => total + (Number(counts[kind]) || 0), 0);
  els.confirmPush.disabled = !destination;
  els.pushPreview.innerHTML = `<div class="preview-grid">
    <div class="label">Destination</div><div class="value">${escapeHtml(destination ? destination.full_name : 'Select a project above')}</div>
    <div class="label">Local folder</div><div class="value">${escapeHtml(local ? local.local_path : '')}</div>
    <div class="label">Commits</div><div class="value">${local ? local.pending_push_count || 0 : 0} ready to push</div>
    <div class="label">Latest message</div><div class="value">${escapeHtml(latest ? latest.message : 'No commit found')}</div>
    <div class="label">Latest commit</div><div class="value">${escapeHtml(latest ? short(latest.commit) : '')}</div>
    <div class="label">Model changes</div><div class="value">${changedCount} elements (${counts.added || 0} added, ${counts.removed || 0} removed, ${counts.modified || 0} modified, ${counts.moved || 0} moved)</div>
  </div>${destination && destination.has_commits
    ? '<div class="preview-warning">This cloud project already contains commits. The push must be compatible with its existing history.</div>'
    : ''}`;
}

async function pushSelectedProject(event) {
  event.preventDefault();
  if (!selectedProject || !els.cloudProject.value) return;
  els.syncButton.disabled = true;
  els.confirmPush.disabled = true;
  const previousLabel = els.syncButton.textContent;
  els.syncButton.textContent = 'Pushing…';
  try {
    const destination = cloudProjects.find(project => project.id === els.cloudProject.value);
    const result = await api('/v1/repo/push', {
      method: 'POST', headers: jsonHeaders, body: JSON.stringify({
        project_id: selectedProject,
        cloud_project_id: els.cloudProject.value,
        branch: destination && destination.default_branch || 'main'
      })
    });
    els.topStatus.textContent = `Pushed ${short(result.commit_hash)}`;
    closePushPanel();
    // The push cleared the pending ledger; refresh so the badge/count update.
    await refresh();
  } catch (error) {
    els.topStatus.textContent = `Push failed: ${error.message}`;
    els.pushPreview.insertAdjacentHTML('beforeend', `<div class="preview-warning">${escapeHtml(error.message)}</div>`);
    els.syncButton.textContent = previousLabel;
    els.syncButton.disabled = false;
    els.confirmPush.disabled = false;
  }
}

// Enable the Push button only when the device is paired AND the selected
// project has locally-committed work that has not been pushed. Pushing is
// user-determined, so the button surfaces "Push (N)" with the pending count
// and falls back to a disabled "Push" / "All changes pushed" at zero.
function updatePushButton(setup, paired) {
  const projects = (setup.watch && setup.watch.projects) || [];
  const project = projects.find(p => p.project_id === selectedProject);
  const pending = project ? (project.pending_push_count || 0) : 0;
  els.syncButton.textContent = pending > 0 ? `Push (${pending})` : 'Push';
  els.syncButton.disabled = !selectedProject || !paired || pending === 0;
  els.syncButton.title = !paired
    ? 'Pair this device before pushing to the cloud'
    : (pending > 0
        ? `${pending} ${pending === 1 ? 'commit' : 'commits'} ready to push`
        : 'All changes pushed');
}

async function onAddIfcInput(event) {
  const input = event.target;
  const file = input.files && input.files[0];
  if (!file || !selectedProject) { input.value = ''; return; }
  if (!/\.ifc$/i.test(file.name)) {
    els.topStatus.textContent = 'Please choose a .ifc file.';
    input.value = '';
    return;
  }
  const project = selectedProject;
  const importStartedAt = Math.floor(Date.now() / 1000);
  els.addIfcButton.disabled = true;
  const previousLabel = els.addIfcButton.textContent;
  els.addIfcButton.textContent = 'Adding…';
  els.topStatus.textContent = `Preparing ${file.name} preview…`;
  try {
    const previewPromise = ifcViewer
      ? ifcViewer.loadLocalFile(file, project).then(() => null, error => error)
      : Promise.resolve(null);
    const uploadPromise = fetch(`/v1/projects/${encodeURIComponent(project)}/inbox`, {
      method: 'POST',
      headers: {'X-Vex-Bridge-Token': TOKEN, 'X-Vex-Filename': file.name, 'Content-Type': 'application/octet-stream'},
      body: file
    });
    const response = await uploadPromise;
    if (!response.ok) {
      let detail = `HTTP ${response.status}`;
      try { const body = await response.json(); if (body && (body.message || body.error)) detail = body.message || body.error; } catch (_) {}
      if (response.status === 404) {
        // The daemon no longer knows this project (its config changed underneath
        // the UI). Re-sync the project list and drop the stale selection so the
        // user re-selects or re-adds the inbox instead of retrying a dead id.
        selectedProject = null;
        await refresh();
        throw new Error('This project is no longer configured. Re-select or re-add the inbox, then try again.');
      }
      throw new Error(detail);
    }
    const result = await response.json();
    const previewError = await previewPromise;
    els.topStatus.textContent = previewError
      ? `Uploaded ${result.file_name}; semantic import is running. Preview failed: ${previewError.message}`
      : `Preview ready — importing ${result.file_name} in the background…`;
    trackImportCompletion(project, result.file_name, importStartedAt).catch(error => {
      if (selectedProject === project) els.topStatus.textContent = `Import tracking failed: ${error.message}`;
    });
  } catch (error) {
    els.topStatus.textContent = `Add IFC failed: ${error.message}`;
  } finally {
    input.value = '';
    els.addIfcButton.textContent = previousLabel;
    els.addIfcButton.disabled = !selectedProject;
  }
}

async function trackImportCompletion(projectId, fileName, startedAt) {
  const terminalKinds = new Set(['commit_created', 'duplicate_skipped', 'route_skipped', 'no_changes', 'error']);
  const deadline = Date.now() + (2 * 60 * 60 * 1000);
  while (Date.now() < deadline) {
    const activity = await api('/v1/activity/recent?limit=50', {headers});
    const event = (activity.events || []).find(item =>
      item.project_id === projectId
      && item.caught_at_unix >= startedAt
      && (!item.source_path || item.source_path.toLowerCase().endsWith(fileName.toLowerCase()))
      && terminalKinds.has(item.kind));
    if (event) {
      if (selectedProject === projectId) {
        if (event.kind === 'commit_created') {
          els.topStatus.textContent = `Imported ${fileName} — loading semantic changes…`;
          await reloadSelectedProject();
          await refresh({reloadSelected: false});
          els.topStatus.textContent = `Ready — ${fileName} committed`;
        } else if (event.kind === 'error') {
          els.topStatus.textContent = `Import failed: ${event.detail || event.message}`;
        } else {
          els.topStatus.textContent = event.message || `Import finished: ${event.kind}`;
        }
      }
      return;
    }
    await new Promise(resolve => setTimeout(resolve, 2000));
  }
  if (selectedProject === projectId) {
    els.topStatus.textContent = `Import is still running. The local preview remains available.`;
  }
}

function renderProjects(projects) {
  els.projectCount.textContent = String(projects.length);
  els.projects.innerHTML = projects.length ? '' : '<div class="empty">No inboxes configured.</div>';
  for (const project of projects) {
    const wrapper = document.createElement('div');
    wrapper.className = 'project-row';
    const button = document.createElement('button');
    button.className = `row ${project.project_id === selectedProject ? 'active' : ''}`;
    const pending = project.pending_push_count || 0;
    const pushLine = pending > 0
      ? `<div class="row-meta">${pending} ready to push</div>`
      : '';
    button.innerHTML = `<div class="row-title">${escapeHtml(project.project_name || project.project_id)}</div>
      <div class="row-meta">${project.active ? 'Watching' : 'Inactive'} / ${escapeHtml(project.local_path)}</div>
      <div class="row-meta">${project.seen_import_count} imports</div>${pushLine}`;
    button.addEventListener('click', () => selectProject(project.project_id));
    const remove = document.createElement('button');
    remove.className = 'icon-button danger';
    remove.type = 'button';
    remove.title = 'Delete project';
    remove.setAttribute('aria-label', `Delete ${project.project_name || project.project_id}`);
    remove.textContent = '×';
    remove.addEventListener('click', event => {
      event.stopPropagation();
      openDeletePanel(project);
    });
    wrapper.appendChild(button);
    wrapper.appendChild(remove);
    els.projects.appendChild(wrapper);
  }
}

async function selectProject(projectId) {
  selectedProject = projectId;
  selectedCommit = null;
  projectCommits = [];
  els.addIfcButton.disabled = false;
  await reloadSelectedProject();
  await refresh({reloadSelected: false});
}

async function reloadSelectedProject() {
  if (!selectedProject) return;
  const previousCommit = selectedCommit;
  await loadHistory(selectedProject);
  const requested = !previousCommit ? projectCommits.find(commit => requestedCommit && commit.commit.startsWith(requestedCommit)) : null;
  const target = requested || projectCommits[0];
  await selectCommit(target ? target.commit : null);
}

async function loadHistory(projectId) {
  try {
    const history = await api(`/v1/projects/${encodeURIComponent(projectId)}/history`, {headers});
    projectCommits = history.commits || [];
    els.historyMeta.textContent = String(history.commits.length);
    els.history.innerHTML = history.commits.length ? '' : '<div class="empty">No commits yet.</div>';
    for (const commit of history.commits) {
      const row = document.createElement('button');
      row.className = `row ${commit.commit === selectedCommit ? 'active' : ''}`;
      row.dataset.commit = commit.commit;
      row.innerHTML = `<div class="row-title">${escapeHtml(commit.message || commit.commit.slice(0, 12))}</div>
        <div class="row-meta">${formatTime(commit.timestamp)} / ${escapeHtml(commit.author || 'unknown')}</div>
        <div class="row-meta">${short(parentFor(commit))} -> ${commit.commit.slice(0, 12)}</div>`;
      row.addEventListener('click', () => selectCommit(commit.commit));
      els.history.appendChild(row);
    }
  } catch (error) {
    projectCommits = [];
    els.history.innerHTML = `<div class="empty">${escapeHtml(error.message)}</div>`;
  }
}

async function selectCommit(commitHash) {
  selectedCommit = commitHash;
  renderHistorySelection();
  if (!selectedProject) return;
  const commit = projectCommits.find(item => item.commit === commitHash);
  await loadChanges(selectedProject, parentFor(commit), commitHash);
}

function renderHistorySelection() {
  for (const row of els.history.querySelectorAll('.row')) row.classList.remove('active');
  for (const row of els.history.querySelectorAll('[data-commit]')) {
    if (row.dataset.commit === selectedCommit) row.classList.add('active');
  }
}

async function loadChanges(projectId, fromCommit, toCommit) {
  try {
    const params = new URLSearchParams();
    if (fromCommit) params.set('from', fromCommit);
    if (toCommit) params.set('to', toCommit);
    const suffix = params.toString() ? `?${params}` : '';
    const changes = await api(`/v1/projects/${encodeURIComponent(projectId)}/changes${suffix}`, {headers});
    latestChanges = changes;
    renderChanges(changes);
  } catch (error) {
    latestChanges = null;
    els.changeTitle.textContent = error.message;
    els.changeTime.textContent = '';
    els.countBadges.innerHTML = '';
    els.changeRows.innerHTML = '';
    drawChanges(null);
  }
}

function renderChanges(changes) {
  if (!changes) {
    els.changeTitle.textContent = 'No project selected';
    els.changeTime.textContent = '';
    els.countBadges.innerHTML = '';
    els.changeRows.innerHTML = '';
    if (ifcViewer) ifcViewer.clear('Select a project with an imported IFC.');
    return;
  }
  const diff = changes.visual_diff || {};
  const summary = diff.summary || diff.status || 'No previous version to compare';
  els.changeTitle.textContent = summary;
  els.changeTime.textContent = `Caught ${formatTime(changes.caught_at_unix)} / comparing ${short(changes.previous_commit)} -> ${short(changes.latest_commit)}`;
  renderBadges(diff.counts || {});
  const changed = (diff.elements || []).filter(element => element.kind !== 'unchanged');
  renderRows(changed);
  drawChanges(changes);
}

function renderBadges(counts) {
  const kinds = ['added', 'removed', 'modified', 'moved', 'renamed', 'unchanged'];
  els.countBadges.innerHTML = '';
  for (const kind of kinds) {
    const badge = document.createElement('span');
    badge.className = `badge ${kind}`;
    badge.textContent = `${kind} ${counts[kind] || 0}`;
    els.countBadges.appendChild(badge);
  }
  if (counts.internal > 0) {
    const badge = document.createElement('span');
    badge.className = 'badge internal';
    badge.title = 'Anonymous helper entities (placements, geometry carriers) that changed alongside the elements above.';
    badge.textContent = `+${counts.internal} internal`;
    els.countBadges.appendChild(badge);
  }
}

function layerChip(element) {
  const layer = element.layer;
  if (!layer || element.kind === 'added' || element.kind === 'removed') return '';
  if (layer === 'shape') {
    return ' <span class="layer-chip shape" title="The element\'s geometry changed">geometry</span>';
  }
  if (layer === 'relationship') {
    return ' <span class="layer-chip" title="Only the element\'s relationships changed">links</span>';
  }
  if (layer === 'property') {
    return ' <span class="layer-chip property" title="Attribute / property values changed">props</span>';
  }
  return '';
}

function renderRows(elements) {
  els.changeRows.innerHTML = elements.length ? '' : '<tr><td colspan="3" class="row-meta">No element-level changes.</td></tr>';
  for (const element of elements.slice(0, 150)) {
    const deltas = Array.isArray(element.deltas) ? element.deltas : [];
    const tr = document.createElement('tr');
    if (deltas.length) tr.className = 'change-row expandable';
    const caret = deltas.length ? '<span class="caret">\u25B8</span>' : '';
    tr.innerHTML = `<td class="kind ${element.kind}">${caret}${escapeHtml(element.kind)}${layerChip(element)}</td>
      <td>${escapeHtml(elementType(element))}</td>
      <td>${escapeHtml(element.hint || idLabel(element.id) || '')}</td>`;
    els.changeRows.appendChild(tr);
    if (deltas.length) {
      const detail = document.createElement('tr');
      detail.className = 'change-detail';
      detail.style.display = 'none';
      detail.innerHTML = `<td colspan="3">${renderDeltaTable(deltas)}</td>`;
      els.changeRows.appendChild(detail);
      tr.addEventListener('click', () => {
        const open = detail.style.display !== 'none';
        detail.style.display = open ? 'none' : '';
        tr.classList.toggle('open', !open);
      });
    }
  }
}

// Render the per-element attribute changes (the engine's `deltas`) as a
// compact before -> after table. Positional IFC slots are mapped to the
// IfcRoot-stable attribute names where known.
function renderDeltaTable(deltas) {
  const rows = deltas.slice(0, 80).map(delta => {
    const before = escapeHtml(formatSerValue(delta.before));
    const after = escapeHtml(formatSerValue(delta.after));
    return `<tr><td class="dk">${escapeHtml(attrLabel(delta.key))}</td>`
      + `<td class="dv before">${before}</td>`
      + `<td class="dv arrow">\u2192</td>`
      + `<td class="dv after">${after}</td></tr>`;
  }).join('');
  const more = deltas.length > 80 ? `<tr><td colspan="4" class="row-meta">+${deltas.length - 80} more</td></tr>` : '';
  return `<table class="delta-table"><tbody>${rows}${more}</tbody></table>`;
}

// IfcRoot attribute slots are stable across every rooted IFC entity
// (GlobalId, OwnerHistory, Name, Description). Other positional slots vary by
// type, so we show them as "field N" rather than risk a wrong label.
function attrLabel(key) {
  const known = {_0: 'GlobalId', _1: 'OwnerHistory', _2: 'Name', _3: 'Description'};
  if (known[key]) return known[key];
  const match = /^_(\d+)$/.exec(key || '');
  return match ? `field ${match[1]}` : (key || '');
}

// Format one engine SerValue (externally-tagged enum) for display. Handles
// the `Option<SerValue>` wrapper (null), the bare `"Null"` unit variant, and
// every value variant the engine can emit.
function formatSerValue(value) {
  if (value === null || value === undefined) return '\u2014';
  if (typeof value === 'string') return value === 'Null' ? '\u2014' : value;
  if (typeof value !== 'object') return String(value);
  if ('Text' in value) return value.Text;
  if ('Enum' in value) return `.${value.Enum}.`;
  if ('Int' in value) return String(value.Int);
  if ('Real' in value) return String(value.Real);
  if ('Bool' in value) return value.Bool ? 'true' : 'false';
  if ('List' in value) return `[${(value.List || []).map(formatSerValue).join(', ')}]`;
  if ('Typed' in value && value.Typed) return `${value.Typed.name}(${formatSerValue(value.Typed.inner)})`;
  return JSON.stringify(value);
}

function drawChanges(changes) {
  if (ifcViewer) ifcViewer.load(changes, currentViewMode);
}

class RealIfcViewer {
  constructor({planCanvas, modelCanvas, planStatus, modelStatus, planMeta, modelMeta}) {
    this.planCanvas = planCanvas;
    this.modelCanvas = modelCanvas;
    this.planStatus = planStatus;
    this.modelStatus = modelStatus;
    this.planMeta = planMeta;
    this.modelMeta = modelMeta;
    this.modelScene = this.makeScene();
    // The 2D plan renders the SAME scene from a top-down camera. Sharing the
    // scene avoids cloning the web-ifc model (clone(true) frequently yields an
    // empty object, which is why the 2D pane rendered nothing).
    this.planScene = this.modelScene;
    this.planCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 1000000);
    this.modelPersp = new THREE.PerspectiveCamera(45, 1, 0.1, 1000000);
    this.modelOrtho = new THREE.OrthographicCamera(-1, 1, 1, -1, -1000000, 1000000);
    this.modelPersp.up.set(0, 0, 1);
    this.modelOrtho.up.set(0, 0, 1);
    this.modelCamera = this.modelPersp;
    this.projection = 'perspective';
    this.planCamera.up.set(0, 1, 0);
    this.planRenderer = this.makeRenderer(planCanvas);
    // The 2D pane renders the shared scene through a horizontal cut plane so it
    // reads as a true floor plan (everything above the cut height is removed)
    // rather than a top-down roof view. Toggle to 'top' to see the full model.
    this.planCutMode = 'plan';
    // Two horizontal clip planes isolate a single storey in the 2D pane: the
    // upper keeps geometry at/below the section height (~1.2 m above the floor),
    // the lower keeps geometry at/above the floor so storeys above don't stack
    // into the plan.
    this.planClipPlane = new THREE.Plane(new THREE.Vector3(0, 0, -1), 0);
    this.planClipPlaneLower = new THREE.Plane(new THREE.Vector3(0, 0, 1), 0);
    this.planRenderer.clippingPlanes = [this.planClipPlane, this.planClipPlaneLower];
    this.storeys = [];
    this.planLevelIndex = 0;
    this.planCutZ = null;
    this.modelRenderer = this.makeRenderer(modelCanvas);
    this.modelRenderer.localClippingEnabled = true;
    this.controls = new OrbitControls(this.modelCamera, modelCanvas);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    // ArchiCAD-style orbit: left-drag rotates the model around the target, wheel
    // zooms, right/middle drag pans. Set explicitly so a later tweak can't
    // silently disable rotation, and allow the full polar range so you can orbit
    // under the model to inspect its underside.
    this.controls.enableRotate = true;
    this.controls.enableZoom = true;
    this.controls.enablePan = true;
    this.controls.rotateSpeed = 0.9;
    this.controls.zoomSpeed = 1.1;
    this.controls.panSpeed = 0.9;
    this.controls.screenSpacePanning = true;
    this.controls.minPolarAngle = 0;
    this.controls.maxPolarAngle = Math.PI;
    this.controls.mouseButtons = {
      LEFT: THREE.MOUSE.ROTATE,
      MIDDLE: THREE.MOUSE.DOLLY,
      RIGHT: THREE.MOUSE.PAN
    };
    this.controls.touches = {
      ONE: THREE.TOUCH.ROTATE,
      TWO: THREE.TOUCH.DOLLY_PAN
    };
    if ('zoomToCursor' in this.controls) this.controls.zoomToCursor = true;
    this.helpers = new THREE.Group();
    this.modelScene.add(this.helpers);
    this.raycaster = new THREE.Raycaster();
    this.pointer = new THREE.Vector2();
    this.downAt = null;
    this.selectionSubset = null;
    this.selectedId = null;
    this.sectionActive = false;
    this.sectionPlane = new THREE.Plane(new THREE.Vector3(0, 0, -1), 0);
    // web-ifc loads geometry Y-up; this viewer is authored Z-up. Rotate every
    // loaded model (and its subsets) by this angle to map IFC up (Y) onto +Z.
    this.upAxisFix = Math.PI / 2;
    // 3D floor isolation (full structure by default). When a level is selected
    // these two planes clip the 3D model to one storey's floor-to-ceiling band.
    this.modelLevelLower = new THREE.Plane(new THREE.Vector3(0, 0, 1), 0);
    this.modelLevelUpper = new THREE.Plane(new THREE.Vector3(0, 0, -1), 0);
    this.modelLevelIndex = null;
    this.modelBox = null;
    this.gizmo = this.makeGizmo();
    this.currentKey = '';
    this.loadToken = 0;
    this.ifcAbortController = null;
    this.artifactAbortController = null;
    this.ifcLoader = null;
    this.gltfLoader = new GLTFLoader();
    this.model = null;
    this.modelKind = null;
    this.artifactManifest = null;
    this.artifactSemanticIndex = null;
    this.artifactTilesLoaded = 0;
    this.highlightObjects = [];
    this.removedObjects = [];
    this.planPan = new THREE.Vector2(0, 0);
    this.planZoom = 1;
    this.planDrag = null;
    this.modelCanvas.addEventListener('pointerdown', event => { this.downAt = {x: event.clientX, y: event.clientY}; });
    this.modelCanvas.addEventListener('pointerup', event => this.handlePointerUp(event));
    // Hide the orbit hint the first time the user actually drags the model.
    this.modelCanvas.addEventListener('pointerdown', () => this.dismissOrbitHint(true));
    // The navigation cube doubles as a quick view switcher: click to step
    // Iso -> Top -> Front -> Right -> Iso, like the corner gizmo in ArchiCAD.
    this.viewCycle = ['iso', 'top', 'front', 'right'];
    this.viewCycleIndex = 0;
    if (this.gizmo && this.gizmo.renderer && this.gizmo.renderer.domElement) {
      const gizmoEl = this.gizmo.renderer.domElement;
      gizmoEl.style.cursor = 'pointer';
      gizmoEl.title = 'Click to step through standard views';
      gizmoEl.addEventListener('click', () => this.cycleView());
    }
    this.planCanvas.addEventListener('pointerdown', event => this.beginPlanPan(event));
    this.planCanvas.addEventListener('pointermove', event => this.movePlanPan(event));
    this.planCanvas.addEventListener('pointerup', event => this.endPlanPan(event));
    this.planCanvas.addEventListener('pointerleave', event => this.endPlanPan(event));
    this.planCanvas.addEventListener('wheel', event => this.zoomPlan(event), {passive: false});
    this.resize();
    if (window.ResizeObserver) {
      this.resizeObserver = new ResizeObserver(() => this.resize());
      if (this.modelCanvas.parentElement) this.resizeObserver.observe(this.modelCanvas.parentElement);
      if (this.planCanvas.parentElement) this.resizeObserver.observe(this.planCanvas.parentElement);
    }
    this.animate();
    this.clear('Select a project with an imported IFC.');
  }

  makeGizmo() {
    const canvas = document.getElementById('gizmoCanvas');
    if (!canvas) return null;
    const renderer = new THREE.WebGLRenderer({canvas, antialias: true, alpha: true});
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.setSize(86, 86, false);
    renderer.setClearColor(0x000000, 0);
    const scene = new THREE.Scene();
    const axes = new THREE.AxesHelper(1);
    axes.material.depthTest = false;
    scene.add(axes);
    const cam = new THREE.OrthographicCamera(-1.6, 1.6, 1.6, -1.6, 0.1, 100);
    cam.up.set(0, 0, 1);
    return {renderer, scene, cam};
  }

  rebuildHelpers(box) {
    while (this.helpers.children.length) {
      const child = this.helpers.children.pop();
      if (child.geometry) child.geometry.dispose();
      if (child.material) child.material.dispose();
    }
    const size = box.getSize(new THREE.Vector3());
    const center = box.getCenter(new THREE.Vector3());
    const span = Math.max(size.x, size.y, 1);
    const divisions = 20;
    const grid = new THREE.GridHelper(span * 1.6, divisions, 0x3a3f43, 0x24282b);
    grid.rotation.x = Math.PI / 2;
    grid.position.set(center.x, center.y, box.min.z);
    this.helpers.add(grid);
    const axes = new THREE.AxesHelper(span * 0.35);
    axes.position.set(box.min.x, box.min.y, box.min.z);
    this.helpers.add(axes);
  }

  handlePointerUp(event) {
    if (!this.downAt || !this.model || !['ifc', 'artifact'].includes(this.modelKind)) { this.downAt = null; return; }
    const moved = Math.hypot(event.clientX - this.downAt.x, event.clientY - this.downAt.y);
    this.downAt = null;
    if (moved > 5) return;
    const rect = this.modelCanvas.getBoundingClientRect();
    this.pointer.set(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      -((event.clientY - rect.top) / rect.height) * 2 + 1
    );
    this.raycaster.setFromCamera(this.pointer, this.modelCamera);
    const hits = this.raycaster.intersectObject(this.model, true);
    const hit = hits.find(item => item.object && item.object.geometry && Number.isFinite(item.faceIndex));
    if (!hit) { this.clearSelection(); return; }
    if (this.modelKind === 'artifact') {
      this.selectArtifactElement(hit);
      return;
    }
    try {
      const expressId = this.model.getExpressId(hit.object.geometry, hit.faceIndex);
      if (Number.isFinite(expressId)) this.selectElement(expressId);
    } catch (_) { /* ignore picking errors */ }
  }

  selectArtifactElement(hit) {
    const index = this.artifactSemanticIndex;
    if (!index || !Array.isArray(index.entries)) return;
    const entry = index.entries.find(candidate => (candidate.triangle_ranges || []).some(range =>
      Number.isFinite(range.first_triangle) && Number.isFinite(range.triangle_count)
      && hit.faceIndex >= range.first_triangle
      && hit.faceIndex < range.first_triangle + range.triangle_count
    ));
    if (!entry) return;

    this.clearSelection();
    this.selectedId = Number.isFinite(entry.express_id) ? entry.express_id : null;
    const range = (entry.triangle_ranges || []).find(candidate =>
      hit.faceIndex >= candidate.first_triangle
      && hit.faceIndex < candidate.first_triangle + candidate.triangle_count
    );
    const source = hit.object.geometry;
    const sourceIndex = source && source.getIndex && source.getIndex();
    if (range && sourceIndex) {
      const start = range.first_triangle * 3;
      const count = range.triangle_count * 3;
      const selectionGeometry = new THREE.BufferGeometry();
      const sourcePositions = source.getAttribute('position');
      const positions = new Float32Array(count * 3);
      // Build a compact standalone overlay so disposing a selection never
      // disposes the GPU buffers shared by its source tile.
      for (let index = 0; index < count; index += 1) {
        const vertex = sourceIndex.array[start + index];
        positions[index * 3] = sourcePositions.getX(vertex);
        positions[index * 3 + 1] = sourcePositions.getY(vertex);
        positions[index * 3 + 2] = sourcePositions.getZ(vertex);
      }
      selectionGeometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      const material = new THREE.MeshBasicMaterial({
        color: 0x4b8fe3,
        transparent: true,
        opacity: 0.7,
        depthTest: false,
        side: THREE.DoubleSide,
      });
      this.selectionSubset = new THREE.Mesh(selectionGeometry, material);
      this.selectionSubset.userData.vexArtifactSelection = true;
      this.selectionSubset.renderOrder = 1;
      this.modelScene.add(this.selectionSubset);
      this.orientModel(this.selectionSubset);
    }
    this.showProperties(
      entry.global_id ? {GlobalId: {value: entry.global_id}} : null,
      Number.isFinite(entry.express_id) ? entry.express_id : 'artifact',
    );
  }

  async selectElement(expressId) {
    if (!this.model || this.modelKind !== 'ifc') return;
    this.selectedId = expressId;
    if (this.selectionSubset && this.selectionSubset.parent) this.selectionSubset.parent.remove(this.selectionSubset);
    const material = new THREE.MeshLambertMaterial({color: 0x4b8fe3, transparent: true, opacity: 0.85, depthTest: false, side: THREE.DoubleSide});
    this.selectionSubset = this.model.createSubset({ids: [expressId], material, scene: this.modelScene, removePrevious: true, customID: 'vex-selection'});
    this.orientModel(this.selectionSubset);
    try {
      const props = await this.model.getItemProperties(expressId, true);
      this.showProperties(props, expressId);
    } catch (_) {
      this.showProperties(null, expressId);
    }
  }

  showProperties(props, expressId) {
    const panel = document.getElementById('propsPanel');
    const title = document.getElementById('propsTitle');
    const body = document.getElementById('propsBody');
    if (!panel) return;
    const name = props && valueOf(props.Name);
    title.textContent = name || `Element ${expressId}`;
    const rows = [['Express ID', String(expressId)]];
    if (props) {
      const tag = valueOf(props.Tag); if (tag) rows.push(['Tag', tag]);
      const gid = valueOf(props.GlobalId); if (gid) rows.push(['GlobalId', gid]);
      const desc = valueOf(props.Description); if (desc) rows.push(['Description', desc]);
      const objType = valueOf(props.ObjectType); if (objType) rows.push(['ObjectType', objType]);
      if (Number.isFinite(props.type)) rows.push(['IFC Type', String(props.type)]);
    }
    body.innerHTML = rows.map(([k, v]) =>
      `<div class="prop"><span class="k">${escapeHtml(k)}</span><span class="v">${escapeHtml(v)}</span></div>`).join('');
    panel.classList.add('open');
  }

  clearSelection() {
    this.selectedId = null;
    if (this.selectionSubset && this.selectionSubset.parent) this.selectionSubset.parent.remove(this.selectionSubset);
    if (this.selectionSubset && this.selectionSubset.userData.vexArtifactSelection) {
      this.selectionSubset.geometry.dispose();
      this.selectionSubset.material.dispose();
    }
    this.selectionSubset = null;
    const panel = document.getElementById('propsPanel');
    if (panel) panel.classList.remove('open');
  }

  fit() {
    if (this.model) this.fitToModel(this.model);
  }

  setView(name) {
    if (!this.modelBox) return;
    const center = this.modelBox.getCenter(new THREE.Vector3());
    const size = this.modelBox.getSize(new THREE.Vector3());
    const radius = Math.max(size.x, size.y, size.z, 1);
    const d = radius * 1.8;
    const dirs = {
      iso: new THREE.Vector3(1, -1, 0.8),
      top: new THREE.Vector3(0, 0, 1),
      front: new THREE.Vector3(0, -1, 0),
      right: new THREE.Vector3(1, 0, 0)
    };
    const dir = (dirs[name] || dirs.iso).clone().normalize();
    this.modelCamera.position.copy(center.clone().add(dir.multiplyScalar(d)));
    this.controls.target.copy(center);
    this.modelCamera.updateProjectionMatrix();
    this.controls.update();
  }

  cycleView() {
    if (!this.modelBox) return;
    this.viewCycleIndex = (this.viewCycleIndex + 1) % this.viewCycle.length;
    this.setView(this.viewCycle[this.viewCycleIndex]);
  }

  showOrbitHint() {
    const hint = document.getElementById('orbitHint');
    if (!hint) return;
    try { if (localStorage.getItem('vexOrbitHintSeen')) return; } catch (_) {}
    hint.classList.add('show');
    clearTimeout(this.orbitHintTimer);
    this.orbitHintTimer = setTimeout(() => this.dismissOrbitHint(false), 5000);
  }

  dismissOrbitHint(persist) {
    const hint = document.getElementById('orbitHint');
    if (hint) hint.classList.remove('show');
    clearTimeout(this.orbitHintTimer);
    if (persist) {
      try { localStorage.setItem('vexOrbitHintSeen', '1'); } catch (_) {}
    }
  }

  toggleProjection() {
    const next = this.projection === 'perspective' ? this.modelOrtho : this.modelPersp;
    next.position.copy(this.modelCamera.position);
    next.up.copy(this.modelCamera.up);
    this.projection = this.projection === 'perspective' ? 'orthographic' : 'perspective';
    this.modelCamera = next;
    this.controls.object = next;
    const btn = document.getElementById('projBtn');
    if (btn) { btn.textContent = this.projection === 'perspective' ? 'Persp' : 'Ortho'; btn.classList.toggle('active', this.projection === 'orthographic'); }
    this.resizeRenderer(this.modelRenderer, this.modelCanvas, this.modelCamera);
    if (this.modelBox) {
      this.controls.target.copy(this.modelBox.getCenter(new THREE.Vector3()));
    }
    this.modelCamera.lookAt(this.controls.target);
    this.modelCamera.updateProjectionMatrix();
    this.controls.update();
  }

  toggleSection(force) {
    this.sectionActive = (typeof force === 'boolean') ? force : !this.sectionActive;
    const row = document.getElementById('sectionRow');
    const btn = document.getElementById('sectionBtn');
    if (row) row.classList.toggle('open', this.sectionActive);
    if (btn) btn.classList.toggle('active', this.sectionActive);
    if (this.sectionActive) {
      // Section and 3D floor isolation share the model clip planes; turning on
      // Section drops any active floor isolation.
      this.modelLevelIndex = null;
      const sel = document.getElementById('modelLevel');
      if (sel) sel.value = 'full';
    }
    this.modelRenderer.clippingPlanes = this.sectionActive ? [this.sectionPlane] : [];
    if (this.sectionActive) {
      const slider = document.getElementById('sectionSlider');
      this.setSection(slider ? Number(slider.value) : 100);
    }
  }

  setSection(percent) {
    if (!this.modelBox) return;
    const minZ = this.modelBox.min.z;
    const maxZ = this.modelBox.max.z;
    const z = minZ + (maxZ - minZ) * (percent / 100);
    // Plane normal points -Z: keep geometry below the cut height visible.
    this.sectionPlane.set(new THREE.Vector3(0, 0, -1), z);
  }

  // Distance above a storey level where a floor plan is conventionally cut
  // (~1.2 m). web-ifc normalises to model units, so detect mm vs m by extent.
  planCutOffset() {
    if (!this.modelBox) return 1.2;
    const height = this.modelBox.max.z - this.modelBox.min.z;
    return height > 400 ? 1200 : 1.2;
  }

  async extractStoreys(model) {
    this.storeys = [];
    try {
      const structure = await model.getSpatialStructure();
      const found = [];
      const visit = node => {
        if (!node) return;
        const type = String(node.type || '').toUpperCase();
        if (type.indexOf('STOREY') !== -1 && Number.isFinite(node.expressID)) {
          found.push(node.expressID);
        }
        for (const child of node.children || []) visit(child);
      };
      visit(structure);
      for (const expressID of found) {
        let name = `Level ${this.storeys.length + 1}`;
        let elevation = null;
        try {
          const props = await model.getItemProperties(expressID, false);
          name = valueOf(props && props.Name) || valueOf(props && props.LongName) || name;
          const raw = props && props.Elevation;
          const value = raw && typeof raw === 'object' ? raw.value : raw;
          if (Number.isFinite(value)) elevation = Number(value);
        } catch (_) { /* keep defaults */ }
        this.storeys.push({expressID, name, elevation});
      }
      // Sort by elevation so the level list reads ground-up.
      this.storeys.sort((a, b) => (a.elevation ?? 0) - (b.elevation ?? 0));
    } catch (_) {
      this.storeys = [];
    }
    this.planLevelIndex = 0;
    if (typeof this.onStoreys === 'function') this.onStoreys(this.storeys);
  }

  setPlanCutMode(mode) {
    this.planCutMode = mode === 'top' ? 'top' : 'plan';
    this.applyPlanCut();
  }

  setPlanLevel(index) {
    this.planLevelIndex = index;
    this.applyPlanCut();
  }

  planCutHeight() {
    if (!this.modelBox) return 0;
    const minZ = this.modelBox.min.z;
    const maxZ = this.modelBox.max.z;
    const storey = this.storeys[this.planLevelIndex];
    if (storey && Number.isFinite(storey.elevation)) {
      const z = storey.elevation + this.planCutOffset();
      // Clamp a hair below the roof so a single-storey plan still shows walls.
      return Math.min(z, maxZ - (maxZ - minZ) * 0.02);
    }
    // No storey metadata: cut low through the model so it reads as a plan.
    return minZ + (maxZ - minZ) * 0.25;
  }

  // Lower bound of the current storey: just below the floor slab so the plan
  // isolates one level instead of stacking every storey above the cut.
  planFloorHeight() {
    if (!this.modelBox) return 0;
    const minZ = this.modelBox.min.z;
    const storey = this.storeys[this.planLevelIndex];
    if (storey && Number.isFinite(storey.elevation)) {
      return storey.elevation - this.planCutOffset() * 0.4;
    }
    return minZ;
  }

  applyPlanCut() {
    if (!this.modelBox) return;
    if (this.planCutMode === 'top') {
      this.planRenderer.clippingPlanes = [];
      this.planCutZ = null;
      return;
    }
    this.planCutZ = this.planCutHeight();
    const floorZ = this.planFloorHeight();
    // Upper plane keeps z <= cut height; lower plane keeps z >= floor height.
    this.planClipPlane.set(new THREE.Vector3(0, 0, -1), this.planCutZ);
    this.planClipPlaneLower.set(new THREE.Vector3(0, 0, 1), -floorZ);
    this.planRenderer.clippingPlanes = [this.planClipPlane, this.planClipPlaneLower];
  }

  setModelLevel(index) {
    this.modelLevelIndex = (index === null || index === undefined) ? null : index;
    // Isolating a floor and the free section plane both drive the 3D renderer's
    // clipping, so they are mutually exclusive.
    if (this.modelLevelIndex !== null && this.sectionActive) this.toggleSection(false);
    this.applyModelLevel();
  }

  applyModelLevel() {
    if (!this.modelBox) return;
    if (this.modelLevelIndex === null) {
      this.modelRenderer.clippingPlanes = this.sectionActive ? [this.sectionPlane] : [];
      return;
    }
    const minZ = this.modelBox.min.z;
    const maxZ = this.modelBox.max.z;
    const storey = this.storeys[this.modelLevelIndex];
    const lowerZ = (storey && Number.isFinite(storey.elevation))
      ? storey.elevation - this.planCutOffset() * 0.4
      : minZ;
    const next = this.storeys[this.modelLevelIndex + 1];
    const upperZ = (next && Number.isFinite(next.elevation)) ? next.elevation : maxZ;
    this.modelLevelLower.set(new THREE.Vector3(0, 0, 1), -lowerZ);
    this.modelLevelUpper.set(new THREE.Vector3(0, 0, -1), upperZ);
    this.modelRenderer.clippingPlanes = [this.modelLevelLower, this.modelLevelUpper];
  }

  animate() {
    requestAnimationFrame(() => this.animate());
    this.controls.update();
    if (this.modelCanvas.offsetParent !== null) {
      this.modelRenderer.render(this.modelScene, this.modelCamera);
      this.renderGizmo();
    }
    if (this.planCanvas.offsetParent !== null) {
      this.planRenderer.render(this.planScene, this.planCamera);
    }
  }

  renderGizmo() {
    if (!this.gizmo) return;
    const offset = this.modelCamera.position.clone().sub(this.controls.target);
    if (offset.lengthSq() < 1e-9) return;
    offset.normalize().multiplyScalar(3.2);
    this.gizmo.cam.position.copy(offset);
    this.gizmo.cam.up.copy(this.modelCamera.up);
    this.gizmo.cam.lookAt(0, 0, 0);
    this.gizmo.renderer.render(this.gizmo.scene, this.gizmo.cam);
  }

  makeRenderer(canvas) {
    const renderer = new THREE.WebGLRenderer({canvas, antialias: true, alpha: false});
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.setClearColor(0x111313, 1);
    return renderer;
  }

  makeScene() {
    const scene = new THREE.Scene();
    scene.add(new THREE.HemisphereLight(0xffffff, 0x303437, 0.85));
    const sun = new THREE.DirectionalLight(0xffffff, 0.8);
    sun.position.set(40, -35, 70);
    scene.add(sun);
    return scene;
  }

  clear(message = '') {
    this.abortPendingLoads();
    ++this.loadToken;
    this.clearSceneModels();
    this.currentKey = '';
    this.planStatus.textContent = message;
    this.modelStatus.textContent = message;
    this.planMeta.textContent = '';
    this.modelMeta.textContent = '';
  }

  async load(changes, mode) {
    const projectId = changes && changes.project_id;
    const latestCommit = changes && changes.latest_commit;
    if (!projectId || !latestCommit) {
      this.clear('Drop an IFC into this inbox to render the model.');
      return;
    }
    const key = `${projectId}:${latestCommit}`;
    const requiresIfcDiff = mode === 'changes' && this.hasVisualChanges(changes);
    if (this.currentKey === key && !(requiresIfcDiff && this.modelKind === 'artifact')) {
      const token = this.loadToken;
      await this.applyDiff(changes, mode, token);
      if (token !== this.loadToken) return;
      this.setModelSourceMeta(mode);
      return;
    }
    this.abortPendingLoads();
    const token = ++this.loadToken;
    try {
      this.clearSceneModels();
      this.currentKey = '';
      let artifactUnavailable = '';
      if (this.isFullCommitHash(latestCommit)) {
        const artifact = await this.loadRenderArtifact(projectId, latestCommit, token);
        if (token !== this.loadToken) {
          if (artifact && artifact.model) this.disposeArtifactObject(artifact.model);
          return;
        }
        if (artifact && artifact.model) {
          this.model = artifact.model;
          this.modelKind = 'artifact';
          this.artifactManifest = artifact.manifest;
          this.artifactTilesLoaded = 1;
          this.modelScene.add(this.model);
          this.orientModel(this.model);
          this.currentKey = key;
          this.fitArtifactToManifest(artifact.manifest, this.model);
          this.applyPlanCut();
          this.applyModelLevel();
          this.showOrbitHint();
          if (!requiresIfcDiff) {
            this.loadRemainingArtifactTiles(artifact.tiles.slice(1), artifact.model, token);
            artifact.semanticIndex.then(index => {
              if (token !== this.loadToken || this.model !== artifact.model) return;
              this.artifactSemanticIndex = index;
              this.setModelSourceMeta(mode);
            }).catch(error => {
              if (token === this.loadToken && this.model === artifact.model) {
                this.modelMeta.textContent = `render artifact · semantic index unavailable`;
                console.warn('Render artifact semantic index unavailable:', error);
              }
            });
            await this.applyDiff(changes, mode, token);
            if (token !== this.loadToken) return;
            this.setModelSourceMeta(mode);
            return;
          }
          artifactUnavailable = 'Render artifact loaded; raw IFC fallback is needed for change overlays.';
          artifact.semanticIndex.catch(() => {});
          this.abortPendingLoads();
          this.clearSceneModels();
        } else {
          artifactUnavailable = artifact && artifact.reason
            ? artifact.reason
            : 'Render artifact unavailable; using raw IFC fallback.';
        }
      } else {
        artifactUnavailable = 'Render artifact skipped because this commit ID is abbreviated; using raw IFC fallback.';
      }
      await this.loadIfcFallback(projectId, latestCommit, key, token, artifactUnavailable);
      await this.applyDiff(changes, mode, token);
      if (token !== this.loadToken) return;
      this.setModelSourceMeta(mode);
    } catch (error) {
      if (token !== this.loadToken) return;
      if (error && error.name === 'AbortError') return;
      this.clear(`Raw IFC fallback failed: ${error.message}`);
    }
  }

  isFullCommitHash(value) {
    return typeof value === 'string' && /^[0-9a-f]{64}$/i.test(value);
  }

  hasVisualChanges(changes) {
    return ((changes && changes.visual_diff && changes.visual_diff.elements) || [])
      .some(element => element.kind && element.kind !== 'unchanged');
  }

  setLoadStatus(message) {
    this.planStatus.textContent = message;
    this.modelStatus.textContent = message;
  }

  setModelSourceMeta(mode) {
    const suffix = mode === 'changes' ? 'changes only' : 'full model';
    if (this.modelKind === 'artifact') {
      const progress = this.artifactManifest && this.artifactManifest.tiles
        ? `${this.artifactTilesLoaded}/${this.artifactManifest.tiles.length} tiles`
        : 'coarse tile';
      this.planStatus.textContent = '';
      this.modelStatus.textContent = '';
      this.planMeta.textContent = `render artifact · ${progress}`;
      this.modelMeta.textContent = `render artifact · ${progress} · ${suffix}`;
      return;
    }
    this.planStatus.textContent = '';
    this.modelStatus.textContent = '';
    this.planMeta.textContent = `raw IFC fallback · ${suffix}`;
    this.modelMeta.textContent = `raw IFC fallback · ${suffix}`;
  }

  async loadIfcFallback(projectId, commit, key, token, reason) {
    this.setLoadStatus(reason || 'Loading raw IFC fallback...');
    const model = await this.loadIfcModel(projectId, commit, token);
    if (token !== this.loadToken) {
      this.releaseIfcModel(model);
      return;
    }
    this.model = model;
    this.modelKind = 'ifc';
    this.modelScene.add(this.model);
    this.currentKey = key;
    const firstSceneStartedAt = performance.now();
    this.fitToModel(this.model);
    this.recordLoadMetric('ifc_first_scene', firstSceneStartedAt, {fallback: true});
    if (!this.modelBox) {
      if (token !== this.loadToken) return;
      this.clear('No 3D geometry found in this raw IFC fallback.');
      return;
    }
    const storeyStartedAt = performance.now();
    await this.extractStoreys(this.model);
    this.recordLoadMetric('ifc_storey_index', storeyStartedAt, {fallback: true});
    if (token !== this.loadToken) return;
    this.applyPlanCut();
    this.applyModelLevel();
    this.showOrbitHint();
  }

  async loadRenderArtifact(projectId, commit, token) {
    const controller = new AbortController();
    this.artifactAbortController = controller;
    const statusUrl = `/v1/projects/${encodeURIComponent(projectId)}/render/${encodeURIComponent(commit)}/status`;
    this.setLoadStatus('Checking render artifact...');
    try {
      const response = await fetch(statusUrl, {headers, signal: controller.signal});
      if (!response.ok) {
        return {reason: `Render artifact status unavailable (${response.status}); using raw IFC fallback.`};
      }
      const status = await response.json();
      if (token !== this.loadToken) throw new DOMException('Render artifact load superseded', 'AbortError');
      if (!status || status.status !== 'ready' || !status.manifest) {
        return {reason: 'Render artifact unavailable; using raw IFC fallback.'};
      }
      const manifest = status.manifest;
      if (status.commit_hash !== commit || manifest.commit_hash !== commit || manifest.project_id !== projectId) {
        return {reason: 'Render artifact identity did not match this commit; using raw IFC fallback.'};
      }
      const tiles = (manifest.tiles || []).filter(tile =>
        tile && tile.artifact && /gltf-binary/i.test(tile.artifact.content_type || ''));
      if (!tiles.length || !manifest.semantic_index || !manifest.semantic_index.artifact) {
        return {reason: 'Render artifact is incomplete; using raw IFC fallback.'};
      }
      tiles.sort((a, b) => (a.lod - b.lod) || (b.geometric_error - a.geometric_error));
      this.setLoadStatus('Downloading render artifact coarse tile...');
      const first = await this.loadArtifactTile(tiles[0], controller.signal, token);
      if (token !== this.loadToken) {
        this.disposeArtifactObject(first);
        throw new DOMException('Render artifact load superseded', 'AbortError');
      }
      const model = new THREE.Group();
      model.name = 'Vex render artifact';
      model.userData.vexArtifact = true;
      model.add(first);
      const semanticIndex = this.fetchArtifactSemanticIndex(manifest.semantic_index.artifact, controller.signal);
      return {model, manifest, tiles, semanticIndex};
    } catch (error) {
      if (error && error.name === 'AbortError') throw error;
      console.warn('Render artifact load failed; falling back to raw IFC:', error);
      return {reason: `Render artifact failed; using raw IFC fallback (${error.message || 'load error'}).`};
    }
  }

  artifactResourceUrl(resource) {
    if (!resource || typeof resource.uri !== 'string') throw new Error('missing artifact resource URI');
    const url = new URL(resource.uri, window.location.origin);
    if (url.origin !== window.location.origin) throw new Error('artifact resource must use this bridge');
    return url.href;
  }

  async fetchArtifactResource(resource, signal) {
    const url = this.artifactResourceUrl(resource);
    const response = await fetch(url, {headers, signal});
    if (!response.ok) throw new Error(`${url} -> ${response.status}`);
    return response.arrayBuffer();
  }

  async loadArtifactTile(tile, signal, token) {
    const startedAt = performance.now();
    const buffer = await this.fetchArtifactResource(tile.artifact, signal);
    if (token !== this.loadToken) throw new DOMException('Render artifact load superseded', 'AbortError');
    const scene = await new Promise((resolve, reject) => {
      this.gltfLoader.parse(buffer, '', gltf => resolve(gltf.scene || new THREE.Group()), reject);
    });
    if (token !== this.loadToken) {
      this.disposeArtifactObject(scene);
      throw new DOMException('Render artifact load superseded', 'AbortError');
    }
    scene.name = `Artifact tile ${tile.tile_id || tile.lod}`;
    scene.userData.vexTileId = tile.tile_id;
    this.recordLoadMetric('artifact_tile_decode', startedAt, {
      bytes: buffer.byteLength,
      tile_id: tile.tile_id,
      lod: tile.lod
    });
    return scene;
  }

  async fetchArtifactSemanticIndex(resource, signal) {
    const buffer = await this.fetchArtifactResource(resource, signal);
    const text = new TextDecoder().decode(buffer);
    return JSON.parse(text);
  }

  async loadRemainingArtifactTiles(tiles, model, token) {
    for (const tile of tiles) {
      try {
        if (token !== this.loadToken || this.model !== model || !this.artifactAbortController) return;
        this.setLoadStatus(`Render artifact: loading tile ${this.artifactTilesLoaded + 1}/${this.artifactManifest.tiles.length}...`);
        const scene = await this.loadArtifactTile(tile, this.artifactAbortController.signal, token);
        if (token !== this.loadToken || this.model !== model) {
          this.disposeArtifactObject(scene);
          return;
        }
        model.add(scene);
        ++this.artifactTilesLoaded;
        this.setModelSourceMeta(currentViewMode);
      } catch (error) {
        if (error && error.name === 'AbortError') return;
        console.warn('Render artifact tile failed to load:', error);
        if (token === this.loadToken && this.model === model) {
          this.modelMeta.textContent = `render artifact · ${this.artifactTilesLoaded} tiles (some unavailable)`;
        }
      }
    }
    if (token === this.loadToken && this.model === model) {
      this.setModelSourceMeta(currentViewMode);
    }
  }

  fitArtifactToManifest(manifest, model) {
    const bounds = (manifest && manifest.tiles || []).map(tile => tile.bounds).filter(bounds =>
      bounds && Array.isArray(bounds.min) && Array.isArray(bounds.max)
      && bounds.min.length === 3 && bounds.max.length === 3
      && bounds.min.concat(bounds.max).every(Number.isFinite)
    );
    if (bounds.length) {
      this.modelBox = new THREE.Box3();
      for (const tile of bounds) {
        for (const x of [tile.min[0], tile.max[0]]) {
          for (const y of [tile.min[1], tile.max[1]]) {
            for (const z of [tile.min[2], tile.max[2]]) {
              const point = new THREE.Vector3(x, y, z);
              if (this.upAxisFix) point.applyAxisAngle(new THREE.Vector3(1, 0, 0), this.upAxisFix);
              this.modelBox.expandByPoint(point);
            }
          }
        }
      }
      this.fitToModel(model, this.modelBox);
      return;
    }
    this.fitToModel(model);
  }

  disposeArtifactObject(object) {
    if (!object) return;
    object.traverse(item => {
      if (item.geometry) item.geometry.dispose();
      const materials = Array.isArray(item.material) ? item.material : item.material ? [item.material] : [];
      for (const material of materials) {
        for (const value of Object.values(material)) {
          if (value && value.isTexture) value.dispose();
        }
        material.dispose();
      }
    });
  }

  // Re-orient an IFC object from web-ifc's Y-up output onto this viewer's Z-up
  // world. Applied to the model group and to every diff/selection subset (which
  // are added at the scene root, not under the model group, so they must be
  // rotated individually to stay aligned). web-ifc's axis convention is constant
  // across files; set upAxisFix to 0 in the constructor if a future build ever
  // returns Z-up geometry.
  orientModel(object) {
    if (!object || !this.upAxisFix) return object;
    object.rotation.x = this.upAxisFix;
    object.updateMatrixWorld(true);
    return object;
  }

  async getIfcLoader() {
    if (this.ifcLoader) return this.ifcLoader;
    const loader = new IFCLoader();
    loader.ifcManager.setWasmPath('/assets/viewer/web-ifc/');
    // Keep one worker-backed parser for the life of this desktop window. A new
    // worker per commit leaves large IFC parser state alive until the app exits.
    try {
      await loader.ifcManager.useWebWorkers(true, '/assets/viewer/web-ifc-three/IFCWorker.js');
    } catch (error) {
      console.warn('IFC web worker unavailable; parsing on the main thread instead', error);
    }
    if (loader.ifcManager.applyWebIfcConfig) {
      await loader.ifcManager.applyWebIfcConfig({COORDINATE_TO_ORIGIN: true, USE_FAST_BOOLS: true});
    }
    this.ifcLoader = loader;
    return loader;
  }

  async loadLocalFile(file, projectId) {
    this.abortPendingLoads();
    const token = ++this.loadToken;
    this.clearSceneModels();
    this.currentKey = '';
    this.planStatus.textContent = `Preparing local preview of ${file.name}…`;
    this.modelStatus.textContent = `Preparing local preview of ${file.name}…`;
    const buffer = await file.arrayBuffer();
    const model = await this.parseIfcBuffer(buffer, 'Preparing local preview', token);
    if (token !== this.loadToken || selectedProject !== projectId) {
      this.releaseIfcModel(model);
      return;
    }
    this.model = model;
    this.modelKind = 'ifc';
    this.modelScene.add(model);
    this.currentKey = `local:${projectId}`;
    const firstSceneStartedAt = performance.now();
    this.fitToModel(model);
    this.recordLoadMetric('ifc_first_scene', firstSceneStartedAt, {local_preview: true});
    if (!this.modelBox) {
      this.clear('No 3D geometry found in this IFC.');
      return;
    }
    const storeyStartedAt = performance.now();
    await this.extractStoreys(model);
    this.recordLoadMetric('ifc_storey_index', storeyStartedAt, {local_preview: true});
    if (token !== this.loadToken) return;
    this.applyPlanCut();
    this.applyModelLevel();
    this.planStatus.textContent = '';
    this.modelStatus.textContent = '';
    this.planMeta.textContent = 'local preview · semantic import running';
    this.modelMeta.textContent = 'local preview · semantic import running';
  }

  abortPendingIfcLoad() {
    if (this.ifcAbortController) this.ifcAbortController.abort();
    this.ifcAbortController = null;
  }

  abortPendingLoads() {
    this.abortPendingIfcLoad();
    if (this.artifactAbortController) this.artifactAbortController.abort();
    this.artifactAbortController = null;
  }

  recordLoadMetric(stage, startedAt, detail = {}) {
    const elapsedMs = Math.round(performance.now() - startedAt);
    console.info('[vex-performance]', {stage, elapsed_ms: elapsedMs, ...detail});
    return elapsedMs;
  }

  async loadIfcModel(projectId, commit, token = this.loadToken) {
    const url = `/v1/projects/${encodeURIComponent(projectId)}/ifc/${encodeURIComponent(commit)}`;
    const controller = new AbortController();
    this.ifcAbortController = controller;
    const fetchStartedAt = performance.now();
    this.planStatus.textContent = 'Downloading committed IFC...';
    this.modelStatus.textContent = 'Downloading committed IFC...';
    const response = await fetch(url, {headers, signal: controller.signal});
    if (!response.ok) throw new Error(`${url} -> ${response.status}`);
    const buffer = await response.arrayBuffer();
    this.recordLoadMetric('ifc_download', fetchStartedAt, {
      bytes: buffer.byteLength,
      project_id: projectId,
      commit
    });
    if (token !== this.loadToken) throw new DOMException('IFC load superseded', 'AbortError');
    return this.parseIfcBuffer(buffer, 'Loading IFC geometry', token);
  }

  async parseIfcBuffer(buffer, progressLabel, token = this.loadToken) {
    const loader = await this.getIfcLoader();
    // Real progress feedback (not just a static "Loading..." string) so a big
    // model's load time reads as "working, N% of M elements" instead of a
    // silent hang.
    loader.ifcManager.setOnProgress(({loaded, total}) => {
      if (!total) return;
      const pct = Math.min(100, Math.round((loaded / total) * 100));
      const label = `${progressLabel}... ${pct}%`;
      this.planStatus.textContent = label;
      this.modelStatus.textContent = label;
    });
    // Yield once so the "Loading IFC geometry..." status paints before the
    // synchronous web-ifc parse takes over the main thread.
    await new Promise(resolve => setTimeout(resolve, 0));
    if (token !== this.loadToken) throw new DOMException('IFC load superseded', 'AbortError');
    const parseStartedAt = performance.now();
    const model = await loader.parse(buffer);
    this.recordLoadMetric('ifc_parse_and_tessellate', parseStartedAt, {
      bytes: buffer.byteLength
    });
    if (token !== this.loadToken) {
      this.releaseIfcModel(model);
      throw new DOMException('IFC load superseded', 'AbortError');
    }
    // web-ifc returns geometry in IFC's Y-up frame (building height runs along
    // world Y); re-orient it onto this viewer's Z-up world so the 3D view stands
    // upright AND the top-down 2D pane reads as a real floor plan, not a side
    // elevation.
    this.orientModel(model);
    return model;
  }

  releaseIfcModel(model) {
    if (model && typeof model.close === 'function') model.close(this.modelScene);
  }

  clearSceneModels() {
    if (this.selectionSubset && this.selectionSubset.parent) this.selectionSubset.parent.remove(this.selectionSubset);
    this.selectionSubset = null;
    this.selectedId = null;
    const panel = document.getElementById('propsPanel');
    if (panel) panel.classList.remove('open');
    const closedModels = new Set();
    for (const object of [this.model, ...this.highlightObjects, ...this.removedObjects]) {
      if (object && typeof object.close === 'function' && !closedModels.has(object)) {
        closedModels.add(object);
        this.releaseIfcModel(object);
      }
      if (object && object.userData && object.userData.vexArtifact) this.disposeArtifactObject(object);
      if (object && object.parent) object.parent.remove(object);
    }
    this.model = null;
    this.modelKind = null;
    this.modelBox = null;
    this.artifactManifest = null;
    this.artifactSemanticIndex = null;
    this.artifactTilesLoaded = 0;
    this.highlightObjects = [];
    this.removedObjects = [];
    this.storeys = [];
    this.planLevelIndex = 0;
    this.planCutZ = null;
    this.modelLevelIndex = null;
    this.modelRenderer.clippingPlanes = this.sectionActive ? [this.sectionPlane] : [];
    this.planRenderer.clippingPlanes = this.planCutMode === 'top' ? [] : [this.planClipPlane, this.planClipPlaneLower];
    if (typeof this.onStoreys === 'function') this.onStoreys([]);
  }

  async applyDiff(changes, mode, token) {
    if (!this.model) return;
    for (const object of [...this.highlightObjects, ...this.removedObjects]) {
      if (object && object.parent) object.parent.remove(object);
    }
    this.highlightObjects = [];
    this.removedObjects = [];
    if (this.modelKind !== 'ifc') {
      // GLB tiles deliberately render without web-ifc subsets. Switching to
      // "Changes Only" reloads this commit through the raw IFC fallback.
      this.setObjectOpacity(this.model, 1);
      this.model.visible = true;
      return;
    }
    const grouped = groupedGlobalIds(changes);
    const hasChanges = Object.values(grouped).some(set => set.size > 0);
    this.setObjectOpacity(this.model, mode === 'changes' ? 0.08 : 1);
    this.model.visible = mode !== 'changes' || !hasChanges;
    for (const [kind, ids] of Object.entries(grouped)) {
      if (!ids.size || kind === 'removed') continue;
      const expressIds = await this.globalIdsToExpressIds(this.model, ids);
      if (token !== this.loadToken) return;
      if (!expressIds.length) continue;
      const material = highlightMaterial(kind);
      const subset = this.model.createSubset({ids: expressIds, material, scene: this.modelScene, removePrevious: false, customID: `vex-${kind}`});
      this.orientModel(subset);
      this.highlightObjects.push(subset);
    }
    if (grouped.removed.size && changes.previous_commit) {
      await this.addRemovedSubset(changes, grouped.removed, token);
    }
  }

  async addRemovedSubset(changes, ids, token) {
    try {
      const previous = await this.loadIfcModel(changes.project_id, changes.previous_commit);
      if (token !== this.loadToken) return;
      const expressIds = await this.globalIdsToExpressIds(previous, ids);
      if (token !== this.loadToken || !expressIds.length) return;
      this.setObjectOpacity(previous, 0.04);
      previous.visible = false;
      const material = highlightMaterial('removed');
      const subset = previous.createSubset({ids: expressIds, material, scene: this.modelScene, removePrevious: false, customID: 'vex-removed'});
      this.orientModel(subset);
      this.removedObjects.push(previous, subset);
    } catch (error) {
      this.modelMeta.textContent = `removed unavailable: ${error.message}`;
    }
  }

  async globalIdsToExpressIds(model, wanted) {
    const out = [];
    const structure = await model.getSpatialStructure();
    const visit = async node => {
      if (!node) return;
      const direct = valueOf(node.GlobalId || node.globalId);
      if (direct && wanted.has(direct) && Number.isFinite(node.expressID)) out.push(node.expressID);
      if (!direct && Number.isFinite(node.expressID)) {
        try {
          const props = await model.getItemProperties(node.expressID, false);
          const globalId = valueOf(props && (props.GlobalId || props.globalId));
          if (globalId && wanted.has(globalId)) out.push(node.expressID);
        } catch (_) {}
      }
      for (const child of node.children || []) await visit(child);
    };
    await visit(structure);
    return [...new Set(out)];
  }

  setObjectOpacity(object, opacity) {
    object.traverse(item => {
      const materials = Array.isArray(item.material) ? item.material : item.material ? [item.material] : [];
      for (const material of materials) {
        material.transparent = opacity < 1;
        material.opacity = opacity;
        material.depthWrite = opacity >= 0.5;
      }
    });
  }

  fitToModel(model, bounds = null) {
    const box = bounds ? bounds.clone() : new THREE.Box3().setFromObject(model);
    if (box.isEmpty()) return;
    this.modelBox = box;
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3());
    const radius = Math.max(size.x, size.y, size.z, 1);
    this.modelCamera.position.set(center.x + radius, center.y - radius, center.z + radius * 0.7);
    if (this.modelCamera.isPerspectiveCamera) {
      this.modelCamera.near = Math.max(radius / 1000, 0.01);
      this.modelCamera.far = radius * 100;
    }
    this.modelCamera.lookAt(center);
    this.modelCamera.updateProjectionMatrix();
    this.controls.target.copy(center);
    // Bound the dolly so the wheel can't fly past the model or invert through it.
    this.controls.minDistance = radius * 0.05;
    this.controls.maxDistance = radius * 40;
    this.controls.update();
    this.rebuildHelpers(box);
    if (this.sectionActive) {
      const slider = document.getElementById('sectionSlider');
      this.setSection(slider ? Number(slider.value) : 100);
    }
    this.planPan.set(0, 0);
    this.planZoom = 1;
    this.updatePlanFraming();
    this.applyPlanCut();
  }

  updatePlanFraming() {
    if (!this.modelBox) return;
    const center = this.modelBox.getCenter(new THREE.Vector3());
    const size = this.modelBox.getSize(new THREE.Vector3());
    const radius = Math.max(size.x, size.y, size.z, 1);
    const rect = this.planCanvas.getBoundingClientRect();
    const aspect = rect.width / Math.max(rect.height, 1);
    const planSize = (Math.max(size.x, size.y, 1) * 0.58) / this.planZoom;
    const cx = center.x + this.planPan.x;
    const cy = center.y + this.planPan.y;
    this.planCamera.left = -planSize * aspect;
    this.planCamera.right = planSize * aspect;
    this.planCamera.top = planSize;
    this.planCamera.bottom = -planSize;
    this.planCamera.near = -radius * 10;
    this.planCamera.far = radius * 10;
    this.planCamera.position.set(cx, cy, center.z + radius * 2);
    this.planCamera.lookAt(cx, cy, center.z);
    this.planCamera.updateProjectionMatrix();
  }

  beginPlanPan(event) {
    if (!this.modelBox) return;
    this.planDrag = {x: event.clientX, y: event.clientY};
    try { this.planCanvas.setPointerCapture(event.pointerId); } catch (e) {}
    this.planCanvas.classList.add('panning');
  }

  movePlanPan(event) {
    if (!this.planDrag) return;
    const rect = this.planCanvas.getBoundingClientRect();
    const worldPerPx = (this.planCamera.top - this.planCamera.bottom) / Math.max(rect.height, 1);
    const dx = event.clientX - this.planDrag.x;
    const dy = event.clientY - this.planDrag.y;
    this.planPan.x -= dx * worldPerPx;
    this.planPan.y += dy * worldPerPx;
    this.planDrag = {x: event.clientX, y: event.clientY};
    this.updatePlanFraming();
  }

  endPlanPan(event) {
    if (!this.planDrag) return;
    this.planDrag = null;
    try { this.planCanvas.releasePointerCapture(event.pointerId); } catch (e) {}
    this.planCanvas.classList.remove('panning');
  }

  zoomPlan(event) {
    if (!this.modelBox) return;
    event.preventDefault();
    const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
    this.planZoom = Math.min(50, Math.max(0.1, this.planZoom * factor));
    this.updatePlanFraming();
  }

  resize() {
    this.resizeRenderer(this.modelRenderer, this.modelCanvas, this.modelCamera);
    this.resizeRenderer(this.planRenderer, this.planCanvas, this.planCamera);
    this.updatePlanFraming();
  }

  resizeRenderer(renderer, canvas, camera) {
    const rect = canvas.getBoundingClientRect();
    const width = Math.max(1, Math.floor(rect.width));
    const height = Math.max(1, Math.floor(rect.height));
    renderer.setSize(width, height, false);
    const aspect = width / Math.max(height, 1);
    if (camera.isPerspectiveCamera) {
      camera.aspect = aspect;
      camera.updateProjectionMatrix();
    } else if (camera === this.modelOrtho) {
      const extent = this.modelBox ? Math.max(this.modelBox.getSize(new THREE.Vector3()).x, this.modelBox.getSize(new THREE.Vector3()).y, 1) * 0.7 : 10;
      camera.left = -extent * aspect;
      camera.right = extent * aspect;
      camera.top = extent;
      camera.bottom = -extent;
      camera.updateProjectionMatrix();
    }
  }
}

function groupedGlobalIds(changes) {
  const out = {added: new Set(), removed: new Set(), modified: new Set(), moved: new Set(), renamed: new Set()};
  const elements = ((changes && changes.visual_diff && changes.visual_diff.elements) || []);
  for (const element of elements) {
    if (!out[element.kind]) continue;
    const id = idLabel(element.id);
    if (id) out[element.kind].add(id);
  }
  return out;
}

function highlightMaterial(kind) {
  const colors = {added: 0x43c26b, removed: 0xe05a47, modified: 0xd99a2b, moved: 0x4b8fe3, renamed: 0x9b6bd3};
  return new THREE.MeshLambertMaterial({
    color: colors[kind] || 0xa8aaa7,
    transparent: true,
    opacity: 0.9,
    side: THREE.DoubleSide,
    depthTest: true
  });
}

function valueOf(value) {
  if (value == null) return null;
  if (typeof value === 'string') return value;
  if (typeof value.value === 'string') return value.value;
  return null;
}

async function saveInbox(event) {
  event.preventDefault();
  const projectName = els.projectName.value.trim();
  if (!projectName) {
    els.inboxHint.textContent = 'Enter a project name first.';
    els.projectName.focus();
    return;
  }
  if (!els.projectId.value.trim()) els.projectId.value = genProjectId();
  const body = {
    project_id: els.projectId.value.trim(),
    project_name: projectName,
    include: ['*.ifc']
  };
  const root = lastSetup && (lastSetup.inbox_root_path || lastSetup.suggested_inbox_path);
  if (pickedFolderPath && root && pickedFolderPath.startsWith(root)) {
    body.local_path = pickedFolderPath;
  }
  const response = await api('/v1/setup/inbox', {method: 'POST', headers: jsonHeaders, body: JSON.stringify(body)});
  els.setupPanel.classList.remove('open');
  els.setupForm.reset();
  els.projectId.value = '';
  pickedFolderPath = null;
  await selectProject(response.repo.project_id);
}

function openDeletePanel(project) {
  pendingDeleteProject = project;
  els.deleteProjectText.textContent = `${project.project_name || project.project_id} / ${project.local_path}`;
  const keep = els.deleteForm.querySelector('input[value="keep_folder"]');
  if (keep) keep.checked = true;
  els.deletePanel.classList.add('open');
}

function closeDeletePanel() {
  pendingDeleteProject = null;
  els.deletePanel.classList.remove('open');
}

async function deleteProject(event) {
  event.preventDefault();
  if (!pendingDeleteProject) return;
  const projectId = pendingDeleteProject.project_id;
  const selected = els.deleteForm.querySelector('input[name="deletePolicy"]:checked');
  const deletion_policy = selected ? selected.value : 'keep_folder';
  await api(`/v1/projects/${encodeURIComponent(projectId)}`, {
    method: 'DELETE', headers: jsonHeaders, body: JSON.stringify({deletion_policy})
  });
  closeDeletePanel();
  if (selectedProject === projectId) {
    selectedProject = null;
    selectedCommit = null;
    projectCommits = [];
    latestChanges = null;
    els.history.innerHTML = '<div class="empty">No project selected.</div>';
    els.historyMeta.textContent = '';
    renderChanges(null);
  }
  await refresh();
}

function optionalValue(id) { const value = document.getElementById(id).value.trim(); return value || null; }
function parentFor(commit) { return commit && commit.parents && commit.parents.length ? commit.parents[0] : null; }
function populatePlanLevels(storeys) {
  if (!els.planLevel) return;
  const list = Array.isArray(storeys) ? storeys : [];
  if (!list.length) {
    els.planLevel.innerHTML = '<option value="auto">Auto level</option>';
    els.planLevel.disabled = true;
    return;
  }
  const options = list.map((storey, index) => {
    const elevation = Number.isFinite(storey.elevation) ? ` (${Math.round(storey.elevation)})` : '';
    return `<option value="${index}">${escapeHtml(storey.name || ('Level ' + (index + 1)))}${escapeHtml(elevation)}</option>`;
  });
  els.planLevel.innerHTML = options.join('');
  els.planLevel.value = '0';
  els.planLevel.disabled = ifcViewer ? ifcViewer.planCutMode === 'top' : false;
}
function populateModelLevels(storeys) {
  if (!els.modelLevel) return;
  const list = Array.isArray(storeys) ? storeys : [];
  if (!list.length) {
    els.modelLevel.innerHTML = '<option value="full">Full structure</option>';
    els.modelLevel.value = 'full';
    els.modelLevel.disabled = true;
    return;
  }
  const options = ['<option value="full">Full structure</option>'];
  list.forEach((storey, index) => {
    const elevation = Number.isFinite(storey.elevation) ? ` (${Math.round(storey.elevation)})` : '';
    options.push(`<option value="${index}">${escapeHtml(storey.name || ('Level ' + (index + 1)))}${escapeHtml(elevation)}</option>`);
  });
  els.modelLevel.innerHTML = options.join('');
  els.modelLevel.value = 'full';
  els.modelLevel.disabled = false;
}
function populateLevels(storeys) {
  populatePlanLevels(storeys);
  populateModelLevels(storeys);
}
function short(value) { return value ? value.slice(0, 12) : 'none'; }
function idLabel(id) { return typeof id === 'string' ? id : id && (id.GlobalId || id.StepId || id.step_id); }
function elementType(element) { return element.type_name || element.type || 'IFC element'; }
function formatTime(seconds) { return seconds ? new Date(seconds * 1000).toLocaleString() : 'not caught yet'; }
function escapeHtml(value) { return String(value).replace(/[&<>"']/g, ch => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[ch])); }

try {
  ifcViewer = new RealIfcViewer({
    planCanvas: els.planCanvas,
    modelCanvas: els.modelCanvas,
    planStatus: els.planStatus,
    modelStatus: els.modelStatus,
    planMeta: els.planMeta,
    modelMeta: els.modelMeta
  });
  ifcViewer.onStoreys = populateLevels;
} catch (error) {
  // 3D rendering may be unavailable (e.g. no WebGL/GPU context). Keep the rest
  // of the dashboard fully usable for pairing and project management instead of
  // aborting startup.
  ifcViewer = null;
  console.error('3D viewer unavailable:', error);
  const msg = '3D preview unavailable on this machine (no WebGL).';
  if (els.modelStatus) els.modelStatus.textContent = msg;
  if (els.planStatus) els.planStatus.textContent = msg;
}
refresh();
loadHealth();
checkUpdates();
setInterval(refresh, 15000);
setInterval(loadHealth, 60000);
setInterval(checkUpdates, 1800000);
</script>
</body>
</html>
"#;
