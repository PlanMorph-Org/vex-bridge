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
.app {
  height: 100%;
  display: grid;
  grid-template-rows: 48px 1fr;
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
  grid-template-columns: minmax(220px, 280px) minmax(260px, 360px) minmax(520px, 1fr);
}
.sidebar, .history, .viewer {
  min-height: 0;
  border-right: 1px solid var(--line);
  background: var(--panel);
}
.viewer { border-right: 0; display: grid; grid-template-rows: auto 1fr; }
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
.empty {
  padding: 18px;
  color: var(--muted);
}
.change-table {
  overflow: auto;
  border-top: 1px solid var(--line);
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
@media (max-width: 980px) {
  .main { grid-template-columns: 1fr; grid-template-rows: 220px 260px 1fr; }
  .sidebar, .history { border-right: 0; border-bottom: 1px solid var(--line); }
  .view-grid { grid-template-columns: 1fr; grid-template-rows: 1fr 1fr; }
}
</style>
</head>
<body>
<div class="app">
  <div class="topbar">
    <div class="status-dot" id="statusDot"></div>
    <div class="brand">Vex</div>
    <div id="topStatus" class="row-meta">Loading</div>
    <div class="toolbar-spacer"></div>
    <button id="setupButton">Add Inbox</button>
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
        <div class="badges" id="countBadges"></div>
      </div>
      <div class="view-grid">
        <div class="view-pane">
          <header><span>2D Change Plan</span><span id="planMeta"></span></header>
          <canvas id="planCanvas"></canvas>
        </div>
        <div class="view-pane">
          <header><span>3D Change View</span><span id="modelMeta"></span></header>
          <canvas id="modelCanvas"></canvas>
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
</div>
<div class="setup" id="setupPanel">
  <div class="panel-head"><div class="panel-title">Add Inbox</div><button id="closeSetup" type="button">Close</button></div>
  <form id="setupForm">
    <div class="field"><label for="projectId">Project ID</label><input id="projectId" required placeholder="prj_01HXYZ"></div>
    <div class="field"><label for="projectName">Project Name</label><input id="projectName" placeholder="Commercial Tower"></div>
    <div class="field"><label for="localPath">Inbox Path</label><input id="localPath" placeholder="~/VexInbox/Commercial-Tower"></div>
    <div class="field"><label for="ifcGuid">IFC Project GUID</label><input id="ifcGuid" placeholder="2HnQxDrSH5sBbC4NkVOGR8"></div>
    <button class="primary" type="submit">Save Inbox</button>
  </form>
</div>
<script>
const TOKEN = __VEX_TOKEN__;
const headers = {'X-Vex-Bridge-Token': TOKEN};
const jsonHeaders = {'X-Vex-Bridge-Token': TOKEN, 'Content-Type': 'application/json'};
let selectedProject = null;
let selectedCommit = null;
let projectCommits = [];
let latestChanges = null;
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
  setupPanel: document.getElementById('setupPanel'), setupForm: document.getElementById('setupForm')
};

document.getElementById('refreshButton').addEventListener('click', refresh);
document.getElementById('setupButton').addEventListener('click', () => els.setupPanel.classList.add('open'));
document.getElementById('closeSetup').addEventListener('click', () => els.setupPanel.classList.remove('open'));
els.setupForm.addEventListener('submit', saveInbox);
window.addEventListener('resize', () => drawChanges(latestChanges));

async function api(path, options = {}) {
  const response = await fetch(path, options);
  if (!response.ok) throw new Error(`${path} -> ${response.status}`);
  return await response.json();
}

async function refresh() {
  try {
    const setup = await api('/v1/setup/status', {headers});
    els.statusDot.className = `status-dot ${setup.watch.active_watchers > 0 ? 'ok' : 'warn'}`;
    els.topStatus.textContent = `${setup.watch.active_watchers}/${setup.watch.configured_projects} watching`;
    document.getElementById('localPath').placeholder = setup.suggested_inbox_path;
    renderProjects(setup.watch.projects);
    if (!selectedProject && setup.watch.projects.length) {
      const requested = setup.watch.projects.find(project => project.project_id === requestedProject);
      selectProject((requested || setup.watch.projects[0]).project_id);
    }
  } catch (error) {
    els.statusDot.className = 'status-dot warn';
    els.topStatus.textContent = error.message;
  }
}

function renderProjects(projects) {
  els.projectCount.textContent = String(projects.length);
  els.projects.innerHTML = projects.length ? '' : '<div class="empty">No inboxes configured.</div>';
  for (const project of projects) {
    const button = document.createElement('button');
    button.className = `row ${project.project_id === selectedProject ? 'active' : ''}`;
    button.innerHTML = `<div class="row-title">${escapeHtml(project.project_name || project.project_id)}</div>
      <div class="row-meta">${project.active ? 'Watching' : 'Inactive'} / ${escapeHtml(project.local_path)}</div>
      <div class="row-meta">${project.seen_import_count} caught</div>`;
    button.addEventListener('click', () => selectProject(project.project_id));
    els.projects.appendChild(button);
  }
}

async function selectProject(projectId) {
  selectedProject = projectId;
  selectedCommit = null;
  projectCommits = [];
  await loadHistory(projectId);
  const requested = projectCommits.find(commit => requestedCommit && commit.commit.startsWith(requestedCommit));
  const target = requested || projectCommits[0];
  await selectCommit(target ? target.commit : null);
  await refresh();
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
  const diff = changes.visual_diff || {};
  const summary = diff.summary || diff.status || 'No previous version to compare';
  els.changeTitle.textContent = summary;
  els.changeTime.textContent = `Caught ${formatTime(changes.caught_at_unix)} / comparing ${short(changes.previous_commit)} -> ${short(changes.latest_commit)}`;
  renderBadges(diff.counts || {});
  renderRows(diff.elements || []);
  drawChanges(changes);
}

