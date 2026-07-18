#!/usr/bin/env node
/**
 * Focused behavioural test for deterministic storey-first tiling.
 *
 * It drives the real render worker against a multi-storey IFC fixture and
 * proves the properties the epic requires:
 *   - one tile per containing storey plus a single unassigned tile;
 *   - exact GlobalId membership resolved from the IFC spatial hierarchy
 *     (IfcRelContainedInSpatialStructure + IfcRelAggregates), including an
 *     element reached only through a space aggregated under its storey;
 *   - tall/multi-storey elements stay whole in their explicit storey;
 *   - deterministic tile ids, ordering, and content-addressed identity across
 *     repeated runs;
 *   - semantic entry triangle counts match the tile GLB;
 *   - safe handling of a model with storeys but no geometry.
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
const webIfcDir = join(root, 'crates', 'vex-bridge', 'assets', 'render-worker', 'web-ifc');
const api = join(webIfcDir, 'web-ifc-api-node.js');
const commit = 'e'.repeat(64);

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

async function generate(ifc, out) {
  await run(node, [
    worker,
    '--ifc', ifc,
    '--out', out,
    '--project-id', 'storey-tiling-project',
    '--commit', commit,
    '--web-ifc-api', api,
    '--wasm-dir', webIfcDir,
  ]);
  return JSON.parse(await readFile(join(out, 'manifest.json'), 'utf8'));
}

async function loadIndex(out, tile) {
  const bytes = await readFile(join(out, 'objects', tile.semantic_index.artifact.sha256));
  return JSON.parse(bytes.toString('utf8'));
}

function glbTriangleCount(glb) {
  const jsonLength = glb.readUInt32LE(12);
  const gltf = JSON.parse(glb.subarray(20, 20 + jsonLength).toString('utf8').trim());
  const primitive = gltf.meshes?.[0]?.primitives?.[0];
  if (!primitive) return { triangles: 0, hasColor: true };
  return {
    triangles: gltf.accessors[primitive.indices].count / 3,
    hasColor: primitive.attributes?.COLOR_0 !== undefined,
  };
}

const scratch = await mkdtemp(join(root, '.render-storey-test-'));
try {
  const outA = join(scratch, 'run-a');
  const outB = join(scratch, 'run-b');
  const manifest = await generate(fixture, outA);
  const manifestB = await generate(fixture, outB);

  // Schema and policy identity.
  assert(manifest.schema === 'vex.render-manifest/2', 'manifest must be v2');
  assert(manifest.render_policy?.id === 'vex.render-policy.storey-first/1', 'unexpected render policy id');
  assert(!('semantic_index' in manifest), 'v2 manifest must not carry a manifest-global semantic index');

  // Deterministic tile ids and ordering.
  const tileIds = manifest.tiles.map(tile => tile.tile_id);
  assert(
    JSON.stringify(tileIds) === JSON.stringify([
      'storey-STOREYA000000000000000',
      'storey-STOREYB000000000000000',
      'unassigned',
    ]),
    `unexpected tile ordering: ${JSON.stringify(tileIds)}`,
  );

  // Determinism across independent runs: identical artifact identity and every
  // content-addressed object hash.
  assert(manifest.artifact_id === manifestB.artifact_id, 'artifact_id is not deterministic');
  for (let index = 0; index < manifest.tiles.length; index += 1) {
    assert(
      manifest.tiles[index].artifact.sha256 === manifestB.tiles[index].artifact.sha256,
      `tile ${index} GLB hash is not deterministic`,
    );
    assert(
      manifest.tiles[index].semantic_index.artifact.sha256
        === manifestB.tiles[index].semantic_index.artifact.sha256,
      `tile ${index} semantic index hash is not deterministic`,
    );
  }

  const membership = {};
  for (const tile of manifest.tiles) {
    // Validate the content-addressed GLB.
    const glb = await readFile(join(outA, 'objects', tile.artifact.sha256));
    assert(sha256(glb) === tile.artifact.sha256, `tile ${tile.tile_id} GLB digest mismatch`);
    assert(glb.readUInt32LE(0) === 0x46546c67, `tile ${tile.tile_id} bad GLB magic`);
    assert(tile.bounds.min.every(Number.isFinite) && tile.bounds.max.every(Number.isFinite),
      `tile ${tile.tile_id} bounds are not finite`);
    const { triangles, hasColor } = glbTriangleCount(glb);
    assert(hasColor, `tile ${tile.tile_id} lost placement colors`);

    // Validate the tile-local semantic index against its own GLB.
    const index = await loadIndex(outA, tile);
    assert(index.tile_id === tile.tile_id, `semantic index tile_id mismatch for ${tile.tile_id}`);
    assert(index.entries.length === tile.semantic_index.entry_count, `entry_count mismatch for ${tile.tile_id}`);
    const mappedTriangles = index.entries
      .flatMap(entry => entry.triangle_ranges || [])
      .reduce((sum, range) => sum + range.triangle_count, 0);
    assert(mappedTriangles === triangles,
      `tile ${tile.tile_id} semantic triangles (${mappedTriangles}) != GLB triangles (${triangles})`);
    membership[tile.tile_id] = index.entries.map(entry => entry.global_id);
  }

  // Exact GlobalId membership, in deterministic GlobalId order.
  assert(
    JSON.stringify(membership['storey-STOREYA000000000000000'])
      === JSON.stringify([
        'COLUMNC100000000000000', // reached via a space aggregated under storey A
        'TALLT10000000000000000', // tall element spanning both storeys stays whole here
        'WALLA10000000000000000',
      ]),
    `unexpected storey A membership: ${JSON.stringify(membership['storey-STOREYA000000000000000'])}`,
  );
  assert(
    JSON.stringify(membership['storey-STOREYB000000000000000']) === JSON.stringify(['WALLB10000000000000000']),
    `unexpected storey B membership: ${JSON.stringify(membership['storey-STOREYB000000000000000'])}`,
  );
  assert(
    JSON.stringify(membership.unassigned) === JSON.stringify(['UNASSN1000000000000000']),
    `unexpected unassigned membership: ${JSON.stringify(membership.unassigned)}`,
  );

  // The tall element must remain a single whole entry, not split across tiles.
  const tallOccurrences = Object.values(membership)
    .flat()
    .filter(globalId => globalId === 'TALLT10000000000000000').length;
  assert(tallOccurrences === 1, 'tall element must belong to exactly one storey and stay whole');

  // Safety: a model with storeys but no geometry publishes one empty
  // unassigned tile rather than failing.
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
  const emptyOut = join(scratch, 'empty');
  const emptyManifest = await generate(emptyIfc, emptyOut);
  assert(emptyManifest.tiles.length === 1, 'empty model must publish exactly one tile');
  assert(emptyManifest.tiles[0].tile_id === 'unassigned', 'empty model tile must be unassigned');
  assert(emptyManifest.tiles[0].semantic_index.entry_count === 0, 'empty model tile must have no entries');
  const emptyGlb = await readFile(join(emptyOut, 'objects', emptyManifest.tiles[0].artifact.sha256));
  assert(emptyGlb.readUInt32LE(0) === 0x46546c67, 'empty tile GLB is invalid');

  console.log(JSON.stringify({
    schema: 'vex.render-storey-tiling-test/1',
    status: 'passed',
    tiles: tileIds,
    storey_a_members: membership['storey-STOREYA000000000000000'],
    storey_b_members: membership['storey-STOREYB000000000000000'],
    unassigned_members: membership.unassigned,
  }));
} finally {
  await rm(scratch, { recursive: true, force: true });
}
