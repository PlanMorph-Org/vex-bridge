#!/usr/bin/env node
/**
 * End-to-end fidelity check for the derived render path.
 *
 * It compares raw Node-target web-ifc extraction with the generated GLB and
 * semantic index for a shape-bearing IFC fixture. This catches the most
 * damaging failure mode for a fast viewer: a valid-looking tile that silently
 * drops or remaps geometry.
 */

import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const node = process.execPath;
const fixture = join(root, 'crates', 'vex-bridge', 'tests', 'fixtures', 'simple-extruded-wall.ifc');
const worker = join(root, 'tools', 'vex-render-worker.mjs');
const spike = join(root, 'tools', 'ifc-render-spike.mjs');
const webIfcDir = join(root, 'crates', 'vex-bridge', 'assets', 'render-worker', 'web-ifc');
const api = join(webIfcDir, 'web-ifc-api-node.js');
const commit = 'd'.repeat(64);

function run(command, args) {
  return new Promise((resolveRun, reject) => {
    const child = spawn(command, args, { windowsHide: true });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', chunk => { stdout += chunk; });
    child.stderr.on('data', chunk => { stderr += chunk; });
    child.on('error', reject);
    child.on('close', code => {
      if (code === 0) resolveRun({ stdout, stderr });
      else reject(new Error(`${command} exited ${code}: ${stderr || stdout}`));
    });
  });
}

function jsonOutput(stdout) {
  const line = stdout.trim().split(/\r?\n/).reverse().find(value => value.trim().startsWith('{'));
  if (!line) throw new Error(`command did not emit JSON: ${stdout}`);
  return JSON.parse(line);
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function glbJson(glb) {
  const jsonLength = glb.readUInt32LE(12);
  assert(glb.readUInt32LE(16) === 0x4e4f534a, 'GLB JSON chunk missing');
  return JSON.parse(glb.subarray(20, 20 + jsonLength).toString('utf8').trim());
}

const out = await mkdtemp(join(tmpdir(), 'vex-render-pressure-'));
try {
  const raw = jsonOutput((await run(node, [spike, fixture])).stdout);
  const generated = jsonOutput((await run(node, [
    worker,
    '--ifc', fixture,
    '--out', out,
    '--project-id', 'pressure-project',
    '--commit', commit,
    '--web-ifc-api', api,
    '--wasm-dir', webIfcDir,
  ])).stdout);
  const manifest = JSON.parse(await readFile(join(out, 'manifest.json'), 'utf8'));
  const tile = manifest.tiles?.[0];
  assert(manifest.schema === 'vex.render-manifest/1', 'manifest schema mismatch');
  assert(tile?.artifact?.content_type === 'model/gltf-binary', 'tile is not GLB');

  const glb = await readFile(join(out, 'objects', tile.artifact.sha256));
  assert(glb.readUInt32LE(0) === 0x46546c67, 'GLB magic mismatch');
  assert(glb.readUInt32LE(4) === 2, 'GLB version mismatch');
  assert(glb.readUInt32LE(8) === glb.length, 'GLB length mismatch');
  assert(sha256(glb) === tile.artifact.sha256, 'GLB digest mismatch');
  const gltf = glbJson(glb);
  assert(gltf.meshes?.[0]?.primitives?.[0]?.attributes?.COLOR_0 !== undefined,
    'GLB does not preserve IFC placement colors');

  const indexResource = manifest.semantic_index.artifact;
  const semanticIndex = JSON.parse(await readFile(join(out, 'objects', indexResource.sha256), 'utf8'));
  assert(sha256(Buffer.from(JSON.stringify(semanticIndex))) === indexResource.sha256, 'semantic index digest mismatch');
  const mappedTriangles = semanticIndex.entries
    .flatMap(entry => entry.triangle_ranges || [])
    .reduce((sum, range) => sum + range.triangle_count, 0);
  const mappedGlobalIds = semanticIndex.entries.filter(entry => entry.global_id).length;

  assert(raw.vertex_count > 0 && raw.triangle_count > 0, 'fixture did not yield geometry');
  assert(generated.vertex_count === raw.vertex_count, 'artifact vertex count differs from raw web-ifc');
  assert(generated.triangle_count === raw.triangle_count, 'artifact triangle count differs from raw web-ifc');
  assert(mappedTriangles === raw.triangle_count, 'semantic ranges do not cover every triangle');
  assert(mappedGlobalIds === raw.global_id_count, 'GlobalId mapping differs from raw web-ifc');

  console.log(JSON.stringify({
    schema: 'vex.render-pressure-test/1',
    status: 'passed',
    vertex_count: raw.vertex_count,
    triangle_count: raw.triangle_count,
    global_id_count: mappedGlobalIds,
    glb_bytes: glb.length,
    raw_timings_ms: raw.timings_ms,
    generated_timings_ms: generated.timings_ms,
  }));
} finally {
  await rm(out, { recursive: true, force: true });
}