function renderBadges(counts) {
  const kinds = ['added', 'removed', 'modified', 'moved', 'renamed'];
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
      <td>${escapeHtml(element.type_name || 'IFC element')}</td>
      <td>${escapeHtml(element.hint || idLabel(element.id) || '')}</td>`;
    els.changeRows.appendChild(tr);
  }
}

function drawChanges(changes) {
  drawPlan(els.planCanvas, changes);
  drawModel(els.modelCanvas, changes);
}

function prepareCanvas(canvas) {
  const rect = canvas.getBoundingClientRect();
  const scale = window.devicePixelRatio || 1;
  canvas.width = Math.max(1, Math.floor(rect.width * scale));
  canvas.height = Math.max(1, Math.floor(rect.height * scale));
  const ctx = canvas.getContext('2d');
  ctx.setTransform(scale, 0, 0, scale, 0, 0);
  return {ctx, width: rect.width, height: rect.height};
}

function drawPlan(canvas, changes) {
  const {ctx, width, height} = prepareCanvas(canvas);
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = '#111313'; ctx.fillRect(0, 0, width, height);
  ctx.strokeStyle = '#2f3435'; ctx.lineWidth = 1;
  for (let x = 24; x < width; x += 36) { ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, height); ctx.stroke(); }
  for (let y = 24; y < height; y += 36) { ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(width, y); ctx.stroke(); }
  const elements = ((changes && changes.visual_diff && changes.visual_diff.elements) || []).slice(0, 80);
  if (!elements.length) return drawEmpty(ctx, width, height);
  elements.forEach((el, i) => {
    const x = 38 + ((i * 53) % Math.max(60, width - 80));
    const y = 42 + ((i * 31) % Math.max(60, height - 90));
    ctx.strokeStyle = color(el.kind); ctx.fillStyle = color(el.kind, 0.18); ctx.lineWidth = 2;
    ctx.beginPath(); ctx.rect(x, y, 30 + (i % 4) * 8, 18 + (i % 3) * 7); ctx.fill(); ctx.stroke();
  });
  els.planMeta.textContent = `${elements.length} drawn`;
}

function drawModel(canvas, changes) {
  const {ctx, width, height} = prepareCanvas(canvas);
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = '#111313'; ctx.fillRect(0, 0, width, height);
  const elements = ((changes && changes.visual_diff && changes.visual_diff.elements) || []).slice(0, 64);
  if (!elements.length) return drawEmpty(ctx, width, height);
  elements.forEach((el, i) => {
    const x = width * 0.18 + ((i * 47) % Math.max(80, width * 0.64));
    const y = height * 0.22 + ((i * 29) % Math.max(80, height * 0.58));
    const z = 12 + (i % 5) * 8;
    drawBlock(ctx, x, y, 36, 24, z, color(el.kind));
  });
  els.modelMeta.textContent = 'visual diff';
}

function drawBlock(ctx, x, y, w, h, z, c) {
  ctx.fillStyle = c; ctx.globalAlpha = 0.78;
  ctx.beginPath(); ctx.rect(x, y, w, h); ctx.fill();
  ctx.globalAlpha = 0.52; ctx.beginPath(); ctx.moveTo(x, y); ctx.lineTo(x + z, y - z); ctx.lineTo(x + w + z, y - z); ctx.lineTo(x + w, y); ctx.closePath(); ctx.fill();
  ctx.globalAlpha = 0.36; ctx.beginPath(); ctx.moveTo(x + w, y); ctx.lineTo(x + w + z, y - z); ctx.lineTo(x + w + z, y + h - z); ctx.lineTo(x + w, y + h); ctx.closePath(); ctx.fill();
  ctx.globalAlpha = 1;
}

function drawEmpty(ctx, width, height) {
  ctx.fillStyle = '#747873'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
  ctx.fillText('No visual changes', width / 2, height / 2);
  els.planMeta.textContent = ''; els.modelMeta.textContent = '';
}

function color(kind, alpha) {
  const map = {added: '67,194,107', removed: '224,90,71', modified: '217,154,43', moved: '75,143,227', renamed: '155,107,211'};
  const rgb = map[kind] || '168,170,167';
  return alpha == null ? `rgb(${rgb})` : `rgba(${rgb},${alpha})`;
}

async function saveInbox(event) {
  event.preventDefault();
  const body = {
    project_id: document.getElementById('projectId').value.trim(),
    project_name: optionalValue('projectName'),
    local_path: optionalValue('localPath'),
    include: ['*.ifc'],
    ifc_project_guid: optionalValue('ifcGuid')
  };
  await api('/v1/setup/inbox', {method: 'POST', headers: jsonHeaders, body: JSON.stringify(body)});
  els.setupPanel.classList.remove('open');
  selectedProject = body.project_id;
  await refresh();
}

function optionalValue(id) { const value = document.getElementById(id).value.trim(); return value || null; }
function parentFor(commit) { return commit && commit.parents && commit.parents.length ? commit.parents[0] : null; }
function short(value) { return value ? value.slice(0, 12) : 'none'; }
function idLabel(id) { return id && (id.GlobalId || id.StepId || id.step_id); }
function formatTime(seconds) { return seconds ? new Date(seconds * 1000).toLocaleString() : 'not caught yet'; }
function escapeHtml(value) { return String(value).replace(/[&<>"']/g, ch => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[ch])); }

refresh();
setInterval(refresh, 15000);
</script>
</body>
</html>
"#;
