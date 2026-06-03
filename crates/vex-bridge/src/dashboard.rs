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
button, input {
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
  grid-template-columns: minmax(280px, 1fr) minmax(280px, 1fr);
  gap: 1px;
  background: var(--line);
}
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
.sb-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--subtle); flex: none; }
.sb-dot.ok { background: var(--green); }
.sb-dot.warn { background: var(--amber); }
@media (max-width: 760px) {
  .main { grid-template-columns: 1fr; grid-template-rows: minmax(160px, 220px) minmax(180px, 240px) minmax(220px, 1fr); }
  .sidebar, .history { border-right: 0; border-bottom: 1px solid var(--line); }
  .view-grid { grid-template-columns: 1fr; grid-template-rows: 1fr 1fr; }
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
    <button id="syncButton">Sync</button>
    <button class="primary" id="refreshButton">Refresh</button>
  </div>
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
          <div class="view-toggle" id="viewToggle">
            <button type="button" data-mode="full" class="active">Full Model</button>
            <button type="button" data-mode="changes">Changes Only</button>
          </div>
          <div class="badges" id="countBadges"></div>
        </div>
      </div>
      <div class="view-grid">
        <div class="view-pane">
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
  </footer>
</div>
<div class="setup" id="setupPanel">
  <div class="panel-head"><div class="panel-title">Add Inbox</div><button id="closeSetup" type="button">Close</button></div>
  <form id="setupForm">
    <div class="field"><label for="projectName">Project Name</label><input id="projectName" placeholder="Commercial Tower"></div>
    <div class="field"><label for="folderName">Folder Name</label><input id="folderName" required placeholder="Commercial-Tower"></div>
    <div class="field" id="browseField" style="display:none"><button type="button" id="browseFolder">Browse for folder…</button></div>
    <div class="row-meta" id="inboxHint">Folder will be created inside VexInbox.</div>
    <div class="field"><label for="ifcGuid">IFC Project GUID</label><input id="ifcGuid" placeholder="2HnQxDrSH5sBbC4NkVOGR8"></div>
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
<script type="importmap">
{
  "imports": {
    "three": "/assets/viewer/three/three.module.js",
    "three/examples/jsm/utils/BufferGeometryUtils": "/assets/viewer/three/examples/jsm/utils/BufferGeometryUtils.js",
    "three/examples/jsm/controls/OrbitControls": "/assets/viewer/three/examples/jsm/controls/OrbitControls.js",
    "web-ifc": "/assets/viewer/web-ifc/web-ifc-api.js"
  }
}
</script>
<script type="module">
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls';
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
  pairButton: document.getElementById('pairButton'), syncButton: document.getElementById('syncButton'),
  setupPanel: document.getElementById('setupPanel'), setupForm: document.getElementById('setupForm'),
  deletePanel: document.getElementById('deletePanel'), deleteForm: document.getElementById('deleteForm'),
  deleteProjectText: document.getElementById('deleteProjectText'),
  inboxHint: document.getElementById('inboxHint'), viewToggle: document.getElementById('viewToggle'),
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
  browseField: document.getElementById('browseField'), browseFolder: document.getElementById('browseFolder')
};

// Native desktop bridge (present only inside the vex-desktop window). Falls back
// gracefully to plain browser behaviour when unavailable.
const native = (typeof window !== 'undefined' && window.__vexNative && window.__vexNative.available)
  ? window.__vexNative : null;
let pickedFolderPath = null;
let healthInfo = null;

let ifcViewer = null;

document.getElementById('refreshButton').addEventListener('click', refresh);
document.getElementById('setupButton').addEventListener('click', () => els.setupPanel.classList.add('open'));
els.pairButton.addEventListener('click', startOrPollPairing);
els.syncButton.addEventListener('click', syncSelectedProject);
document.getElementById('closeSetup').addEventListener('click', () => els.setupPanel.classList.remove('open'));
document.getElementById('closeDelete').addEventListener('click', closeDeletePanel);
document.getElementById('cancelDelete').addEventListener('click', closeDeletePanel);
els.setupForm.addEventListener('submit', saveInbox);
els.deleteForm.addEventListener('submit', deleteProject);
els.viewToggle.addEventListener('click', event => {
  const button = event.target.closest('button[data-mode]');
  if (!button) return;
  currentViewMode = button.dataset.mode;
  for (const item of els.viewToggle.querySelectorAll('button')) item.classList.toggle('active', item === button);
  renderChanges(latestChanges);
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
    document.getElementById('folderName').value = base;
    const root = lastSetup && (lastSetup.inbox_root_path || lastSetup.suggested_inbox_path);
    els.inboxHint.textContent = (root && path.startsWith(root))
      ? `Tracking ${path}`
      : `Will create a managed folder named "${base}" inside ${root || 'VexInbox'}.`;
  } catch (error) {
    els.inboxHint.textContent = `Folder pick failed: ${error.message || error}`;
  }
}

