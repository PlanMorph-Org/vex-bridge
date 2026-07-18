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

const out = await mkdtemp(join(root, '.render-pressure-test-'));
try {
  const raw = jsonOutput((await run(node, [spike, fixture])).stdout);
  // No --render-profile: this exercises the default (balanced) path, which
  // emits the exact LOD0 tile plus a conservative coarse proxy.
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
  assert(manifest.schema === 'vex.render-manifest/2', 'manifest schema mismatch');
  assert(manifest.render_policy?.id === 'vex.render-policy.storey-first/1', 'render policy identity mismatch');
  assert(Array.isArray(manifest.tiles) && manifest.tiles.length >= 1, 'manifest has no tiles');
  // The simple fixture has no spatial hierarchy, so all of its geometry falls
  // into the single deterministic unassigned group. LOD0 must be exactly one
  // exact unassigned tile; the default profile also adds one coarse proxy.
  const exactTiles = manifest.tiles.filter(tile => tile.lod === 0);
  const coarseTiles = manifest.tiles.filter(tile => tile.lod > 0);
  assert(exactTiles.length === 1 && exactTiles[0].tile_id === 'unassigned',
    'simple fixture must yield exactly one exact unassigned tile');
  assert(exactTiles[0].geometric_error === 0, 'exact LOD0 tile must have zero geometric error');
  assert(coarseTiles.length === 1 && coarseTiles[0].tile_id === 'unassigned/coarse',
    'default profile must emit exactly one coarse proxy tile');
  assert(coarseTiles[0].lod > 0 && coarseTiles[0].geometric_error > 0,
    'coarse proxy must carry a higher LOD and a positive geometric error');
  assert(coarseTiles[0].group === 'unassigned' && exactTiles[0].group === 'unassigned',
    'both LODs must share the same group ownership key');

  let exactTriangles = 0;
  let exactMappedTriangles = 0;
  let exactMappedGlobalIds = 0;
  let coarseMappedGlobalIds = 0;
  let glbBytes = 0;
  for (const tile of manifest.tiles) {
    assert(tile.artifact?.content_type === 'model/gltf-binary', 'tile is not GLB');
    const glb = await readFile(join(out, 'objects', tile.artifact.sha256));
    assert(glb.readUInt32LE(0) === 0x46546c67, 'GLB magic mismatch');
    assert(glb.readUInt32LE(4) === 2, 'GLB version mismatch');
    assert(glb.readUInt32LE(8) === glb.length, 'GLB length mismatch');
    assert(sha256(glb) === tile.artifact.sha256, 'GLB digest mismatch');
    const gltf = glbJson(glb);
    const primitive = gltf.meshes?.[0]?.primitives?.[0];
    let tileTriangles = 0;
    if (primitive) {
      assert(primitive.attributes?.COLOR_0 !== undefined,
        'GLB does not preserve IFC placement colors');
      tileTriangles = gltf.accessors[primitive.indices].count / 3;
    }
    glbBytes += glb.length;

    // v2 keeps the semantic index tile-local, not manifest-global.
    const indexResource = tile.semantic_index.artifact;
    const semanticIndex = JSON.parse(await readFile(join(out, 'objects', indexResource.sha256), 'utf8'));
    assert(sha256(Buffer.from(JSON.stringify(semanticIndex))) === indexResource.sha256, 'semantic index digest mismatch');
    assert(semanticIndex.tile_id === tile.tile_id, 'semantic index tile_id mismatch');
    assert(semanticIndex.lod === tile.lod, 'semantic index lod mismatch');
    const tileMappedTriangles = semanticIndex.entries
      .flatMap(entry => entry.triangle_ranges || [])
      .reduce((sum, range) => sum + range.triangle_count, 0);
    const tileMappedGlobalIds = semanticIndex.entries.filter(entry => entry.global_id).length;
    // Every tile (exact or coarse) must fully cover its own GLB triangles so a
    // pick never lands on an unmapped triangle.
    assert(tileMappedTriangles === tileTriangles,
      `tile ${tile.tile_id} semantic ranges do not cover every triangle`);
    if (tile.lod === 0) {
      exactTriangles += tileTriangles;
      exactMappedTriangles += tileMappedTriangles;
      exactMappedGlobalIds += tileMappedGlobalIds;
    } else {
      coarseMappedGlobalIds += tileMappedGlobalIds;
    }
  }

  assert(raw.vertex_count > 0 && raw.triangle_count > 0, 'fixture did not yield geometry');
  // Parity is asserted against the exact LOD0 rendition only: the coarse proxy
  // is a conservative bounding-box approximation and is not expected to match
  // raw web-ifc geometry.
  assert(generated.exact_vertex_count === raw.vertex_count, 'exact LOD0 vertex count differs from raw web-ifc');
  assert(generated.exact_triangle_count === raw.triangle_count, 'exact LOD0 triangle count differs from raw web-ifc');
  assert(exactTriangles === raw.triangle_count, 'exact GLB tile triangles differ from raw web-ifc');
  assert(exactMappedTriangles === raw.triangle_count, 'exact semantic ranges do not cover every triangle');
  assert(exactMappedGlobalIds === raw.global_id_count, 'exact GlobalId mapping differs from raw web-ifc');
  // The coarse proxy retains element identity so a coarse pick can be upgraded.
  assert(coarseMappedGlobalIds === raw.global_id_count, 'coarse proxy dropped GlobalId mapping');

  console.log(JSON.stringify({
    schema: 'vex.render-pressure-test/1',
    status: 'passed',
    tile_count: manifest.tiles.length,
    exact_tile_count: exactTiles.length,
    coarse_tile_count: coarseTiles.length,
    vertex_count: raw.vertex_count,
    triangle_count: raw.triangle_count,
    global_id_count: exactMappedGlobalIds,
    glb_bytes: glbBytes,
    raw_timings_ms: raw.timings_ms,
    generated_timings_ms: generated.timings_ms,
  }));
} finally {
  await rm(out, { recursive: true, force: true });
}
