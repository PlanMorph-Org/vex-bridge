#!/usr/bin/env node
/**
 * Deterministic behavioural test for multi-LOD artifacts and profile-aware
 * level-of-detail policy in the render worker.
 *
 * It drives the real worker against the multi-storey fixture (and a purpose
 * built large-plus-tiny fixture) and proves the properties this milestone
 * requires:
 *   - the three render profiles (draft / balanced / accurate) each produce a
 *     distinct render-policy hash and artifact identity, even when their
 *     coarse geometry happens to be identical;
 *   - balanced (the default) and draft emit a coarse LOD proxy per storey plus
 *     the exact LOD0 tile; accurate emits only the exact LOD0 tiles;
 *   - the exact LOD0 tile for every group is byte-for-byte identical across all
 *     profiles (LOD0 stays the exact current-quality rendition);
 *   - coarse proxies sort first, carry a higher lod and a positive geometric
 *     error, share their group's ownership key, and remain valid pickable GLB
 *     whose semantic index retains every element's Express ID and GlobalId (so
 *     a coarse pick can be upgraded to the exact element);
 *   - the draft profile drops elements that are tiny relative to their tile
 *     from the coarse proxy only, never from the exact LOD0 tile;
 *   - a model with no geometry still publishes exactly one valid empty exact
 *     tile (full-model fallback);
 *   - artifact identity and every object hash are deterministic across runs.
 */

import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const node = process.execPath;
const worker = join(root, 'tools', 'vex-render-worker.mjs');
const fixture = join(root, 'crates', 'vex-bridge', 'tests', 'fixtures', 'multi-storey-building.ifc');
const thresholdFixture = join(root, 'crates', 'vex-bridge', 'tests', 'fixtures', 'lod-detail-threshold.ifc');
const webIfcDir = join(root, 'crates', 'vex-bridge', 'assets', 'render-worker', 'web-ifc');
const api = join(webIfcDir, 'web-ifc-api-node.js');
const commit = 'a'.repeat(64);

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

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

async function generate(ifc, out, profile) {
  const args = [
    worker,
    '--ifc', ifc,
    '--out', out,
    '--project-id', 'lod-profiles-project',
    '--commit', commit,
    '--web-ifc-api', api,
    '--wasm-dir', webIfcDir,
  ];
  if (profile) args.push('--render-profile', profile);
  await run(node, args);
  return JSON.parse(await readFile(join(out, 'manifest.json'), 'utf8'));
}

function glbSummary(glb) {
  assert(glb.readUInt32LE(0) === 0x46546c67, 'bad GLB magic');
  assert(glb.readUInt32LE(4) === 2, 'bad GLB version');
  assert(glb.readUInt32LE(8) === glb.length, 'bad GLB length');
  assert(glb.readUInt32LE(16) === 0x4e4f534a, 'GLB JSON chunk missing');
  const jsonLength = glb.readUInt32LE(12);
  const gltf = JSON.parse(glb.subarray(20, 20 + jsonLength).toString('utf8').trim());
  const primitive = gltf.meshes?.[0]?.primitives?.[0];
  return {
    triangles: primitive ? gltf.accessors[primitive.indices].count / 3 : 0,
    hasColor: primitive ? primitive.attributes?.COLOR_0 !== undefined : true,
  };
}

async function loadObject(out, resource) {
  const bytes = await readFile(join(out, 'objects', resource.sha256));
  assert(sha256(bytes) === resource.sha256, `object digest mismatch for ${resource.sha256}`);
  return bytes;
}