async function api(path, options = {}) {
  const response = await fetch(path, options);
  if (!response.ok) throw new Error(`${path} -> ${response.status}`);
  return await response.json();
}

async function refresh(options = {}) {
  try {
    const setup = await api('/v1/setup/status', {headers});
    lastSetup = setup;
    const paired = setup.pair_status && setup.pair_status.status === 'paired';
    els.statusDot.className = `status-dot ${paired && setup.watch.active_watchers > 0 ? 'ok' : 'warn'}`;
    els.topStatus.textContent = `${pairText(setup.pair_status)} / ${setup.watch.active_watchers}/${setup.watch.configured_projects} watching`;
    updatePairButton(setup.pair_status);
    els.syncButton.disabled = !selectedProject;
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
  } catch (_) { /* best-effort */ }
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
    els.sbActivity.textContent = project ? `${escapeHtml(project.project_name || project.project_id)} · ${project.seen_import_count} caught` : '';
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

async function syncSelectedProject() {
  if (!selectedProject) return;
  els.syncButton.disabled = true;
  try {
    const result = await api('/v1/repo/push', {
      method: 'POST', headers: jsonHeaders, body: JSON.stringify({project_id: selectedProject, branch: 'main'})
    });
    els.topStatus.textContent = `Synced ${short(result.commit_hash)}`;
  } catch (error) {
    els.topStatus.textContent = error.message;
  } finally {
    els.syncButton.disabled = false;
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
    button.innerHTML = `<div class="row-title">${escapeHtml(project.project_name || project.project_id)}</div>
      <div class="row-meta">${project.active ? 'Watching' : 'Inactive'} / ${escapeHtml(project.local_path)}</div>
      <div class="row-meta">${project.seen_import_count} caught</div>`;
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
}

function renderRows(elements) {
  els.changeRows.innerHTML = elements.length ? '' : '<tr><td colspan="3" class="row-meta">No element-level changes.</td></tr>';
  for (const element of elements.slice(0, 150)) {
    const tr = document.createElement('tr');
    tr.innerHTML = `<td class="kind ${element.kind}">${escapeHtml(element.kind)}</td>
      <td>${escapeHtml(elementType(element))}</td>
      <td>${escapeHtml(element.hint || idLabel(element.id) || '')}</td>`;
    els.changeRows.appendChild(tr);
  }
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
    this.planClipPlane = new THREE.Plane(new THREE.Vector3(0, 0, -1), 0);
    this.planRenderer.clippingPlanes = [this.planClipPlane];
    this.storeys = [];
    this.planLevelIndex = 0;
    this.planCutZ = null;
    this.modelRenderer = this.makeRenderer(modelCanvas);
    this.modelRenderer.localClippingEnabled = true;
    this.controls = new OrbitControls(this.modelCamera, modelCanvas);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.helpers = new THREE.Group();
    this.modelScene.add(this.helpers);
    this.raycaster = new THREE.Raycaster();
    this.pointer = new THREE.Vector2();
    this.downAt = null;
    this.selectionSubset = null;
    this.selectedId = null;
    this.sectionActive = false;
    this.sectionPlane = new THREE.Plane(new THREE.Vector3(0, 0, -1), 0);
    this.modelBox = null;
    this.gizmo = this.makeGizmo();
    this.currentKey = '';
    this.loadToken = 0;
    this.model = null;
    this.highlightObjects = [];
    this.removedObjects = [];
    this.modelCanvas.addEventListener('pointerdown', event => { this.downAt = {x: event.clientX, y: event.clientY}; });
    this.modelCanvas.addEventListener('pointerup', event => this.handlePointerUp(event));
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
    if (!this.downAt || !this.model) { this.downAt = null; return; }
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
    try {
      const expressId = this.model.getExpressId(hit.object.geometry, hit.faceIndex);
      if (Number.isFinite(expressId)) this.selectElement(expressId);
    } catch (_) { /* ignore picking errors */ }
  }

  async selectElement(expressId) {
    if (!this.model) return;
    this.selectedId = expressId;
    if (this.selectionSubset && this.selectionSubset.parent) this.selectionSubset.parent.remove(this.selectionSubset);
    const material = new THREE.MeshLambertMaterial({color: 0x4b8fe3, transparent: true, opacity: 0.85, depthTest: false, side: THREE.DoubleSide});
    this.selectionSubset = this.model.createSubset({ids: [expressId], material, scene: this.modelScene, removePrevious: true, customID: 'vex-selection'});
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

  applyPlanCut() {
    if (!this.modelBox) return;
    if (this.planCutMode === 'top') {
      this.planRenderer.clippingPlanes = [];
      this.planCutZ = null;
      return;
    }
    this.planCutZ = this.planCutHeight();
    this.planClipPlane.set(new THREE.Vector3(0, 0, -1), this.planCutZ);
    this.planRenderer.clippingPlanes = [this.planClipPlane];
  }

  animate() {
    requestAnimationFrame(() => this.animate());
    this.controls.update();
    this.modelRenderer.render(this.modelScene, this.modelCamera);
    this.planRenderer.render(this.planScene, this.planCamera);
    this.renderGizmo();
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
    const token = ++this.loadToken;
    try {
      if (this.currentKey !== key) {
        this.clearSceneModels();
        this.planStatus.textContent = 'Loading IFC geometry...';
        this.modelStatus.textContent = 'Loading IFC geometry...';
        const model = await this.loadIfcModel(projectId, latestCommit);
        if (token !== this.loadToken) return;
        this.model = model;
        this.modelScene.add(this.model);
        this.currentKey = key;
        this.fitToModel(this.model);
        if (!this.modelBox) {
          if (token !== this.loadToken) return;
          this.clear('No 3D geometry found in this commit.');
          return;
        }
        await this.extractStoreys(this.model);
        if (token !== this.loadToken) return;
        this.applyPlanCut();
      }
      await this.applyDiff(changes, mode, token);
      if (token !== this.loadToken) return;
      this.planStatus.textContent = '';
      this.modelStatus.textContent = '';
      this.planMeta.textContent = mode === 'changes' ? 'changes only' : 'full model';
      this.modelMeta.textContent = mode === 'changes' ? 'changes only' : 'full model';
    } catch (error) {
      if (token !== this.loadToken) return;
      this.clear(`IFC render failed: ${error.message}`);
    }
  }

  async loadIfcModel(projectId, commit) {
    const url = `/v1/projects/${encodeURIComponent(projectId)}/ifc/${encodeURIComponent(commit)}`;
    const response = await fetch(url, {headers});
    if (!response.ok) throw new Error(`${url} -> ${response.status}`);
    const buffer = await response.arrayBuffer();
    const loader = new IFCLoader();
    loader.ifcManager.setWasmPath('/assets/viewer/web-ifc/');
    if (loader.ifcManager.applyWebIfcConfig) {
      await loader.ifcManager.applyWebIfcConfig({COORDINATE_TO_ORIGIN: true, USE_FAST_BOOLS: true});
    }
    // Yield once so the "Loading IFC geometry..." status paints before the
    // synchronous web-ifc parse takes over the main thread.
    await new Promise(resolve => setTimeout(resolve, 0));
    return await loader.parse(buffer);
  }

  clearSceneModels() {
    if (this.selectionSubset && this.selectionSubset.parent) this.selectionSubset.parent.remove(this.selectionSubset);
    this.selectionSubset = null;
    this.selectedId = null;
    const panel = document.getElementById('propsPanel');
    if (panel) panel.classList.remove('open');
    for (const object of [this.model, ...this.highlightObjects, ...this.removedObjects]) {
      if (object && object.parent) object.parent.remove(object);
    }
    this.model = null;
    this.modelBox = null;
    this.highlightObjects = [];
    this.removedObjects = [];
    this.storeys = [];
    this.planLevelIndex = 0;
    this.planCutZ = null;
    this.planRenderer.clippingPlanes = this.planCutMode === 'top' ? [] : [this.planClipPlane];
    if (typeof this.onStoreys === 'function') this.onStoreys([]);
  }

  async applyDiff(changes, mode, token) {
    if (!this.model) return;
    for (const object of [...this.highlightObjects, ...this.removedObjects]) {
      if (object && object.parent) object.parent.remove(object);
    }
    this.highlightObjects = [];
    this.removedObjects = [];
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

  fitToModel(model) {
    const box = new THREE.Box3().setFromObject(model);
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
    this.controls.update();
    this.rebuildHelpers(box);
    if (this.sectionActive) {
      const slider = document.getElementById('sectionSlider');
      this.setSection(slider ? Number(slider.value) : 100);
    }
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
    const planSize = Math.max(size.x, size.y, 1) * 0.58;
    this.planCamera.left = -planSize * aspect;
    this.planCamera.right = planSize * aspect;
    this.planCamera.top = planSize;
    this.planCamera.bottom = -planSize;
    this.planCamera.near = -radius * 10;
    this.planCamera.far = radius * 10;
    this.planCamera.position.set(center.x, center.y, center.z + radius * 2);
    this.planCamera.lookAt(center);
    this.planCamera.updateProjectionMatrix();
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
  const folderName = document.getElementById('folderName').value.trim();
  const body = {
    project_name: optionalValue('projectName'),
    folder_name: folderName,
    include: ['*.ifc'],
    ifc_project_guid: optionalValue('ifcGuid')
  };
  const root = lastSetup && (lastSetup.inbox_root_path || lastSetup.suggested_inbox_path);
  if (pickedFolderPath && root && pickedFolderPath.startsWith(root)) {
    body.local_path = pickedFolderPath;
  }
  const response = await api('/v1/setup/inbox', {method: 'POST', headers: jsonHeaders, body: JSON.stringify(body)});
  els.setupPanel.classList.remove('open');
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
  ifcViewer.onStoreys = populatePlanLevels;
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
setInterval(refresh, 15000);
setInterval(loadHealth, 60000);
</script>
</body>
</html>
"#;
