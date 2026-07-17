#!/usr/bin/env node
/**
 * Measure server-side web-ifc geometry extraction before committing to a
 * production render worker. It deliberately emits metrics only: the renderer
 * must still prove its GLB/tile and semantic-index output against the Bridge
 * artifact contract before it can be enabled for users.
 *
 * Usage:
 *   node tools/ifc-render-spike.mjs path\to\model.ifc
 */

import { readFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { performance } from 'node:perf_hooks';

const [input] = process.argv.slice(2);
if (!input || input === '--help' || input === '-h') {
  console.log('Usage: node tools/ifc-render-spike.mjs <model.ifc>');
  process.exit(input ? 0 : 2);
}

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const webIfcDir = join(repoRoot, 'crates', 'vex-bridge', 'assets', 'render-worker', 'web-ifc');
const apiSource = join(webIfcDir, 'web-ifc-api-node.js');
const wasmDir = `${webIfcDir}${process.platform === 'win32' ? '\\' : '/'}`;
const require = createRequire(import.meta.url);

function deleteWebIfcObject(value) {
  if (typeof value?.delete === 'function') value.delete();
}

let api;
let modelId;
try {
  // The browser API cannot initialize in Node. Use the matching Node-target
  // web-ifc pair shipped specifically for the isolated artifact worker.
  const { IfcAPI } = require(apiSource);
  const bytes = new Uint8Array(await readFile(resolve(input)));
  const startedAt = performance.now();
  api = new IfcAPI();
  await api.Init((file) => join(wasmDir, file), true);
  const initializedAt = performance.now();
  modelId = api.OpenModel(bytes, { COORDINATE_TO_ORIGIN: true, USE_FAST_BOOLS: true });
  const openedAt = performance.now();
  const flatMeshes = api.LoadAllGeometry(modelId);

  let placedGeometryCount = 0;
  let vertexCount = 0;
  let triangleCount = 0;
  const expressIds = new Set();
  for (let meshIndex = 0; meshIndex < flatMeshes.size(); meshIndex += 1) {
    const mesh = flatMeshes.get(meshIndex);
    expressIds.add(mesh.expressID);
    const placed = mesh.geometries;
    for (let placedIndex = 0; placedIndex < placed.size(); placedIndex += 1) {
      const placement = placed.get(placedIndex);
      const geometry = api.GetGeometry(modelId, placement.geometryExpressID);
      const vertices = api.GetVertexArray(geometry.GetVertexData(), geometry.GetVertexDataSize());
      const indices = api.GetIndexArray(geometry.GetIndexData(), geometry.GetIndexDataSize());
      // web-ifc vertices are position + normal tuples (six f32 values).
      vertexCount += vertices.length / 6;
      triangleCount += indices.length / 3;
      placedGeometryCount += 1;
      deleteWebIfcObject(geometry);
    }
    deleteWebIfcObject(placed);
    deleteWebIfcObject(mesh);
  }
  deleteWebIfcObject(flatMeshes);
  const geometryLoadedAt = performance.now();

  let globalIdCount = 0;
  for (const expressId of expressIds) {
    const line = api.GetLine(modelId, expressId, false);
    if (line?.GlobalId?.value) globalIdCount += 1;
  }
  const identityMappedAt = performance.now();
  const memory = process.memoryUsage();

  console.log(JSON.stringify({
    schema: 'vex.render-spike/1',
    input_bytes: bytes.byteLength,
    mesh_count: expressIds.size,
    placed_geometry_count: placedGeometryCount,
    vertex_count: vertexCount,
    triangle_count: triangleCount,
    global_id_count: globalIdCount,
    timings_ms: {
      initialize: Math.round(initializedAt - startedAt),
      open_model: Math.round(openedAt - initializedAt),
      load_geometry: Math.round(geometryLoadedAt - openedAt),
      map_identities: Math.round(identityMappedAt - geometryLoadedAt),
      total: Math.round(identityMappedAt - startedAt)
    },
    memory_bytes: {
      rss: memory.rss,
      heap_used: memory.heapUsed,
      external: memory.external
    }
  }));
} finally {
  if (api?.wasmModule && Number.isInteger(modelId)) api.CloseModel(modelId);
  if (api?.wasmModule && api.Dispose) api.Dispose();
}