// Validate every tile's GLB and tile-local semantic index, returning a
// per-tile record for downstream cross-profile assertions.
async function validateManifest(out, manifest) {
  assert(manifest.schema === 'vex.render-manifest/2', 'manifest must be v2');
  assert(manifest.render_policy?.id === 'vex.render-policy.storey-first/1', 'unexpected render policy id');
  const records = [];
  for (const tile of manifest.tiles) {
    assert(tile.artifact.content_type === 'model/gltf-binary', `tile ${tile.tile_id} is not GLB`);
    assert(Number.isFinite(tile.geometric_error) && tile.geometric_error >= 0,
      `tile ${tile.tile_id} has invalid geometric_error`);
    assert(tile.bounds.min.every(Number.isFinite) && tile.bounds.max.every(Number.isFinite),
      `tile ${tile.tile_id} bounds not finite`);
    const glb = await loadObject(out, tile.artifact);
    const { triangles, hasColor } = glbSummary(glb);
    assert(hasColor, `tile ${tile.tile_id} lost placement colors`);

    const index = JSON.parse((await loadObject(out, tile.semantic_index.artifact)).toString('utf8'));
    assert(index.tile_id === tile.tile_id, `semantic index tile_id mismatch for ${tile.tile_id}`);
    assert(index.lod === tile.lod, `semantic index lod mismatch for ${tile.tile_id}`);
    assert(index.entries.length === tile.semantic_index.entry_count, `entry_count mismatch for ${tile.tile_id}`);
    const mappedTriangles = index.entries
      .flatMap(entry => entry.triangle_ranges || [])
      .reduce((sum, range) => sum + range.triangle_count, 0);
    assert(mappedTriangles === triangles,
      `tile ${tile.tile_id} semantic triangles (${mappedTriangles}) != GLB triangles (${triangles})`);
    records.push({
      tile,
      glbHash: tile.artifact.sha256,
      triangles,
      globalIds: index.entries.map(entry => entry.global_id).filter(Boolean),
      expressIds: index.entries.map(entry => entry.express_id).filter(Number.isFinite),
    });
  }
  return records;
}

function tileIds(manifest) {
  return manifest.tiles.map(tile => tile.tile_id);
}

