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
  --bg: #11171b;
  --panel: #172126;
  --panel-2: #202d34;
  --panel-raised: #26353d;
  --line: #32434c;
  --line-soft: rgba(209, 226, 232, .09);
  --text: #f1f6f7;
  --muted: #a8bbc2;
  --subtle: #71858d;
  --green: #53d48b;
  --red: #f17466;
  --amber: #efb654;
  --blue: #69b5ee;
  --violet: #b695ea;
  --accent: #4ca9e7;
  --accent-strong: #1c85c5;
  --accent-ink: #07151e;
  --viewport-bg: #202c32;
  --viewport-grid-major: #6b8a94;
  --viewport-grid-minor: #3e535b;
  --viewport-surface: #dbe6e8;
}
[data-theme="light"] {
  color-scheme: light;
  --bg: #edf2f3;
  --panel: #f7fafb;
  --panel-2: #eaf0f2;
  --panel-raised: #ffffff;
  --line: #c8d4d8;
  --line-soft: rgba(27, 56, 66, .1);
  --text: #18272d;
  --muted: #587078;
  --subtle: #7d9299;
  --green: #177e50;
  --red: #bd4034;
  --amber: #9b6110;
  --blue: #1679b8;
  --violet: #7350ab;
  --accent: #197ebc;
  --accent-strong: #08639d;
  --accent-ink: #ffffff;
  --viewport-bg: #d9e5e8;
  --viewport-grid-major: #77939b;
  --viewport-grid-minor: #acc0c5;
  --viewport-surface: #38545e;
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
  background: var(--panel-raised);
  color: var(--text);
  border-radius: 6px;
  padding: 8px 9px;
}
.field select {
  width: 100%;
  border: 1px solid var(--line);
  background: var(--panel-raised);
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

/* Workspace shell: viewport-first, with dense controls and panels that can be
   resized or collapsed without affecting the canvas height. */
html, body { overflow: hidden; }
body { background: var(--bg); }
button, select, input { accent-color: var(--accent); }
button { border-color: var(--line); background: var(--panel-2); }
button:hover:not(:disabled) { border-color: var(--accent); background: var(--panel-raised); }
button:focus-visible, select:focus-visible, input:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
button.primary { background: var(--accent); border-color: var(--accent); color: var(--accent-ink); }
button.primary:hover:not(:disabled) { background: var(--accent-strong); border-color: var(--accent-strong); }
.app {
  height: 100vh;
  height: 100dvh;
  min-height: 0;
  grid-template-rows: 42px auto minmax(0, 1fr) 26px;
  overflow: hidden;
}
.topbar {
  min-width: 0;
  gap: 8px;
  padding: 0 10px;
  background: var(--panel);
  border-bottom-color: var(--line);
}
.brand { white-space: nowrap; letter-spacing: .015em; }
.topbar .row-meta { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.topbar button { min-height: 28px; padding: 4px 9px; font-size: 12px; }
.topbar .workspace-button { color: var(--muted); background: transparent; }
.topbar .workspace-button[aria-pressed="true"] { color: var(--text); border-color: var(--accent); }
.topbar .settings-button { width: 30px; padding: 0; font-size: 16px; line-height: 1; }
.update-banner { min-height: 0; padding: 6px 12px; }
.main {
  position: relative;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  --projects-width: 218px;
  --history-width: 266px;
  --projects-gutter: 5px;
  --history-gutter: 5px;
  grid-template-columns: minmax(0, var(--projects-width)) var(--projects-gutter) minmax(0, var(--history-width)) var(--history-gutter) minmax(360px, 1fr);
}
.main.projects-collapsed { --projects-width: 0px; --projects-gutter: 0px; }
.main.history-collapsed { --history-width: 0px; --history-gutter: 0px; }
.sidebar, .history, .viewer {
  min-width: 0;
  overflow: hidden;
  background: var(--panel);
  border-right: 0;
}
.sidebar, .history { transition: opacity .16s ease, transform .16s ease; }
.main.projects-collapsed .sidebar, .main.history-collapsed .history {
  opacity: 0;
  pointer-events: none;
}
.panel-resizer {
  position: relative;
  z-index: 5;
  cursor: col-resize;
  background: var(--line);
  transition: background .14s ease;
}
.panel-resizer:hover, .panel-resizer.dragging { background: var(--accent); }
.panel-head {
  height: 38px;
  padding: 0 10px;
  background: color-mix(in srgb, var(--panel) 92%, var(--panel-raised));
}
.panel-title { font-size: 12px; letter-spacing: .035em; text-transform: uppercase; }
.panel-collapse {
  width: 24px;
  height: 24px;
  padding: 0;
  color: var(--muted);
  background: transparent;
  border-color: transparent;
  font-size: 16px;
}
.list { height: calc(100% - 38px); }
.row { padding: 9px 10px; border-bottom-color: var(--line-soft); }
.row:hover, .row.active { background: var(--panel-2); box-shadow: inset 3px 0 0 var(--accent); }
.row.active .row-title { color: var(--text); }
.row-meta { color: var(--muted); }
.project-row { border-bottom-color: var(--line-soft); }
.viewer {
  grid-template-rows: auto minmax(0, 1fr) minmax(88px, 170px);
  background: var(--panel);
}
.viewer-head {
  min-height: 62px;
  align-items: center;
  padding: 7px 12px;
  background: var(--panel);
}
.viewer-head > div:last-child {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 5px;
}
.viewer-head .badges { flex-basis: 100%; justify-content: flex-end; }
.commit-line { font-size: 14px; }
.time-line { font-size: 11px; }
.view-toggle { border-color: var(--line); }
.view-toggle button { padding: 4px 7px; font-size: 11px; }
.view-toggle button.active { background: var(--accent); color: var(--accent-ink); }
.view-grid {
  grid-template-columns: minmax(260px, .9fr) minmax(320px, 1.1fr);
  grid-template-rows: minmax(0, 1fr);
  gap: 1px;
  background: var(--line);
}
.view-grid.dim-3d, .view-grid.dim-2d { grid-template-columns: minmax(0, 1fr); }
.view-grid.dim-3d #planPane, .view-grid.dim-2d #modelPane { display: none; }
.view-pane { background: var(--viewport-bg); grid-template-rows: 34px minmax(0, 1fr); }
.view-pane header {
  min-width: 0;
  padding: 0 10px;
  color: var(--muted);
  background: var(--panel);
  border-bottom-color: var(--line-soft);
  font-size: 12px;
}
.view-pane header > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.plan-tools { min-width: 0; }
.plan-tools select, .tool-select { color: var(--text); background: var(--panel-raised); border-color: var(--line); }
.viewer-toolbar { top: 42px; left: 8px; gap: 3px; }
.tool-btn {
  padding: 4px 7px;
  color: var(--text);
  background: color-mix(in srgb, var(--panel) 90%, transparent);
  border-color: color-mix(in srgb, var(--line) 80%, transparent);
}
.tool-btn.active { background: var(--accent); border-color: var(--accent); color: var(--accent-ink); }
.gizmo { opacity: .94; }
.view-status {
  inset: 34px 0 0 0;
  z-index: 2;
  align-content: center;
  justify-content: center;
  color: var(--muted);
  background: color-mix(in srgb, var(--viewport-bg) 30%, transparent);
  font-size: 13px;
  line-height: 1.4;
}
.view-status:not(:empty) {
  display: grid;
  padding: 24px;
}
.view-status:not(:empty)::before {
  content: "";
  width: 22px;
  height: 22px;
  margin: auto;
  border: 2px solid color-mix(in srgb, var(--muted) 35%, transparent);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: vex-spin .9s linear infinite;
}
.view-status[data-state="empty"]::before { display: none; }
@keyframes vex-spin { to { transform: rotate(360deg); } }
.orbit-hint, .props-panel, .section-row {
  background: color-mix(in srgb, var(--panel) 92%, transparent);
  border-color: var(--line);
  box-shadow: 0 8px 22px rgba(0, 0, 0, .16);
}
.props-panel { top: 42px; right: 8px; }
.change-table {
  max-height: none;
  min-height: 0;
  border-top-color: var(--line);
  background: var(--panel);
}
.change-table table { table-layout: fixed; }
.change-table th, .change-table td { padding: 6px 9px; border-bottom-color: var(--line-soft); }
.change-table th { background: var(--panel); font-size: 11px; text-transform: uppercase; letter-spacing: .03em; }
.change-table th:first-child, .change-table td:first-child { width: 27%; }
.change-table th:nth-child(2), .change-table td:nth-child(2) { width: 28%; }
.change-table td { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.change-detail > td { white-space: normal; }
.badge { min-height: 20px; padding: 1px 6px; background: var(--panel-2); }
.statusbar { padding: 0 10px; background: var(--panel); border-top-color: var(--line); font-size: 11px; }
.statusbar .sb-item { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.statusbar .sb-action { padding: 1px 6px; color: var(--muted); border-color: var(--line); }
.settings-panel {
  position: fixed;
  top: 48px;
  right: 10px;
  z-index: 12;
  width: min(300px, calc(100vw - 20px));
  padding: 12px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--panel-raised);
  box-shadow: 0 18px 52px rgba(0, 0, 0, .28);
}
.settings-panel[hidden] { display: none; }
.settings-panel h2 { margin: 0 0 8px; font-size: 13px; }
.settings-panel p { margin: 0 0 10px; color: var(--muted); font-size: 12px; }
.settings-panel .field { margin-top: 10px; }
.settings-panel .field select {
  background: var(--panel-raised);
  color: var(--text);
  border-color: var(--line);
}
.settings-check {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 0;
  border-top: 1px solid var(--line-soft);
  color: var(--text);
}
.settings-check input { width: 16px; height: 16px; }
.compact-actions {
  display: none;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 6px;
  margin: 10px 0 2px;
}

@media (max-width: 1080px) {
  .topbar #topStatus { display: none; }
  .view-grid { grid-template-columns: minmax(220px, .85fr) minmax(280px, 1.15fr); }
  .main { --projects-width: 196px; --history-width: 228px; }
  .viewer-head .badges { display: none; }
}
@media (min-width: 821px) and (max-width: 1040px) {
  .view-grid { grid-template-columns: minmax(0, 1fr); }
  .view-grid:not(.dim-2d):not(.dim-3d) #planPane { display: none; }
}
@media (max-width: 820px) {
  .topbar { gap: 5px; }
  .topbar #pairButton, .topbar #setupButton, .topbar #syncButton { display: none; }
  .compact-actions { display: grid; }
  .main {
    display: block;
    overflow: hidden;
  }
  .viewer { position: absolute; inset: 0; }
  .sidebar, .history {
    position: absolute;
    z-index: 10;
    top: 0;
    bottom: 0;
    width: min(310px, calc(100vw - 52px));
    border-right: 1px solid var(--line);
    box-shadow: 18px 0 36px rgba(0, 0, 0, .22);
    transform: translateX(-105%);
    opacity: 1;
    pointer-events: auto;
  }
  .sidebar.mobile-open, .history.mobile-open { transform: translateX(0); }
  .panel-resizer { display: none; }
  .main.projects-collapsed .sidebar, .main.history-collapsed .history {
    opacity: 1;
    pointer-events: auto;
  }
  .view-grid { grid-template-columns: minmax(0, 1fr); }
  .view-grid:not(.dim-2d):not(.dim-3d) #planPane { display: none; }
  .viewer { grid-template-rows: auto minmax(0, 1fr) minmax(84px, 145px); }
  .viewer-head { min-height: 54px; }
  .viewer-head .badges, .time-line { display: none; }
  .view-toggle button { padding: 3px 5px; }
  .statusbar #sbWatch, .statusbar #sbVersions, .statusbar #sbAccountItem { display: none; }
}
@media (max-width: 540px) {
  .brand { font-size: 12px; }
  .topbar .workspace-button { width: 28px; padding: 0; font-size: 0; }
  .topbar .workspace-button::before { font-size: 14px; }
  #projectsPanelButton::before { content: "☰"; }
  #historyPanelButton::before { content: "◷"; }
  .topbar #refreshButton { font-size: 0; width: 29px; padding: 0; }
  .topbar #refreshButton::before { content: "↻"; font-size: 16px; }
  .viewer-head { display: block; }
  .viewer-head > div:last-child { justify-content: flex-start; margin-top: 5px; }
  #addIfcButton { display: none; }
  .viewer { grid-template-rows: 82px minmax(0, 1fr) minmax(76px, 130px); }
  .change-table th:nth-child(2), .change-table td:nth-child(2) { display: none; }
  .change-table th:first-child, .change-table td:first-child { width: 36%; }
  .statusbar { gap: 8px; }
  .statusbar #sbActivity { display: none; }
}
</style>
</head>
<body>
<div class="app">
  <div class="topbar">
    <div class="status-dot" id="statusDot"></div>
    <div class="brand">Vex Atlas</div>
    <div id="topStatus" class="row-meta">Loading</div>
    <button class="workspace-button" id="projectsPanelButton" type="button" aria-controls="projectsPanel" aria-pressed="true" title="Show or hide projects">Projects</button>
    <button class="workspace-button" id="historyPanelButton" type="button" aria-controls="historyPanel" aria-pressed="true" title="Show or hide commit history">History</button>
    <div class="toolbar-spacer"></div>
    <button id="pairButton">Pair Device</button>
    <button id="setupButton">Add Inbox</button>
    <button id="syncButton" title="Push committed changes to the cloud">Push</button>
    <button class="primary" id="refreshButton">Refresh</button>
    <button class="settings-button" id="settingsButton" type="button" aria-controls="settingsPanel" aria-expanded="false" title="Workspace display settings">⚙</button>
  </div>
  <div class="update-banner" id="updateBanner" style="display:none"></div>
  <main class="main">
    <section class="sidebar" id="projectsPanel">
      <div class="panel-head"><div class="panel-title">Projects</div><div id="projectCount" class="row-meta"></div><button class="panel-collapse" type="button" data-panel="projects" aria-label="Collapse projects panel" title="Collapse projects">‹</button></div>
      <div id="projects" class="list"></div>
    </section>
    <div class="panel-resizer" id="projectsResizer" aria-hidden="true"></div>
    <section class="history" id="historyPanel">
      <div class="panel-head"><div class="panel-title">Commit History</div><div id="historyMeta" class="row-meta"></div><button class="panel-collapse" type="button" data-panel="history" aria-label="Collapse commit history panel" title="Collapse history">‹</button></div>
      <div id="history" class="list"></div>
    </section>
    <div class="panel-resizer" id="historyResizer" aria-hidden="true"></div>
    <section class="viewer">
      <div class="viewer-head">
        <div>
          <div id="changeTitle" class="commit-line">No project selected</div>
          <div id="changeTime" class="time-line"></div>
        </div>
        <div>
          <div class="view-toggle" id="dimToggle">
            <button type="button" data-dim="split" class="active" title="Show plan and model">Split</button>
            <button type="button" data-dim="3d">3D</button>
            <button type="button" data-dim="2d">2D</button>
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
      <div class="view-grid" id="viewGrid">
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
          <div class="view-status" id="planStatus" role="status" aria-live="polite"></div>
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
          <div class="view-status" id="modelStatus" role="status" aria-live="polite"></div>
        </div>
      </div>
      <div class="change-table">
        <table aria-label="Element changes">
          <thead><tr><th scope="col">Kind</th><th scope="col">Element</th><th scope="col">Change</th></tr></thead>
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
<section class="settings-panel" id="settingsPanel" aria-labelledby="settingsTitle" hidden>
  <h2 id="settingsTitle">Workspace display</h2>
  <p>Saved on this device. Rendering changes apply immediately.</p>
  <div class="compact-actions" aria-label="Project actions">
    <button id="compactPairButton" type="button">Pair</button>
    <button id="compactSetupButton" type="button">Inbox</button>
    <button id="compactSyncButton" type="button">Push</button>
  </div>
  <div class="field"><label for="themePreference">Theme</label><select id="themePreference" data-preference="theme" aria-describedby="themeHelp"><option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option></select><span class="row-meta" id="themeHelp">Matches your system unless overridden.</span></div>
  <label class="settings-check" for="gridPreference"><span>Reference grid</span><input id="gridPreference" data-preference="showGrid" type="checkbox"></label>
  <label class="settings-check" for="axesPreference"><span>Origin axes</span><input id="axesPreference" data-preference="showAxes" type="checkbox"></label>
  <div class="field"><label for="shadowPreference">Shadows</label><select id="shadowPreference" data-preference="shadows"><option value="auto">Auto (safe models only)</option><option value="off">Off</option><option value="on">On</option></select></div>
</section>
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
const WORKSPACE_PREFERENCES_KEY = 'vexWorkspacePreferences';
const DEFAULT_WORKSPACE_PREFERENCES = {
  theme: 'system',
  showGrid: true,
  showAxes: false,
  shadows: 'auto',
  projectsWidth: 218,
  historyWidth: 266,
  projectsCollapsed: false,
  historyCollapsed: false
};
const workspacePreferences = loadWorkspacePreferences();

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
  updateBanner: document.getElementById('updateBanner'),
  workspace: document.querySelector('.main'),
  projectsPanel: document.getElementById('projectsPanel'), historyPanel: document.getElementById('historyPanel'),
  projectsPanelButton: document.getElementById('projectsPanelButton'), historyPanelButton: document.getElementById('historyPanelButton'),
  projectsResizer: document.getElementById('projectsResizer'), historyResizer: document.getElementById('historyResizer'),
  settingsButton: document.getElementById('settingsButton'), settingsPanel: document.getElementById('settingsPanel'),
  themePreference: document.getElementById('themePreference'), gridPreference: document.getElementById('gridPreference'),
  axesPreference: document.getElementById('axesPreference'), shadowPreference: document.getElementById('shadowPreference'),
  compactSyncButton: document.getElementById('compactSyncButton')
};

// Native desktop bridge (present only inside the vex-desktop window). Falls back
// gracefully to plain browser behaviour when unavailable.
const native = (typeof window !== 'undefined' && window.__vexNative && window.__vexNative.available)
  ? window.__vexNative : null;
let pickedFolderPath = null;
let healthInfo = null;
let updateInfo = null;

let ifcViewer = null;

function loadWorkspacePreferences() {
  try {
    const saved = JSON.parse(localStorage.getItem(WORKSPACE_PREFERENCES_KEY) || '{}');
    const merged = {...DEFAULT_WORKSPACE_PREFERENCES, ...(saved && typeof saved === 'object' ? saved : {})};
    merged.theme = ['system', 'light', 'dark'].includes(merged.theme) ? merged.theme : 'system';
    merged.shadows = ['auto', 'on', 'off'].includes(merged.shadows) ? merged.shadows : 'auto';
    merged.showGrid = merged.showGrid !== false;
    merged.showAxes = merged.showAxes === true;
    merged.projectsWidth = Math.max(168, Math.min(420, Number(merged.projectsWidth) || DEFAULT_WORKSPACE_PREFERENCES.projectsWidth));
    merged.historyWidth = Math.max(190, Math.min(460, Number(merged.historyWidth) || DEFAULT_WORKSPACE_PREFERENCES.historyWidth));
    merged.projectsCollapsed = merged.projectsCollapsed === true;
    merged.historyCollapsed = merged.historyCollapsed === true;
    return merged;
  } catch (_) {
    return {...DEFAULT_WORKSPACE_PREFERENCES};
  }
}

function saveWorkspacePreferences() {
  try { localStorage.setItem(WORKSPACE_PREFERENCES_KEY, JSON.stringify(workspacePreferences)); } catch (_) {}
}

function isCompactWorkspace() {
  return window.matchMedia && window.matchMedia('(max-width: 820px)').matches;
}

function resolvedTheme() {
  if (workspacePreferences.theme !== 'system') return workspacePreferences.theme;
  return window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

function updateWorkspaceControls() {
  const compact = isCompactWorkspace();
  if (els.themePreference) els.themePreference.value = workspacePreferences.theme;
  if (els.gridPreference) els.gridPreference.checked = workspacePreferences.showGrid;
  if (els.axesPreference) els.axesPreference.checked = workspacePreferences.showAxes;
  if (els.shadowPreference) els.shadowPreference.value = workspacePreferences.shadows;
  if (!els.workspace) return;
  els.workspace.style.setProperty('--projects-width', `${workspacePreferences.projectsWidth}px`);
  els.workspace.style.setProperty('--history-width', `${workspacePreferences.historyWidth}px`);
  els.workspace.classList.toggle('projects-collapsed', !compact && workspacePreferences.projectsCollapsed);
  els.workspace.classList.toggle('history-collapsed', !compact && workspacePreferences.historyCollapsed);
  if (els.projectsPanelButton) {
    els.projectsPanelButton.setAttribute('aria-pressed', compact
      ? String(els.projectsPanel && els.projectsPanel.classList.contains('mobile-open'))
      : String(!workspacePreferences.projectsCollapsed));
  }
  if (els.historyPanelButton) {
    els.historyPanelButton.setAttribute('aria-pressed', compact
      ? String(els.historyPanel && els.historyPanel.classList.contains('mobile-open'))
      : String(!workspacePreferences.historyCollapsed));
  }
}

function applyWorkspacePreferences() {
  document.documentElement.dataset.theme = resolvedTheme();
  updateWorkspaceControls();
  if (ifcViewer) ifcViewer.refreshAppearance();
}

function setPanelCollapsed(panel, collapsed) {
  const compact = isCompactWorkspace();
  const element = panel === 'projects' ? els.projectsPanel : els.historyPanel;
  if (compact) {
    if (element) element.classList.toggle('mobile-open', !collapsed);
  } else if (panel === 'projects') {
    workspacePreferences.projectsCollapsed = collapsed;
    if (els.projectsPanel) els.projectsPanel.classList.remove('mobile-open');
    saveWorkspacePreferences();
  } else {
    workspacePreferences.historyCollapsed = collapsed;
    if (els.historyPanel) els.historyPanel.classList.remove('mobile-open');
    saveWorkspacePreferences();
  }
  updateWorkspaceControls();
  if (ifcViewer) requestAnimationFrame(() => ifcViewer.resize());
}

function toggleWorkspacePanel(panel) {
  const element = panel === 'projects' ? els.projectsPanel : els.historyPanel;
  if (isCompactWorkspace()) {
    const opening = !(element && element.classList.contains('mobile-open'));
    if (els.projectsPanel && panel !== 'projects') els.projectsPanel.classList.remove('mobile-open');
    if (els.historyPanel && panel !== 'history') els.historyPanel.classList.remove('mobile-open');
    setPanelCollapsed(panel, !opening);
    return;
  }
  const collapsed = panel === 'projects' ? workspacePreferences.projectsCollapsed : workspacePreferences.historyCollapsed;
  setPanelCollapsed(panel, !collapsed);
}

function enablePanelResize(handle, panel) {
  if (!handle) return;
  handle.addEventListener('pointerdown', event => {
    if (isCompactWorkspace()) return;
    const collapsed = panel === 'projects' ? workspacePreferences.projectsCollapsed : workspacePreferences.historyCollapsed;
    if (collapsed || event.button !== 0) return;
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = panel === 'projects' ? workspacePreferences.projectsWidth : workspacePreferences.historyWidth;
    const min = panel === 'projects' ? 168 : 190;
    const max = panel === 'projects' ? 420 : 460;
    handle.classList.add('dragging');
    const move = moveEvent => {
      const next = Math.max(min, Math.min(max, startWidth + moveEvent.clientX - startX));
      if (panel === 'projects') workspacePreferences.projectsWidth = next;
      else workspacePreferences.historyWidth = next;
      updateWorkspaceControls();
      if (ifcViewer) ifcViewer.resize();
    };
    const stop = () => {
      handle.classList.remove('dragging');
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', stop);
      saveWorkspacePreferences();
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', stop, {once: true});
  });
}

function toggleSettings(force) {
  if (!els.settingsPanel || !els.settingsButton) return;
  const open = typeof force === 'boolean' ? force : els.settingsPanel.hidden;
  els.settingsPanel.hidden = !open;
  els.settingsButton.setAttribute('aria-expanded', String(open));
  if (open) els.themePreference.focus();
}

applyWorkspacePreferences();
enablePanelResize(els.projectsResizer, 'projects');
enablePanelResize(els.historyResizer, 'history');

document.getElementById('refreshButton').addEventListener('click', refresh);
els.projectsPanelButton.addEventListener('click', () => toggleWorkspacePanel('projects'));
els.historyPanelButton.addEventListener('click', () => toggleWorkspacePanel('history'));
document.querySelectorAll('.panel-collapse').forEach(button => button.addEventListener('click', () => {
  const panel = button.dataset.panel;
  if (panel) setPanelCollapsed(panel, true);
}));
els.settingsButton.addEventListener('click', () => toggleSettings());
els.settingsPanel.addEventListener('change', event => {
  const key = event.target && event.target.dataset && event.target.dataset.preference;
  if (!key) return;
  workspacePreferences[key] = event.target.type === 'checkbox' ? event.target.checked : event.target.value;
  saveWorkspacePreferences();
  applyWorkspacePreferences();
});
document.addEventListener('pointerdown', event => {
  if (!els.settingsPanel.hidden && !els.settingsPanel.contains(event.target) && event.target !== els.settingsButton) toggleSettings(false);
});
window.addEventListener('resize', () => {
  if (els.projectsPanel) els.projectsPanel.classList.remove('mobile-open');
  if (els.historyPanel) els.historyPanel.classList.remove('mobile-open');
  updateWorkspaceControls();
});
if (window.matchMedia) {
  const systemTheme = window.matchMedia('(prefers-color-scheme: light)');
  const syncSystemTheme = () => {
    if (workspacePreferences.theme === 'system') applyWorkspacePreferences();
  };
  if (systemTheme.addEventListener) systemTheme.addEventListener('change', syncSystemTheme);
  else if (systemTheme.addListener) systemTheme.addListener(syncSystemTheme);
}
document.getElementById('setupButton').addEventListener('click', () => {
  if (!els.projectId.value.trim()) els.projectId.value = genProjectId();
  els.setupPanel.classList.add('open');
});
els.pairButton.addEventListener('click', startOrPollPairing);
els.syncButton.addEventListener('click', openPushPanel);
document.getElementById('compactPairButton').addEventListener('click', () => {
  toggleSettings(false);
  startOrPollPairing();
});
document.getElementById('compactSetupButton').addEventListener('click', () => {
  toggleSettings(false);
  document.getElementById('setupButton').click();
});
document.getElementById('compactSyncButton').addEventListener('click', () => {
  toggleSettings(false);
  if (!els.syncButton.disabled) openPushPanel();
});
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
  const wanted = ['2d', '3d', 'split'].includes(dim) ? dim : 'split';
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
  if (!selectedProject || els.syncButton.disabled) return;
  els.pushPanel.classList.add('open');
  els.confirmPush.disabled = true;
  cloudProjects = [];
  els.cloudProject.innerHTML = '<option value="">Loading projects…</option>';
  els.pushPreview.innerHTML = '<div class="row-meta">Loading preview…</div>';
  try {
    const projects = await api('/v1/cloud/projects', {headers});
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
    els.pushPanel.dataset.changes = JSON.stringify({});
    renderPushPreview();
    try {
      const changes = await api(`/v1/projects/${encodeURIComponent(selectedProject)}/changes`, {headers});
      els.pushPanel.dataset.changes = JSON.stringify(changes || {});
      renderPushPreview();
    } catch (error) {
      els.pushPreview.insertAdjacentHTML('beforeend',
        `<div class="preview-warning">Could not load the optional change preview: ${escapeHtml(error.message)}. You can still push.</div>`);
    }
  } catch (error) {
    cloudProjects = [];
    els.cloudProject.innerHTML = '<option value="">Cloud projects unavailable</option>';
    els.confirmPush.disabled = true;
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
  if (els.compactSyncButton) {
    els.compactSyncButton.textContent = pending > 0 ? `Push (${pending})` : 'Push';
    els.compactSyncButton.disabled = els.syncButton.disabled;
    els.compactSyncButton.title = els.syncButton.title;
  }
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
    this.shadowsEnabled = false;
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
    this.planRenderer = this.makeRenderer(planCanvas, false);
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
    this.modelRenderer = this.makeRenderer(modelCanvas, true);
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
    this.controls.addEventListener('change', () => this.scheduleArtifactTiles());
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
    this.artifactSemanticIndexes = new Map();
    // Resident tile scenes are the single source of truth for the loaded-tile
    // count; deriving progress from this map keeps the UI honest even after
    // eviction, so there is no separate counter that can drift.
    this.artifactTileQueue = [];
    this.artifactTileDescriptors = new Map();
    this.artifactTileRecords = new Map();
    // Tiles with an in-flight fetch/decode, keyed by tile_id, so the scheduler
    // never dispatches the same tile twice.
    this.artifactTileLoading = new Set();
    this.artifactTileLoads = 0;
    this.artifactResidentBytes = 0;
    // Storey-first tiling isolates a storey by tile ownership rather than clip
    // planes (so tall/multi-storey elements stay whole). The base/coarse tile
    // and unassigned tiles are never hidden by storey isolation.
    this.artifactStoreyTileIds = new Set();
    this.artifactBaseTileId = null;
    // Multi-LOD grouping: several tiles (an exact LOD0 tile and an optional
    // coarser proxy) can describe the same logical content (a storey). This
    // map keys each group id to its {exact, coarse} tile descriptors so the
    // viewer shows exactly one LOD per group and can upgrade coarse -> exact.
    this.artifactGroups = new Map();
    // The group whose element is currently selected. A selection on a coarse
    // proxy sets this so the exact LOD0 tile is fetched and the precise
    // selection is finalised against LOD0 geometry (a coarse proxy is never
    // treated as the exact selection).
    this.selectedArtifactGroupId = null;
    this.pendingArtifactSelection = null;
    // A group is upgraded from its coarse proxy to exact LOD0 once it subtends
    // at least this fraction of the view (radius / camera distance).
    this.artifactExactLodRatio = 0.35;
    // Keep artifact streaming responsive on integrated GPUs. Tile descriptors
    // carry compressed byte sizes, so this is a deliberately conservative
    // proxy for decoded GPU allocation rather than a false-precision reading.
    this.artifactMemoryBudgetBytes = 512 * 1024 * 1024;
    this.artifactMaxInflight = 3;
    this.artifactSchedulePending = false;
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
    this.configureRenderer(renderer, false);
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

  colorFromWorkspace(name, fallback) {
    const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    try { return new THREE.Color(value || fallback); } catch (_) { return new THREE.Color(fallback); }
  }

  gridStep(span) {
    const ideal = Math.max(span / 7, 0.0001);
    const exponent = Math.pow(10, Math.floor(Math.log10(ideal)));
    const fraction = ideal / exponent;
    const nice = fraction <= 1 ? 1 : fraction <= 2 ? 2 : fraction <= 5 ? 5 : 10;
    return nice * exponent;
  }

  rebuildHelpers(box) {
    while (this.helpers.children.length) {
      const child = this.helpers.children.pop();
      if (child.geometry) child.geometry.dispose();
      const materials = Array.isArray(child.material) ? child.material : child.material ? [child.material] : [];
      for (const material of materials) material.dispose();
    }
    const size = box.getSize(new THREE.Vector3());
    const center = box.getCenter(new THREE.Vector3());
    const span = Math.max(size.x, size.y, 1);
    const lightExtent = Math.max(span, size.z, 1);
    if (this.lighting) {
      const {key, fill} = this.lighting;
      key.position.copy(center).addScaledVector(new THREE.Vector3(0.9, -0.8, 1.6).normalize(), lightExtent * 2.2);
      key.target.position.copy(center);
      key.shadow.camera.left = -lightExtent;
      key.shadow.camera.right = lightExtent;
      key.shadow.camera.top = lightExtent;
      key.shadow.camera.bottom = -lightExtent;
      key.shadow.camera.far = lightExtent * 6;
      key.shadow.camera.updateProjectionMatrix();
      fill.position.copy(center).addScaledVector(new THREE.Vector3(-0.8, 0.7, 1.0).normalize(), lightExtent * 1.8);
      fill.target.position.copy(center);
    }
    const step = this.gridStep(span);
    const gridSize = step * 10;
    const groundZ = box.min.z - Math.max(span * 0.0005, 0.002);
    const shadowFloor = new THREE.Mesh(
      new THREE.PlaneGeometry(gridSize, gridSize),
      new THREE.ShadowMaterial({color: 0x000000, opacity: 0.16, transparent: true})
    );
    shadowFloor.position.set(center.x, center.y, groundZ);
    shadowFloor.receiveShadow = true;
    shadowFloor.visible = this.shadowsEnabled === true;
    shadowFloor.userData.vexShadowFloor = true;
    this.helpers.add(shadowFloor);
    if (workspacePreferences.showGrid) {
      const grid = new THREE.GridHelper(
        gridSize,
        10,
        this.colorFromWorkspace('--viewport-grid-major', '#6b8a94'),
        this.colorFromWorkspace('--viewport-grid-minor', '#3e535b')
      );
      grid.rotation.x = Math.PI / 2;
      grid.position.set(center.x, center.y, box.min.z);
      const materials = Array.isArray(grid.material) ? grid.material : [grid.material];
      for (const material of materials) {
        material.transparent = true;
        material.opacity = document.documentElement.dataset.theme === 'light' ? 0.42 : 0.36;
        material.depthWrite = false;
      }
      grid.userData.vexGrid = true;
      this.helpers.add(grid);
    }
    if (workspacePreferences.showAxes) {
      const axes = new THREE.AxesHelper(Math.max(step * 1.5, span * 0.15));
      axes.position.set(box.min.x, box.min.y, box.min.z);
      axes.userData.vexAxes = true;
      this.helpers.add(axes);
    }
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
    let tileId = null;
    for (let object = hit.object; object; object = object.parent) {
      if (object.userData && object.userData.vexTileId) {
        tileId = object.userData.vexTileId;
        break;
      }
    }
    // v2 keeps a semantic index per tile: selection MUST resolve against the
    // index of the tile that was actually hit. Falling back to another tile's
    // index (or a stale global one) would map the hit triangle to the wrong
    // element, so if the hit tile's index is not resident yet we simply do not
    // select. v1 has a single manifest-global index that every tile shares.
    let index;
    if (this.artifactUsesTileLocalIndex()) {
      if (!tileId) return;
      index = this.artifactSemanticIndexes.get(tileId);
      if (!index) return;
    } else {
      index = this.artifactSemanticIndex;
    }
    if (!index || !Array.isArray(index.entries)) return;
    if (tileId) {
      const record = this.artifactTileRecords.get(tileId);
      if (record) record.lastUsedAt = performance.now();
    }
    const entry = index.entries.find(candidate => (candidate.triangle_ranges || []).some(range =>
      Number.isFinite(range.first_triangle) && Number.isFinite(range.triangle_count)
      && hit.faceIndex >= range.first_triangle
      && hit.faceIndex < range.first_triangle + range.triangle_count
    ));
    if (!entry) return;

    this.clearSelection();
    this.selectedId = Number.isFinite(entry.express_id) ? entry.express_id : null;
    const tile = tileId ? this.artifactTileDescriptors.get(tileId) : null;
    const groupId = tile ? this.artifactTileGroupId(tile) : (tileId || null);
    this.selectedArtifactGroupId = groupId;
    const group = groupId ? this.artifactGroups.get(groupId) : null;
    const isCoarseHit = tile ? Number(tile.lod) > 0 : false;

    if (isCoarseHit && group && group.exact) {
      // A coarse proxy is never treated as the exact selection. Record the
      // element identity (the coarse index still carries express_id/global_id),
      // upgrade the group to its exact LOD0 tile, and finalise the precise
      // selection overlay against LOD0 geometry once it is resident.
      this.pendingArtifactSelection = {
        groupId,
        expressId: Number.isFinite(entry.express_id) ? entry.express_id : null,
        globalId: entry.global_id || null,
      };
      this.showProperties(
        entry.global_id ? {GlobalId: {value: entry.global_id}} : null,
        Number.isFinite(entry.express_id) ? entry.express_id : 'artifact',
      );
      // The exact tile may already be resident (for example hidden under
      // isolation); finalise now if so, otherwise schedule its fetch/reveal.
      const selectionFinalized = this.maybeFinalizeArtifactSelection();
      this.refreshArtifactVisibility();
      if (!selectionFinalized) this.scheduleArtifactTiles();
      return;
    }

    // Exact LOD0 hit: build the precise selection overlay straight from the
    // hit geometry.
    const range = (entry.triangle_ranges || []).find(candidate =>
      hit.faceIndex >= candidate.first_triangle
      && hit.faceIndex < candidate.first_triangle + candidate.triangle_count
    );
    this.buildArtifactSelectionOverlay(hit.object.geometry, range ? [range] : []);
    this.showProperties(
      entry.global_id ? {GlobalId: {value: entry.global_id}} : null,
      Number.isFinite(entry.express_id) ? entry.express_id : 'artifact',
    );
  }

  // Find the first indexed mesh inside a resident tile scene so a selection
  // overlay can be rebuilt from that tile's geometry.
  findArtifactTileMesh(object) {
    if (!object) return null;
    let found = null;
    object.traverse(item => {
      if (found) return;
      if (item.isMesh && item.geometry && item.geometry.getIndex && item.geometry.getIndex()) found = item;
    });
    return found;
  }

  // Build a compact standalone selection overlay from a source geometry's
  // indexed triangle ranges. Standalone so disposing a selection never
  // disposes the GPU buffers shared by its source tile.
  buildArtifactSelectionOverlay(sourceGeometry, ranges) {
    const sourceIndex = sourceGeometry && sourceGeometry.getIndex && sourceGeometry.getIndex();
    const sourcePositions = sourceGeometry && sourceGeometry.getAttribute && sourceGeometry.getAttribute('position');
    if (this.selectionSubset && this.selectionSubset.parent) this.selectionSubset.parent.remove(this.selectionSubset);
    if (this.selectionSubset && this.selectionSubset.userData.vexArtifactSelection) {
      this.selectionSubset.geometry.dispose();
      this.selectionSubset.material.dispose();
    }
    this.selectionSubset = null;
    if (!sourceIndex || !sourcePositions || !Array.isArray(ranges) || !ranges.length) return;
    let total = 0;
    for (const range of ranges) {
      if (Number.isFinite(range.first_triangle) && Number.isFinite(range.triangle_count)) {
        total += range.triangle_count * 3;
      }
    }
    if (!total) return;
    const positions = new Float32Array(total * 3);
    let cursor = 0;
    for (const range of ranges) {
      if (!Number.isFinite(range.first_triangle) || !Number.isFinite(range.triangle_count)) continue;
      const start = range.first_triangle * 3;
      const count = range.triangle_count * 3;
      for (let offset = 0; offset < count; offset += 1) {
        const vertex = sourceIndex.array[start + offset];
        positions[cursor * 3] = sourcePositions.getX(vertex);
        positions[cursor * 3 + 1] = sourcePositions.getY(vertex);
        positions[cursor * 3 + 2] = sourcePositions.getZ(vertex);
        cursor += 1;
      }
    }
    const selectionGeometry = new THREE.BufferGeometry();
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

  // Complete a pending coarse-selection upgrade once the group's exact LOD0
  // tile and index are resident, rebuilding the precise overlay from LOD0.
  // Returns true when the pending selection is resolved (finalised or found to
  // have no exact tile/entry), false while still waiting for LOD0 to arrive.
  maybeFinalizeArtifactSelection() {
    const pending = this.pendingArtifactSelection;
    if (!pending) return false;
    const group = this.artifactGroups.get(pending.groupId);
    const exactTile = group && group.exact;
    if (!exactTile) { this.pendingArtifactSelection = null; return true; }
    const record = this.artifactTileRecords.get(exactTile.tile_id);
    const exactIndex = this.artifactUsesTileLocalIndex()
      ? this.artifactSemanticIndexes.get(exactTile.tile_id)
      : this.artifactSemanticIndex;
    if (!record || !record.scene || !exactIndex || !Array.isArray(exactIndex.entries)) return false;
    const entry = exactIndex.entries.find(candidate =>
      (pending.expressId !== null && candidate.express_id === pending.expressId)
      || (pending.globalId && candidate.global_id === pending.globalId)
    );
    if (!entry) { this.pendingArtifactSelection = null; return true; }
    const mesh = this.findArtifactTileMesh(record.scene);
    if (!mesh) { this.pendingArtifactSelection = null; return true; }
    record.lastUsedAt = performance.now();
    this.buildArtifactSelectionOverlay(mesh.geometry, entry.triangle_ranges || []);
    this.pendingArtifactSelection = null;
    return true;
  }

  async selectElement(expressId) {
    if (!this.model || this.modelKind !== 'ifc') return;
    this.selectedId = expressId;
    if (this.selectionSubset && this.selectionSubset.parent) this.selectionSubset.parent.remove(this.selectionSubset);
    const material = new THREE.MeshStandardMaterial({color: 0x4b8fe3, transparent: true, opacity: 0.85, depthTest: false, side: THREE.DoubleSide, roughness: 0.42, metalness: 0});
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
    this.selectedArtifactGroupId = null;
    this.pendingArtifactSelection = null;
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
    const diameter = Math.max(size.x, size.y, size.z, 1);
    const halfExtent = diameter * 0.5;
    const fov = THREE.MathUtils.degToRad(this.modelPersp.fov);
    const d = Math.max(diameter * 1.5, (halfExtent / Math.tan(fov / 2)) * 1.22);
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
    // In artifact mode storey isolation is by tile ownership, not clip planes.
    // Turning Section on cleared any active storey (above); re-apply ownership
    // so every tile hidden by a prior isolation becomes visible again, and
    // turning Section off restores the still-selected storey's isolation.
    if (this.modelKind === 'artifact') this.applyArtifactStoreyIsolation();
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
    if (this.modelKind === 'artifact') {
      this.applyArtifactStoreyIsolation();
      return;
    }
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

  configureRenderer(renderer, shadows) {
    if ('outputColorSpace' in renderer && THREE.SRGBColorSpace) {
      renderer.outputColorSpace = THREE.SRGBColorSpace;
    } else if ('outputEncoding' in renderer && THREE.sRGBEncoding) {
      renderer.outputEncoding = THREE.sRGBEncoding;
    }
    if (THREE.ACESFilmicToneMapping !== undefined) {
      renderer.toneMapping = THREE.ACESFilmicToneMapping;
      renderer.toneMappingExposure = document.documentElement.dataset.theme === 'light' ? 0.95 : 1.08;
    }
    renderer.shadowMap.enabled = Boolean(shadows && this.shadowsEnabled);
    if (renderer.shadowMap.enabled) renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  }

  makeRenderer(canvas, shadows) {
    const renderer = new THREE.WebGLRenderer({canvas, antialias: true, alpha: false});
    this.configureRenderer(renderer, shadows);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.setClearColor(this.colorFromWorkspace('--viewport-bg', '#202c32'), 1);
    return renderer;
  }

  makeScene() {
    const scene = new THREE.Scene();
    scene.background = this.colorFromWorkspace('--viewport-bg', '#202c32');
    const hemisphere = new THREE.HemisphereLight(0xe8f4ff, 0x52636b, 1.25);
    const key = new THREE.DirectionalLight(0xfff5e8, 1.65);
    key.position.set(45, -35, 72);
    key.castShadow = false;
    key.shadow.mapSize.set(1024, 1024);
    key.shadow.camera.near = 0.1;
    key.shadow.camera.far = 5000;
    const fill = new THREE.DirectionalLight(0xb8d8ff, 0.55);
    fill.position.set(-38, 28, 36);
    this.lighting = {hemisphere, key, fill};
    scene.add(hemisphere, key, key.target, fill, fill.target);
    return scene;
  }

  refreshAppearance() {
    const background = this.colorFromWorkspace('--viewport-bg', '#202c32');
    this.modelScene.background = background;
    for (const renderer of [this.modelRenderer, this.planRenderer]) {
      if (!renderer) continue;
      this.configureRenderer(renderer, renderer === this.modelRenderer);
      renderer.setClearColor(background, 1);
    }
    if (this.lighting) {
      const lightTheme = document.documentElement.dataset.theme === 'light';
      this.lighting.hemisphere.intensity = lightTheme ? 1.05 : 1.25;
      this.lighting.key.intensity = lightTheme ? 1.42 : 1.65;
      this.lighting.fill.intensity = lightTheme ? 0.46 : 0.55;
    }
    this.updateShadowPolicy();
    if (this.modelBox) this.rebuildHelpers(this.modelBox);
  }

  applyMaterialQuality(object) {
    if (!object) return;
    object.traverse(item => {
      const materials = Array.isArray(item.material) ? item.material : item.material ? [item.material] : [];
      for (const material of materials) {
        // Keep the color supplied by IFC or GLB; only improve how that color is
        // lit and tone-mapped in this workspace.
        material.toneMapped = !material.isMeshBasicMaterial;
        if ('roughness' in material && Number.isFinite(material.roughness)) material.roughness = Math.max(0.35, material.roughness);
        if ('metalness' in material && Number.isFinite(material.metalness)) material.metalness = Math.min(0.35, material.metalness);
        material.needsUpdate = true;
      }
    });
  }

  updateShadowPolicy(model = this.model) {
    let meshCount = 0;
    if (model) model.traverse(item => { if (item.isMesh) ++meshCount; });
    const cores = Number(navigator.hardwareConcurrency || 4);
    const safeForShadows = meshCount > 0 && meshCount <= 1200 && cores >= 4;
    const enabled = workspacePreferences.shadows === 'on'
      ? meshCount > 0 && meshCount <= 2400
      : workspacePreferences.shadows === 'auto' && safeForShadows;
    this.shadowsEnabled = enabled;
    if (this.modelRenderer) {
      this.modelRenderer.shadowMap.enabled = enabled;
      if (enabled) {
        this.modelRenderer.shadowMap.type = THREE.PCFSoftShadowMap;
        this.modelRenderer.shadowMap.needsUpdate = true;
      }
    }
    if (this.lighting) this.lighting.key.castShadow = enabled;
    if (model) {
      model.traverse(item => {
        if (!item.isMesh) return;
        item.castShadow = enabled;
        item.receiveShadow = false;
      });
    }
    this.helpers && this.helpers.traverse(item => {
      if (item.userData && item.userData.vexShadowFloor) item.visible = enabled;
    });
  }

  prepareModelForRender(model) {
    this.applyMaterialQuality(model);
    this.updateShadowPolicy(model);
  }

  clear(message = '') {
    this.abortPendingLoads();
    ++this.loadToken;
    this.clearSceneModels();
    this.currentKey = '';
    this.setViewStatus(message, 'empty');
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
          this.registerArtifactTiles(artifact.tiles);
          this.artifactTileRecords.clear();
          this.artifactTileLoading.clear();
          this.artifactResidentBytes = 0;
          this.artifactBaseTileId = (artifact.firstTile && artifact.firstTile.tile_id) || null;
          if (artifact.firstTile && artifact.firstTile.tile_id) {
            const bytes = Number(artifact.firstTile.artifact && artifact.firstTile.artifact.byte_length) || 0;
            this.artifactTileRecords.set(artifact.firstTile.tile_id, {
              scene: artifact.model.children[0],
              bytes,
              lastUsedAt: performance.now()
            });
            this.artifactResidentBytes = bytes;
          }
          this.modelScene.add(this.model);
          this.orientModel(this.model);
          this.prepareModelForRender(this.model);
          this.currentKey = key;
          this.fitArtifactToManifest(artifact.manifest, this.model);
          this.deriveArtifactStoreys();
          this.applyPlanCut();
          this.applyModelLevel();
          this.showOrbitHint();
          if (!requiresIfcDiff) {
            this.loadRemainingArtifactTiles(artifact.tiles.slice(1), artifact.model, token);
            artifact.semanticIndex.then(index => {
              if (token !== this.loadToken || this.model !== artifact.model) return;
              this.setArtifactSemanticIndex(artifact.firstTile, index);
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

  setViewStatus(message, state = 'loading') {
    for (const status of [this.planStatus, this.modelStatus]) {
      status.textContent = message;
      status.dataset.state = message ? state : '';
    }
  }

  setLoadStatus(message) {
    this.setViewStatus(message, 'loading');
  }

  setModelSourceMeta(mode) {
    const suffix = mode === 'changes' ? 'changes only' : 'full model';
    if (this.modelKind === 'artifact') {
      const progress = this.artifactManifest && this.artifactManifest.tiles
        ? `${this.artifactTileRecords.size}/${this.artifactManifest.tiles.length} tiles`
        : 'coarse tile';
      this.setViewStatus('');
      this.planMeta.textContent = `render artifact · ${progress}`;
      this.modelMeta.textContent = `render artifact · ${progress} · ${suffix}`;
      return;
    }
    this.setViewStatus('');
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
    this.prepareModelForRender(this.model);
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
      if (!tiles.length || !this.semanticIndexResource(manifest, tiles[0])) {
        return {reason: 'Render artifact is incomplete; using raw IFC fallback.'};
      }
      // Coarse proxies (higher lod / larger geometric error) sort first so the
      // initial synchronous tile and early streaming give fast whole-model
      // coverage; exact LOD0 tiles are upgraded afterwards by distance and
      // interaction.
      tiles.sort((a, b) => (b.lod - a.lod) || (b.geometric_error - a.geometric_error) || (a.tile_id < b.tile_id ? -1 : a.tile_id > b.tile_id ? 1 : 0));
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
      const semanticIndex = this.fetchArtifactSemanticIndexForTile(manifest, tiles[0], controller.signal);
      return {model, manifest, tiles, semanticIndex, firstTile: tiles[0]};
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

  semanticIndexResource(manifest, tile) {
    if (manifest && manifest.semantic_index && manifest.semantic_index.artifact) {
      return manifest.semantic_index.artifact;
    }
    if (tile && tile.semantic_index && tile.semantic_index.artifact) {
      return tile.semantic_index.artifact;
    }
    return null;
  }

  async fetchArtifactSemanticIndexForTile(manifest, tile, signal) {
    const resource = this.semanticIndexResource(manifest, tile);
    if (!resource) throw new Error(`artifact tile ${tile && tile.tile_id || 'unknown'} has no semantic index`);
    return this.fetchArtifactSemanticIndex(resource, signal);
  }

  setArtifactSemanticIndex(tile, index) {
    const tileId = tile && tile.tile_id;
    if (tileId) this.artifactSemanticIndexes.set(tileId, index);
    this.artifactSemanticIndex = index;
  }

  async loadRemainingArtifactTiles(tiles, model, token) {
    this.registerArtifactTiles(tiles);
    this.queueMissingArtifactTiles(model, token);
    this.scheduleArtifactTiles();
  }

  scheduleArtifactTiles() {
    if (!this.artifactAbortController || !this.model || this.modelKind !== 'artifact') return;
    // Distance and interaction can flip which LOD a group wants, so refresh
    // visibility (to reveal an already-resident upgrade) before queueing any
    // newly-wanted tiles.
    this.refreshArtifactVisibility();
    this.queueMissingArtifactTiles(this.model, this.loadToken);
    if (this.artifactSchedulePending || !this.artifactTileQueue.length) return;
    this.artifactSchedulePending = true;
    requestAnimationFrame(() => {
      this.artifactSchedulePending = false;
      this.pumpArtifactTileQueue();
    });
  }

  registerArtifactTiles(tiles) {
    for (const tile of tiles || []) {
      if (!tile || !tile.tile_id) continue;
      this.artifactTileDescriptors.set(tile.tile_id, tile);
      const groupId = this.artifactTileGroupId(tile);
      let group = this.artifactGroups.get(groupId);
      if (!group) {
        group = {id: groupId, exact: null, coarse: null};
        this.artifactGroups.set(groupId, group);
      }
      if (Number(tile.lod) > 0) {
        // Keep the coarsest proxy (largest geometric error) as the group's
        // single coarse LOD when several are ever published.
        if (!group.coarse || Number(tile.geometric_error) > Number(group.coarse.geometric_error)) {
          group.coarse = tile;
        }
      } else {
        group.exact = tile;
      }
    }
  }

  // The stable ownership key shared by every LOD tile that renders one logical
  // group. Falls back to the tile id for single-LOD (accurate profile / v1)
  // tiles that never advertise a group.
  artifactTileGroupId(tile) {
    if (!tile) return null;
    return (typeof tile.group === 'string' && tile.group) ? tile.group : tile.tile_id;
  }

  queueMissingArtifactTiles(model, token) {
    if (!model || token !== this.loadToken) return;
    const queued = new Set(this.artifactTileQueue.map(request => request.tile && request.tile.tile_id));
    for (const tile of this.artifactTileDescriptors.values()) {
      const tileId = tile.tile_id;
      if (!tileId
        || queued.has(tileId)
        || this.artifactTileRecords.has(tileId)
        || this.artifactTileLoading.has(tileId)) {
        continue;
      }
      // Only stream the LOD each group currently wants: one LOD per group,
      // coarse for distant/initial groups and exact for near/selected/isolated
      // ones. Non-wanted LODs and hidden storeys stay as descriptors and are
      // queued when distance, interaction, or isolation changes.
      if (!this.artifactTileWanted(tileId)) continue;
      this.artifactTileQueue.push({tile, model, token});
      queued.add(tileId);
    }
  }

  // v2 keeps a semantic index per tile; v1 keeps a single manifest-global one.
  artifactUsesTileLocalIndex() {
    const manifest = this.artifactManifest;
    return !!manifest && !(manifest.semantic_index && manifest.semantic_index.artifact);
  }

  // Resolve the storey a tile belongs to. Explicit manifest storey metadata
  // wins; otherwise fall back to the stable `storey-<globalId>` v2 ownership
  // key (a tile's group id, or its tile id for single-LOD tiles). Returns null
  // for the unassigned tile and for v1 LOD tiles. Deriving from the group id
  // keeps a coarse proxy (`storey-<id>/coarse`) in the same storey as its
  // exact tile instead of inventing a phantom storey.
  tileStoreyId(tile) {
    if (!tile) return null;
    if (tile.storey && typeof tile.storey.global_id === 'string') return tile.storey.global_id;
    if (typeof tile.storey_id === 'string') return tile.storey_id;
    const groupId = this.artifactTileGroupId(tile);
    if (typeof groupId === 'string' && groupId.indexOf('storey-') === 0) return groupId.slice('storey-'.length);
    return null;
  }

  // Build the storey list for artifact mode from group ownership so the level
  // selector, prioritisation, and isolation all agree. One storey is derived
  // per storey-owning group, and every LOD tile of that group is registered as
  // storey-owned so isolation shows or hides a storey's coarse and exact tiles
  // together. The unassigned group is intentionally excluded: it is not a
  // storey and stays visible under isolation.
  deriveArtifactStoreys() {
    this.artifactStoreyTileIds = new Set();
    this.storeys = [];
    this.planLevelIndex = 0;
    this.modelLevelIndex = null;
    if (this.modelKind !== 'artifact' || !this.artifactManifest) {
      if (typeof this.onStoreys === 'function') this.onStoreys([]);
      return;
    }
    const rotationAxis = this.upAxisFix ? new THREE.Vector3(1, 0, 0) : null;
    const found = [];
    for (const group of this.artifactGroups.values()) {
      const representative = group.exact || group.coarse;
      const storeyId = this.tileStoreyId(representative);
      if (!storeyId) continue;
      // Every LOD tile of this storey group is storey-owned.
      for (const tile of [group.exact, group.coarse]) {
        if (tile && tile.tile_id) this.artifactStoreyTileIds.add(tile.tile_id);
      }
      let elevation = null;
      const bounds = representative && representative.bounds;
      if (bounds && Array.isArray(bounds.min) && bounds.min.length === 3 && bounds.min.every(Number.isFinite)) {
        const min = new THREE.Vector3(bounds.min[0], bounds.min[1], bounds.min[2]);
        if (rotationAxis) min.applyAxisAngle(rotationAxis, this.upAxisFix);
        elevation = min.z;
      }
      found.push({groupId: group.id, tileId: representative.tile_id, storeyId, elevation});
    }
    // Ground-up ordering so the level list reads bottom to top.
    found.sort((a, b) => (a.elevation ?? 0) - (b.elevation ?? 0));
    this.storeys = found.map((entry, index) => ({
      groupId: entry.groupId,
      tileId: entry.tileId,
      storeyId: entry.storeyId,
      name: `Storey ${index + 1}`,
      elevation: entry.elevation
    }));
    if (typeof this.onStoreys === 'function') this.onStoreys(this.storeys);
  }

  activeArtifactStoreyGroupId() {
    if (this.modelKind !== 'artifact') return null;
    if (this.modelLevelIndex === null || this.modelLevelIndex === undefined) return null;
    const storey = this.storeys[this.modelLevelIndex];
    return storey ? storey.groupId : null;
  }

  // Estimate a group's world-space centre and radius (half the bounds
  // diagonal) after the viewer's Y-up -> Z-up correction, so distance
  // heuristics match what the camera actually sees.
  artifactGroupBounds(group) {
    const source = (group && group.exact) || (group && group.coarse);
    const bounds = source && source.bounds;
    if (!bounds || !Array.isArray(bounds.min) || !Array.isArray(bounds.max)
      || !bounds.min.every(Number.isFinite) || !bounds.max.every(Number.isFinite)) {
      return null;
    }
    const center = new THREE.Vector3(
      (bounds.min[0] + bounds.max[0]) / 2,
      (bounds.min[1] + bounds.max[1]) / 2,
      (bounds.min[2] + bounds.max[2]) / 2
    );
    if (this.upAxisFix) center.applyAxisAngle(new THREE.Vector3(1, 0, 0), this.upAxisFix);
    const radius = 0.5 * Math.hypot(
      bounds.max[0] - bounds.min[0],
      bounds.max[1] - bounds.min[1],
      bounds.max[2] - bounds.min[2]
    );
    return {center, radius};
  }

  // A group is close enough to justify its exact LOD0 tile when it subtends a
  // large enough fraction of the view.
  artifactGroupNearCamera(group) {
    const info = this.artifactGroupBounds(group);
    if (!info) return false;
    const distance = info.center.distanceTo(this.modelCamera.position);
    if (!(distance > 0)) return true;
    return (info.radius / distance) > this.artifactExactLodRatio;
  }

  // Decide whether a group should show its exact LOD0 tile (true) or its
  // coarse proxy (false). Single-LOD groups, the selected group, the isolated
  // storey, and near groups all want exact; everything else stays coarse.
  groupDesiresExact(groupId) {
    const group = this.artifactGroups.get(groupId);
    if (!group || !group.coarse) return true;
    if (!group.exact) return false;
    if (this.selectedArtifactGroupId === groupId) return true;
    const activeGroupId = this.activeArtifactStoreyGroupId();
    if (activeGroupId !== null) return groupId === activeGroupId;
    return this.artifactGroupNearCamera(group);
  }

  // The LOD tile a group aims to show (drives fetching). Falls back to the
  // other LOD when the preferred one does not exist.
  preferredGroupLodTileId(groupId) {
    const group = this.artifactGroups.get(groupId);
    if (!group) return groupId;
    const exactId = group.exact && group.exact.tile_id;
    const coarseId = group.coarse && group.coarse.tile_id;
    return this.groupDesiresExact(groupId) ? (exactId || coarseId) : (coarseId || exactId);
  }

  // The single LOD tile a group should currently render: the best resident
  // tile matching the preference, else any resident tile, else the preferred
  // (not-yet-resident) tile. Guarantees exactly one visible LOD per group and
  // keeps the coarse proxy on screen until the exact upgrade arrives.
  visibleGroupLodTileId(groupId) {
    const group = this.artifactGroups.get(groupId);
    if (!group) return groupId;
    const exactId = group.exact && group.exact.tile_id;
    const coarseId = group.coarse && group.coarse.tile_id;
    const desiresExact = this.groupDesiresExact(groupId);
    const preferred = desiresExact ? (exactId || coarseId) : (coarseId || exactId);
    const other = desiresExact ? coarseId : exactId;
    if (preferred && this.artifactTileRecords.has(preferred)) return preferred;
    if (other && this.artifactTileRecords.has(other)) return other;
    return preferred;
  }

  // Whether a tile is the LOD its group currently wants resident. Drives which
  // tiles are streamed. Hidden storeys are never wanted.
  artifactTileWanted(tileId) {
    const tile = this.artifactTileDescriptors.get(tileId);
    if (!tile) return false;
    const groupId = this.artifactTileGroupId(tile);
    const activeGroupId = this.activeArtifactStoreyGroupId();
    if (activeGroupId !== null && this.artifactStoreyTileIds.has(tileId) && groupId !== activeGroupId) {
      return false;
    }
    return this.preferredGroupLodTileId(groupId) === tileId;
  }

  // A tile is visible when it is its group's currently rendered LOD and the
  // group is not hidden by storey isolation.
  artifactTileVisible(tileId) {
    if (!tileId) return true;
    const tile = this.artifactTileDescriptors.get(tileId);
    const groupId = tile ? this.artifactTileGroupId(tile) : tileId;
    const activeGroupId = this.activeArtifactStoreyGroupId();
    if (activeGroupId !== null && this.artifactStoreyTileIds.has(tileId) && groupId !== activeGroupId) {
      return false;
    }
    return this.visibleGroupLodTileId(groupId) === tileId;
  }

  // Recompute per-tile visibility so exactly one LOD per group is shown as
  // tiles arrive and as distance/interaction/isolation change.
  refreshArtifactVisibility() {
    if (!this.model) return;
    for (const child of this.model.children) {
      const tileId = child.userData && child.userData.vexTileId;
      child.visible = this.artifactTileVisible(tileId);
    }
  }

  applyArtifactStoreyIsolation() {
    // Section stays plane-based; storey isolation is by tile ownership so
    // tall/multi-storey elements are shown whole rather than sliced.
    this.modelRenderer.clippingPlanes = this.sectionActive ? [this.sectionPlane] : [];
    if (!this.model) return;
    this.refreshArtifactVisibility();
    // Prefer streaming the isolated storey next.
    this.scheduleArtifactTiles();
  }

  artifactTilePriority(tile) {
    const groupId = this.artifactTileGroupId(tile);
    const activeGroupId = this.activeArtifactStoreyGroupId();
    // The isolated storey's tile is the most useful thing to have on screen.
    if (activeGroupId !== null && groupId === activeGroupId) return -Infinity;
    // A pending selection upgrade to exact LOD0 is the next most urgent fetch.
    if (this.selectedArtifactGroupId && groupId === this.selectedArtifactGroupId && Number(tile.lod) === 0) {
      return -1e17;
    }
    const bounds = tile && tile.bounds;
    if (!bounds || !Array.isArray(bounds.min) || !Array.isArray(bounds.max)) return Number.MAX_SAFE_INTEGER;
    const center = new THREE.Vector3(
      (bounds.min[0] + bounds.max[0]) / 2,
      (bounds.min[1] + bounds.max[1]) / 2,
      (bounds.min[2] + bounds.max[2]) / 2
    );
    if (this.upAxisFix) center.applyAxisAngle(new THREE.Vector3(1, 0, 0), this.upAxisFix);
    // Coarse proxies (lod > 0) are streamed first for fast whole-model
    // coverage; exact LOD0 tiles follow. Distance orders equally detailed
    // tiles so nearer content arrives sooner.
    const isCoarse = Number(tile.lod) > 0;
    let priority = (isCoarse ? 0 : 1e15) + center.distanceToSquared(this.modelCamera.position);
    // When a storey is isolated, tiles owned by hidden storeys are the least
    // urgent to stream.
    if (activeGroupId !== null && groupId !== activeGroupId && this.artifactStoreyTileIds.has(tile.tile_id)) {
      priority += 1e18;
    }
    return priority;
  }

  pumpArtifactTileQueue() {
    if (!this.artifactAbortController || !this.model || this.modelKind !== 'artifact') return;
    this.artifactTileQueue.sort((left, right) => this.artifactTilePriority(left.tile) - this.artifactTilePriority(right.tile));
    while (this.artifactTileLoads < this.artifactMaxInflight && this.artifactTileQueue.length) {
      const request = this.artifactTileQueue.shift();
      if (!request || request.token !== this.loadToken || request.model !== this.model) continue;
      const tileId = request.tile && request.tile.tile_id;
      // Never dispatch a tile that is already resident or already in flight.
      if (!tileId || this.artifactTileRecords.has(tileId) || this.artifactTileLoading.has(tileId)) continue;
      this.artifactTileLoading.add(tileId);
      this.artifactTileLoads += 1;
      this.loadScheduledArtifactTile(request).finally(() => {
        this.artifactTileLoads -= 1;
        this.artifactTileLoading.delete(tileId);
        this.pumpArtifactTileQueue();
      });
    }
  }

  async loadScheduledArtifactTile({tile, model, token}) {
    try {
      if (!this.artifactAbortController) return;
      this.setLoadStatus(`Render artifact: streaming tiles ${this.artifactTileRecords.size}/${this.artifactManifest.tiles.length}...`);
      const scenePromise = this.loadArtifactTile(tile, this.artifactAbortController.signal, token);
      const indexPromise = this.fetchArtifactSemanticIndexForTile(
        this.artifactManifest,
        tile,
        this.artifactAbortController.signal
      );
      let scene;
      let semanticIndex;
      try {
        [scene, semanticIndex] = await Promise.all([scenePromise, indexPromise]);
      } catch (error) {
        scenePromise.then(loadedScene => this.disposeArtifactObject(loadedScene)).catch(() => {});
        throw error;
      }
      if (token !== this.loadToken || this.model !== model) {
        this.disposeArtifactObject(scene);
        return;
      }
      const bytes = Number(tile.artifact && tile.artifact.byte_length) || 0;
      this.evictArtifactTiles(bytes, tile.tile_id);
      scene.visible = this.artifactTileVisible(tile.tile_id);
      model.add(scene);
      this.applyMaterialQuality(scene);
      this.updateShadowPolicy(model);
      this.artifactTileRecords.set(tile.tile_id, {scene, bytes, lastUsedAt: performance.now()});
      this.setArtifactSemanticIndex(tile, semanticIndex);
      this.artifactResidentBytes += bytes;
      // A newly-resident tile can be the exact upgrade a group now wants, so
      // refresh visibility to swap it in (and hide the coarse proxy) without
      // ever rendering both LODs of a group at once.
      this.refreshArtifactVisibility();
      // If this exact tile completes a pending coarse-selection upgrade,
      // finalise the precise selection against its LOD0 geometry.
      this.maybeFinalizeArtifactSelection();
      this.setModelSourceMeta(currentViewMode);
    } catch (error) {
      if (error && error.name === 'AbortError') return;
      console.warn('Render artifact tile failed to load:', error);
      if (token === this.loadToken && this.model === model) {
        this.modelMeta.textContent = `render artifact · ${this.artifactTileRecords.size} tiles (some unavailable)`;
      }
    }
  }

  evictArtifactTiles(incomingBytes, protectedTileId) {
    if (this.artifactResidentBytes + incomingBytes <= this.artifactMemoryBudgetBytes) return;
    // Never evict the tile we are about to commit, the base/coarse tile, the
    // storey the user is currently isolating, or the group with an active
    // selection upgrade in flight.
    const activeGroupId = this.activeArtifactStoreyGroupId();
    const selectedGroupId = this.selectedArtifactGroupId;
    const candidates = [...this.artifactTileRecords.entries()]
      .filter(([tileId]) => {
        if (tileId === protectedTileId || tileId === this.artifactBaseTileId) return false;
        const groupId = this.artifactTileGroupId(this.artifactTileDescriptors.get(tileId));
        if (activeGroupId !== null && groupId === activeGroupId) return false;
        if (selectedGroupId && groupId === selectedGroupId) return false;
        return true;
      })
      .sort(([, left], [, right]) => {
        // Reclaim hidden tiles before visible ones, then least-recently-used.
        const leftVisible = left.scene && left.scene.visible ? 1 : 0;
        const rightVisible = right.scene && right.scene.visible ? 1 : 0;
        if (leftVisible !== rightVisible) return leftVisible - rightVisible;
        return left.lastUsedAt - right.lastUsedAt;
      });
    for (const [tileId, record] of candidates) {
      if (this.artifactResidentBytes + incomingBytes <= this.artifactMemoryBudgetBytes) break;
      if (record.scene && record.scene.parent) record.scene.parent.remove(record.scene);
      this.disposeArtifactObject(record.scene);
      this.artifactTileRecords.delete(tileId);
      // Drop the evicted tile's semantic index so a stale index can never map a
      // future hit on a re-streamed tile to the wrong element.
      this.artifactSemanticIndexes.delete(tileId);
      this.artifactResidentBytes = Math.max(0, this.artifactResidentBytes - record.bytes);
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
    this.setLoadStatus(`Preparing local preview of ${file.name}…`);
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
    this.prepareModelForRender(model);
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
    this.setViewStatus('');
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
    this.setLoadStatus('Downloading committed IFC...');
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
      this.setLoadStatus(label);
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
    this.artifactSemanticIndexes.clear();
    this.artifactTileQueue = [];
    this.artifactTileDescriptors.clear();
    this.artifactTileRecords.clear();
    this.artifactTileLoading.clear();
    this.artifactStoreyTileIds = new Set();
    this.artifactBaseTileId = null;
    this.artifactGroups.clear();
    this.selectedArtifactGroupId = null;
    this.pendingArtifactSelection = null;
    this.artifactResidentBytes = 0;
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
    const diameter = Math.max(size.x, size.y, size.z, 1);
    const halfExtent = diameter * 0.5;
    const fov = THREE.MathUtils.degToRad(this.modelPersp.fov);
    const distance = Math.max(diameter * 1.5, (halfExtent / Math.tan(fov / 2)) * 1.22);
    const direction = new THREE.Vector3(1, -1, 0.72).normalize();
    this.modelCamera.position.copy(center).addScaledVector(direction, distance);
    if (this.modelCamera.isPerspectiveCamera) {
      this.modelCamera.near = Math.max(diameter / 10000, 0.001);
      this.modelCamera.far = Math.max(diameter * 200, 1000);
    }
    this.modelCamera.lookAt(center);
    this.modelCamera.updateProjectionMatrix();
    this.controls.target.copy(center);
    // Bound the dolly so the wheel can't fly past the model or invert through it.
    this.controls.minDistance = Math.max(diameter * 0.025, 0.01);
    this.controls.maxDistance = diameter * 50;
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
    const planSize = (Math.max(size.x, size.y, 1) * 0.68) / this.planZoom;
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
  return new THREE.MeshStandardMaterial({
    color: colors[kind] || 0xa8aaa7,
    transparent: true,
    opacity: 0.9,
    side: THREE.DoubleSide,
    depthTest: true,
    roughness: 0.5,
    metalness: 0
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
  if (els.modelStatus) { els.modelStatus.textContent = msg; els.modelStatus.dataset.state = 'empty'; }
  if (els.planStatus) { els.planStatus.textContent = msg; els.planStatus.dataset.state = 'empty'; }
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