const scratch = await mkdtemp(join(root, '.render-lod-profiles-test-'));
try {
  // --- Generate all three profiles for the multi-storey fixture ---------------
  const outBalanced = join(scratch, 'balanced');
  const outBalancedTwo = join(scratch, 'balanced-2');
  const outDraft = join(scratch, 'draft');
  const outAccurate = join(scratch, 'accurate');
  const outDefault = join(scratch, 'default');
  const balanced = await generate(fixture, outBalanced, 'balanced');
  const balancedTwo = await generate(fixture, outBalancedTwo, 'balanced');
  const draft = await generate(fixture, outDraft, 'draft');
  const accurate = await generate(fixture, outAccurate, 'accurate');
  const fallbackDefault = await generate(fixture, outDefault, null);

  const balancedRecords = await validateManifest(outBalanced, balanced);
  const draftRecords = await validateManifest(outDraft, draft);
  const accurateRecords = await validateManifest(outAccurate, accurate);

  // --- Default resolves to balanced ------------------------------------------
  assert(fallbackDefault.render_policy.hash === balanced.render_policy.hash,
    'omitting --render-profile must resolve to the balanced default policy');
  assert(fallbackDefault.artifact_id === balanced.artifact_id,
    'omitting --render-profile must resolve to the balanced default artifact');

  // --- Requirement 1: profile-hash / artifact-id separation ------------------
  const policyHashes = new Set([balanced.render_policy.hash, draft.render_policy.hash, accurate.render_policy.hash]);
  assert(policyHashes.size === 3, 'each profile must have a distinct render-policy hash');
  const artifactIds = new Set([balanced.artifact_id, draft.artifact_id, accurate.artifact_id]);
  assert(artifactIds.size === 3, 'each profile must have a distinct artifact_id');

  // --- Requirement 2/3: LOD structure, ordering, and group identity ----------
  // accurate: exact LOD0 tiles only, legacy tile ids, and no group key.
  assert(accurate.tiles.every(tile => tile.lod === 0), 'accurate profile must emit only exact LOD0 tiles');
  assert(accurate.tiles.every(tile => tile.geometric_error === 0), 'accurate LOD0 tiles must be exact');
  assert(accurate.tiles.every(tile => !('group' in tile)),
    'accurate single-LOD tiles must not carry a group key (preserves the original v2 shape)');
  assert(
    JSON.stringify(tileIds(accurate)) === JSON.stringify([
      'storey-STOREYA000000000000000',
      'storey-STOREYB000000000000000',
      'unassigned',
    ]),
    `accurate profile tile ids unexpected: ${JSON.stringify(tileIds(accurate))}`,
  );

  // balanced: every storey group owns a coarse LOD1 proxy that sorts before its
  // exact LOD0 tile and shares the group ownership key.
  const groups = new Map();
  balanced.tiles.forEach((tile, order) => {
    const groupId = tile.group || tile.tile_id;
    if (!groups.has(groupId)) groups.set(groupId, {});
    const slot = groups.get(groupId);
    if (tile.lod === 0) slot.exact = { tile, order };
    else slot.coarse = { tile, order };
  });
  for (const [groupId, slot] of groups) {
    assert(slot.exact, `group ${groupId} is missing its exact LOD0 tile`);
    assert(slot.coarse, `group ${groupId} is missing its coarse proxy under balanced`);
    assert(slot.coarse.order < slot.exact.order, `group ${groupId} coarse LOD must sort before exact LOD0`);
    assert(slot.coarse.tile.lod > slot.exact.tile.lod, `group ${groupId} coarse LOD must be numerically higher`);
    assert(slot.exact.tile.lod === 0 && slot.exact.tile.geometric_error === 0,
      `group ${groupId} exact tile must be LOD0 with zero geometric error`);
    assert(slot.coarse.tile.geometric_error > 0, `group ${groupId} coarse proxy must have a positive geometric error`);
    assert(slot.coarse.tile.group === groupId && slot.exact.tile.group === groupId,
      `group ${groupId} tiles must share the group ownership key`);
    assert(slot.coarse.tile.tile_id === `${groupId}/coarse`, `group ${groupId} coarse tile id must derive from the group`);
    assert(slot.exact.tile.tile_id === groupId, `group ${groupId} exact tile must keep the canonical group tile id`);
  }

  // --- Requirement 2: LOD0 is byte-identical across every profile ------------
  const exactHash = manifest => new Map(
    manifest.tiles.filter(tile => tile.lod === 0).map(tile => [tile.tile_id, tile.artifact.sha256]),
  );
  const balancedExact = exactHash(balanced);
  const draftExact = exactHash(draft);
  const accurateExact = exactHash(accurate);
  assert(balancedExact.size === accurateExact.size && balancedExact.size === draftExact.size,
    'every profile must expose the same set of exact LOD0 tiles');
  for (const [tileId, hash] of balancedExact) {
    assert(draftExact.get(tileId) === hash, `draft LOD0 tile ${tileId} is not byte-identical to balanced`);
    assert(accurateExact.get(tileId) === hash, `accurate LOD0 tile ${tileId} is not byte-identical to balanced`);
  }

  // --- Requirement 2/5: coarse proxy retains identity for selection upgrade --
  // For every group, the coarse proxy's elements are a subset of the exact
  // tile's elements (balanced keeps all of them), so a coarse pick always maps
  // to an element that exists in the exact LOD0 tile.
  const balancedByTile = new Map(balancedRecords.map(record => [record.tile.tile_id, record]));
  for (const [groupId, slot] of groups) {
    const coarse = balancedByTile.get(slot.coarse.tile.tile_id);
    const exact = balancedByTile.get(slot.exact.tile.tile_id);
    assert(coarse.globalIds.length > 0, `group ${groupId} coarse proxy dropped all GlobalIds`);
    assert(coarse.expressIds.length === coarse.globalIds.length,
      `group ${groupId} coarse proxy must map an Express ID for every element`);
    const exactGlobalIds = new Set(exact.globalIds);
    for (const globalId of coarse.globalIds) {
      assert(exactGlobalIds.has(globalId),
        `group ${groupId} coarse element ${globalId} is not present in the exact LOD0 tile`);
    }
    // balanced keeps every element, so the sets match exactly.
    assert(coarse.globalIds.length === exact.globalIds.length,
      `group ${groupId} balanced coarse proxy must keep every element`);
  }

  // --- Determinism ------------------------------------------------------------
  assert(balanced.artifact_id === balancedTwo.artifact_id, 'balanced artifact_id is not deterministic');
  for (let index = 0; index < balanced.tiles.length; index += 1) {
    assert(balanced.tiles[index].artifact.sha256 === balancedTwo.tiles[index].artifact.sha256,
      `balanced tile ${index} GLB hash is not deterministic`);
    assert(balanced.tiles[index].semantic_index.artifact.sha256
      === balancedTwo.tiles[index].semantic_index.artifact.sha256,
      `balanced tile ${index} semantic index hash is not deterministic`);
  }

  // --- Requirement 2: draft drops tiny elements from the coarse LOD only -----
  const outThreshBalanced = join(scratch, 'thresh-balanced');
  const outThreshDraft = join(scratch, 'thresh-draft');
  const threshBalanced = await generate(thresholdFixture, outThreshBalanced, 'balanced');
  const threshDraft = await generate(thresholdFixture, outThreshDraft, 'draft');
  await validateManifest(outThreshBalanced, threshBalanced);
  await validateManifest(outThreshDraft, threshDraft);

  const coarseOf = manifest => manifest.tiles.find(tile => tile.lod > 0);
  const exactOf = manifest => manifest.tiles.find(tile => tile.lod === 0);
  const balancedCoarse = coarseOf(threshBalanced);
  const draftCoarse = coarseOf(threshDraft);
  const balancedExactTile = exactOf(threshBalanced);
  const draftExactTile = exactOf(threshDraft);
  assert(balancedCoarse && draftCoarse, 'both profiles must emit a coarse proxy for the threshold fixture');
  assert(draftCoarse.semantic_index.entry_count < balancedCoarse.semantic_index.entry_count,
    'draft must drop at least one tiny element from the coarse proxy that balanced keeps');
  // LOD0 is unaffected by the profile and stays byte-identical.
  assert(draftExactTile.artifact.sha256 === balancedExactTile.artifact.sha256,
    'draft LOD0 tile must remain byte-identical to balanced for the threshold fixture');

  // The dropped tiny element is absent from draft's coarse proxy but still
  // present in the exact LOD0 tile (LOD0 never drops anything).
  const readIndex = async (out, tile) =>
    JSON.parse((await loadObject(out, tile.semantic_index.artifact)).toString('utf8'));
  const balancedCoarseIds = new Set((await readIndex(outThreshBalanced, balancedCoarse)).entries.map(e => e.global_id));
  const draftCoarseIds = new Set((await readIndex(outThreshDraft, draftCoarse)).entries.map(e => e.global_id));
  const draftExactIds = new Set((await readIndex(outThreshDraft, draftExactTile)).entries.map(e => e.global_id));
  const dropped = [...balancedCoarseIds].filter(id => !draftCoarseIds.has(id));
  assert(dropped.length >= 1, 'expected draft to drop a tiny element from the coarse proxy');
  for (const id of dropped) {
    assert(draftExactIds.has(id), `dropped coarse element ${id} must still be present in the exact LOD0 tile`);
  }

  // --- Requirement 5: full-model fallback for a geometry-free model ----------
  const emptyIfc = join(scratch, 'no-geometry.ifc');
  await writeFile(emptyIfc, [
    'ISO-10303-21;',
    'HEADER;',
    "FILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');",
    "FILE_NAME('no-geometry.ifc','2026-07-18T00:00:00',('Vex'),('Vex'),'Vex','Vex','');",
    "FILE_SCHEMA(('IFC2X3'));",
    'ENDSEC;',
    'DATA;',
    '#1=IFCCARTESIANPOINT((0.,0.,0.));',
    '#2=IFCDIRECTION((0.,0.,1.));',
    '#3=IFCDIRECTION((1.,0.,0.));',
    '#4=IFCAXIS2PLACEMENT3D(#1,#2,#3);',
    '#5=IFCLOCALPLACEMENT($,#4);',
    "#6=IFCBUILDINGSTOREY('EMPTYSTOREY00000000001',$,'Empty',$,$,#5,$,$,.ELEMENT.,0.);",
    "#7=IFCPROJECT('EMPTYPROJECT0000000001',$,'Project',$,$,$,$,$,$);",
    "#8=IFCRELAGGREGATES('EMPTYAGG000000000000001',$,$,$,#7,(#6));",
    'ENDSEC;',
    'END-ISO-10303-21;',
    '',
  ].join('\n'));
  const outEmpty = join(scratch, 'empty');
  const emptyManifest = await generate(emptyIfc, outEmpty, 'balanced');
  assert(emptyManifest.tiles.length === 1, 'empty model must publish exactly one tile');
  const emptyTile = emptyManifest.tiles[0];
  assert(emptyTile.tile_id === 'unassigned', 'empty model tile must be unassigned');
  assert(emptyTile.lod === 0 && emptyTile.geometric_error === 0, 'empty model tile must be the exact LOD0 fallback');
  assert(!('group' in emptyTile), 'empty model tile is single-LOD and must not carry a group key');
  assert(emptyTile.semantic_index.entry_count === 0, 'empty model tile must have no entries');
  const emptyGlb = await loadObject(outEmpty, emptyTile.artifact);
  assert(emptyGlb.readUInt32LE(0) === 0x46546c67, 'empty tile GLB is invalid');

  console.log(JSON.stringify({
    schema: 'vex.render-lod-profiles-test/1',
    status: 'passed',
    profiles: {
      balanced: { tiles: balanced.tiles.length, policy_hash: balanced.render_policy.hash.slice(0, 12) },
      draft: { tiles: draft.tiles.length, policy_hash: draft.render_policy.hash.slice(0, 12) },
      accurate: { tiles: accurate.tiles.length, policy_hash: accurate.render_policy.hash.slice(0, 12) },
    },
    lod0_parity: true,
    draft_dropped_from_coarse: dropped.length,
  }));
} finally {
  await rm(scratch, { recursive: true, force: true });
}
