#!/usr/bin/env node
/**
 * Builds one content-addressed GLB tile and its IFC selection index for a
 * Bridge render-artifact staging directory.
 */

import {
  lstat,
  mkdir,
  open,
  readFile,
  readdir,
  rename,
  rm,
  rmdir,
  writeFile,
} from 'node:fs/promises';
import { createHash, randomUUID } from 'node:crypto';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { performance } from 'node:perf_hooks';

const MANIFEST_SCHEMA = 'vex.render-manifest/2';
const SEMANTIC_INDEX_SCHEMA = 'vex.render-semantic-index/1';
const RENDER_POLICY_ID = 'vex.render-policy.storey-first/1';
const TILE_STRATEGY = 'storey-first';
const UNASSIGNED_TILE_ID = 'unassigned';
// web-ifc entity type identifiers used to resolve storey membership directly
// from the IFC spatial hierarchy rather than inferring it from geometry.
const IFC_REL_AGGREGATES = 160246688;
const IFC_BUILDING_STOREY = 3124254112;
const IFC_REL_CONTAINED_IN_SPATIAL_STRUCTURE = 3242617779;
const GLB_MAGIC = 0x46546c67;
const GLB_VERSION = 2;
const GLB_JSON = 0x4e4f534a;
const GLB_BIN = 0x004e4942;

function usage() {
  return `Usage:
  node tools\\vex-render-worker.mjs --ifc <model.ifc> --out <staging-dir> \\
    --project-id <project-id> --commit <64-hex-commit> \\
    --web-ifc-api <web-ifc-api-node.js> --wasm-dir <directory>

Writes manifest.json and content-addressed objects/<sha256> files. The output
directory may be new or empty; it is never overwritten.

Runtime dependency:
  --web-ifc-api must name a Node-target web-ifc-api-node.js module.
  --wasm-dir must contain its matching web-ifc-node.wasm.

The browser viewer's web-ifc-api.js/web-ifc.wasm bundle is not Node-compatible.`;
}

function parseArgs(argv) {
  if (argv.length === 1 && (argv[0] === '--help' || argv[0] === '-h')) {
    return { help: true };
  }

  const values = new Map();
  const expected = new Set(['--ifc', '--out', '--project-id', '--commit', '--web-ifc-api', '--wasm-dir']);
  for (let index = 0; index < argv.length; index += 2) {
    const flag = argv[index];
    const value = argv[index + 1];
    if (!expected.has(flag) || value === undefined || value.startsWith('--') || values.has(flag)) {
      throw new Error('expected all required worker options exactly once');
    }
    values.set(flag, value);
  }

  if (values.size !== expected.size) {
    throw new Error('expected --ifc, --out, --project-id, --commit, --web-ifc-api, and --wasm-dir');
  }

  const projectId = values.get('--project-id').trim();
  const commit = values.get('--commit').trim();
  if (!projectId) throw new Error('--project-id must not be empty');
  if (!/^[0-9a-fA-F]{64}$/.test(commit)) {
    throw new Error('--commit must be a complete 64-character hexadecimal hash');
  }

  return {
    ifc: resolve(values.get('--ifc')),
    out: resolve(values.get('--out')),
    projectId,
    commit,
    webIfcApi: resolve(values.get('--web-ifc-api')),
    wasmDir: resolve(values.get('--wasm-dir')),
  };
}

async function ensureRegularFile(path, label) {
  const stat = await lstat(path);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error(`${label} must be a regular file`);
  }
  return stat;
}

async function ensureOutputDirectory(path) {
  let created = false;
  try {
    await lstat(path);
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
    await mkdir(path);
    created = true;
  }
  const stat = await lstat(path);
  if (!stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error('--out must be a non-symbolic-link directory');
  }
  return created;
}

async function prepareObjectDirectory(out) {
  const path = join(out, 'objects');
  let created = false;
  try {
    await lstat(path);
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error;
    await mkdir(path);
    created = true;
  }
  const stat = await lstat(path);
  if (!stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error('objects must be a non-symbolic-link directory');
  }
  if (!created && (await readdir(path)).length !== 0) {
    throw new Error('objects directory must be empty');
  }
  return { path, created };
}

async function pathExists(path) {
  try {
    await lstat(path);
    return true;
  } catch (error) {
    if (error?.code === 'ENOENT') return false;
    throw error;
  }
}

async function writeNewFileAtomically(directory, filename, bytes, state) {
  const destination = join(directory, filename);
  if (await pathExists(destination)) {
    throw new Error(`refusing to overwrite existing output ${filename}`);
  }

  const temporary = join(directory, `.${filename}.${randomUUID()}.partial`);
  state.temporaryPaths.add(temporary);
  let handle;
  try {
    handle = await open(temporary, 'wx');
    await handle.writeFile(bytes);
    await handle.sync();
    await handle.close();
    handle = undefined;
    if (await pathExists(destination)) {
      throw new Error(`refusing to overwrite existing output ${filename}`);
    }
    await rename(temporary, destination);
    state.temporaryPaths.delete(temporary);
    state.outputPaths.add(destination);
  } finally {
    if (handle) await handle.close().catch(() => {});
    if (state.temporaryPaths.has(temporary)) await rm(temporary, { force: true }).catch(() => {});
  }
}

function sha256(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function numberOrThrow(value, label) {
  if (!Number.isFinite(value)) throw new Error(`${label} contains a non-finite number`);
  return value;
}

function matrixValues(matrix) {
  let values;
  if (Array.isArray(matrix) || ArrayBuffer.isView(matrix)) {
    values = Array.from(matrix);
  } else if (matrix && typeof matrix.size === 'function' && typeof matrix.get === 'function') {
    values = Array.from({ length: matrix.size() }, (_, index) => matrix.get(index));
  } else {
    throw new Error('web-ifc returned an invalid placement transform');
  }
  if (values.length !== 16) throw new Error('web-ifc placement transform is not 4x4');
  return values.map((value, index) => numberOrThrow(value, `transform[${index}]`));
}

function normalTransform(matrix) {
  const a = matrix[0], b = matrix[4], c = matrix[8];
  const d = matrix[1], e = matrix[5], f = matrix[9];
  const g = matrix[2], h = matrix[6], i = matrix[10];
  const A = e * i - f * h;
  const B = c * h - b * i;
  const C = b * f - c * e;
  const D = f * g - d * i;
  const E = a * i - c * g;
  const F = c * d - a * f;
  const G = d * h - e * g;
  const H = b * g - a * h;
  const I = a * e - b * d;
  const determinant = a * A + b * D + c * G;
  if (!Number.isFinite(determinant) || Math.abs(determinant) < 1e-20) {
    throw new Error('web-ifc placement transform is singular');
  }
  const inverse = 1 / determinant;
  // This is the transpose of the inverse linear transform, for normals.
  return [
    A * inverse, B * inverse, C * inverse,
    D * inverse, E * inverse, F * inverse,
    G * inverse, H * inverse, I * inverse,
  ];
}

function normalizedColor(color) {
  const channels = [color?.x, color?.y, color?.z, color?.w];
  if (channels.every(Number.isFinite)) {
    return channels.map(channel => Math.min(1, Math.max(0, channel)));
  }
  return [0.72, 0.72, 0.72, 1];
}

function transformPlacement(vertices, indices, matrix, color) {
  if (vertices.length % 6 !== 0 || indices.length % 3 !== 0) {
    throw new Error('web-ifc returned malformed geometry buffers');
  }
  const vertexCount = vertices.length / 6;

  const positions = new Float32Array(vertexCount * 3);
  const normals = new Float32Array(vertexCount * 3);
  const colors = new Float32Array(vertexCount * 4);
  const normalMatrix = normalTransform(matrix);
  const [red, green, blue, alpha] = normalizedColor(color);
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (let vertex = 0; vertex < vertexCount; vertex += 1) {
    const source = vertex * 6;
    const target = vertex * 3;
    const x = numberOrThrow(vertices[source], 'vertex x');
    const y = numberOrThrow(vertices[source + 1], 'vertex y');
    const z = numberOrThrow(vertices[source + 2], 'vertex z');
    const tx = numberOrThrow(matrix[0] * x + matrix[4] * y + matrix[8] * z + matrix[12], 'transformed x');
    const ty = numberOrThrow(matrix[1] * x + matrix[5] * y + matrix[9] * z + matrix[13], 'transformed y');
    const tz = numberOrThrow(matrix[2] * x + matrix[6] * y + matrix[10] * z + matrix[14], 'transformed z');
    positions[target] = tx;
    positions[target + 1] = ty;
    positions[target + 2] = tz;
    min[0] = Math.min(min[0], tx);
    min[1] = Math.min(min[1], ty);
    min[2] = Math.min(min[2], tz);
    max[0] = Math.max(max[0], tx);
    max[1] = Math.max(max[1], ty);
    max[2] = Math.max(max[2], tz);

    const nx = numberOrThrow(vertices[source + 3], 'normal x');
    const ny = numberOrThrow(vertices[source + 4], 'normal y');
    const nz = numberOrThrow(vertices[source + 5], 'normal z');
    const nnx = normalMatrix[0] * nx + normalMatrix[3] * ny + normalMatrix[6] * nz;
    const nny = normalMatrix[1] * nx + normalMatrix[4] * ny + normalMatrix[7] * nz;
    const nnz = normalMatrix[2] * nx + normalMatrix[5] * ny + normalMatrix[8] * nz;
    const length = Math.hypot(nnx, nny, nnz);
    if (!Number.isFinite(length) || length < 1e-20) {
      throw new Error('web-ifc returned an invalid transformed normal');
    }
    normals[target] = nnx / length;
    normals[target + 1] = nny / length;
    normals[target + 2] = nnz / length;
    const colorOffset = vertex * 4;
    colors[colorOffset] = red;
    colors[colorOffset + 1] = green;
    colors[colorOffset + 2] = blue;
    colors[colorOffset + 3] = alpha;
  }

  const localIndices = new Uint32Array(indices.length);
  for (let index = 0; index < indices.length; index += 1) {
    const localIndex = indices[index];
    if (!Number.isInteger(localIndex) || localIndex < 0 || localIndex >= vertexCount) {
      throw new Error('web-ifc returned an out-of-range geometry index');
    }
    localIndices[index] = localIndex;
  }
  return { positions, normals, colors, indices: localIndices, vertexCount, min, max };
}

function handleExpressId(value) {
  if (typeof value === 'number' && Number.isInteger(value)) return value;
  if (value && typeof value.value === 'number' && Number.isInteger(value.value)) {
    return value.value;
  }
  return undefined;
}

function handleExpressIds(value) {
  const ids = [];
  if (Array.isArray(value)) {
    for (const item of value) {
      const id = handleExpressId(item);
      if (id !== undefined) ids.push(id);
    }
  } else {
    const id = handleExpressId(value);
    if (id !== undefined) ids.push(id);
  }
  return ids;
}

function collectTypeIds(api, modelId, type) {
  const ids = [];
  const vector = api.GetLineIDsWithType(modelId, type);
  const size = vector.size();
  for (let index = 0; index < size; index += 1) ids.push(vector.get(index));
  return ids;
}

function readGlobalId(api, modelId, expressId) {
  try {
    const line = api.GetLine(modelId, expressId, false);
    if (typeof line?.GlobalId?.value === 'string' && line.GlobalId.value.trim()) {
      return line.GlobalId.value;
    }
  } catch {
    // Some geometry-bearing entities have no ordinary IFC line metadata.
  }
  return null;
}

/**
 * Resolve storey membership directly from the IFC spatial hierarchy.
 *
 * Membership is derived only from `IfcRelContainedInSpatialStructure` (which
 * element sits in which spatial node) and `IfcRelAggregates` (how spatial
 * nodes and assemblies decompose). Storey ownership is never inferred from Z
 * bounds, so a tall element that spans several storeys still belongs to its
 * single explicit containment storey.
 */
function buildSpatialResolver(api, modelId) {
  const storeyIds = new Set(collectTypeIds(api, modelId, IFC_BUILDING_STOREY));

  // element (or assembly) express id -> spatial structure express id
  const containedIn = new Map();
  for (const relId of collectTypeIds(api, modelId, IFC_REL_CONTAINED_IN_SPATIAL_STRUCTURE)) {
    let rel;
    try {
      rel = api.GetLine(modelId, relId, false);
    } catch {
      continue;
    }
    const structure = handleExpressId(rel?.RelatingStructure);
    if (structure === undefined) continue;
    for (const elementId of handleExpressIds(rel?.RelatedElements)) {
      if (!containedIn.has(elementId)) containedIn.set(elementId, structure);
    }
  }

  // child express id -> aggregating parent express id (spatial decomposition
  // such as storey -> space, and element decomposition such as assembly -> part)
  const aggregateParent = new Map();
  for (const relId of collectTypeIds(api, modelId, IFC_REL_AGGREGATES)) {
    let rel;
    try {
      rel = api.GetLine(modelId, relId, false);
    } catch {
      continue;
    }
    const parent = handleExpressId(rel?.RelatingObject);
    if (parent === undefined) continue;
    for (const childId of handleExpressIds(rel?.RelatedObjects)) {
      if (!aggregateParent.has(childId)) aggregateParent.set(childId, parent);
    }
  }

  // Walk aggregation upward from a spatial node until a storey is reached.
  const resolveStorey = (nodeId) => {
    const visited = new Set();
    let current = nodeId;
    while (current !== undefined && !visited.has(current)) {
      visited.add(current);
      if (storeyIds.has(current)) return current;
      current = aggregateParent.get(current);
    }
    return undefined;
  };

  // Resolve an element to its containing storey. Explicit spatial containment
  // wins; if the element is only a part of an assembly, follow the aggregation
  // chain to the containing parent.
  const resolveElementStorey = (elementId) => {
    const visited = new Set();
    let current = elementId;
    while (current !== undefined && !visited.has(current)) {
      visited.add(current);
      const structure = containedIn.get(current);
      if (structure !== undefined) return resolveStorey(structure);
      current = aggregateParent.get(current);
    }
    return undefined;
  };

  const storeyMeta = new Map();
  for (const storeyId of storeyIds) {
    const globalId = readGlobalId(api, modelId, storeyId);
    // Stable identity for the tile id: GlobalId when present, otherwise the
    // express id, so ordering and tile names never depend on iteration order.
    const key = globalId ?? `express-${storeyId}`;
    storeyMeta.set(storeyId, { expressId: storeyId, globalId, key });
  }

  return { resolveElementStorey, storeyMeta };
}

// Deterministic element order within a tile: stable GlobalId first, then the
// numeric express id as a tie-breaker for elements lacking a GlobalId.
function compareElements(a, b) {
  const ga = a.globalId ?? '';
  const gb = b.globalId ?? '';
  if (ga !== gb) return ga < gb ? -1 : 1;
  return a.expressId - b.expressId;
}

/**
 * Assemble a single storey tile: concatenate its elements' placements into one
 * GLB buffer, rebasing indices per tile, and emit tile-local semantic entries
 * whose triangle ranges reference this tile's own mesh.
 */
function assembleTile(tileId, elements) {
  const chunks = [];
  const entries = [];
  const bounds = { min: [Infinity, Infinity, Infinity], max: [-Infinity, -Infinity, -Infinity] };
  const totals = { vertexCount: 0, indexCount: 0, triangleCount: 0, placedGeometryCount: 0 };
  for (const element of elements) {
    const triangleStart = totals.triangleCount;
    for (const placement of element.placements) {
      if (totals.vertexCount + placement.vertexCount > 0xffffffff) {
        throw new Error('tile exceeds the GLB unsigned-32-bit vertex limit');
      }
      const rebased = new Uint32Array(placement.indices.length);
      for (let index = 0; index < placement.indices.length; index += 1) {
        rebased[index] = totals.vertexCount + placement.indices[index];
      }
      chunks.push({
        positions: placement.positions,
        normals: placement.normals,
        colors: placement.colors,
        indices: rebased,
      });
      for (let axis = 0; axis < 3; axis += 1) {
        bounds.min[axis] = Math.min(bounds.min[axis], placement.min[axis]);
        bounds.max[axis] = Math.max(bounds.max[axis], placement.max[axis]);
      }
      totals.vertexCount += placement.vertexCount;
      totals.indexCount += placement.indices.length;
      totals.triangleCount += placement.indices.length / 3;
      totals.placedGeometryCount += 1;
    }
    entries.push({
      express_id: element.expressId,
      global_id: element.globalId,
      triangle_ranges: totals.triangleCount === triangleStart
        ? []
        : [{ first_triangle: triangleStart, triangle_count: totals.triangleCount - triangleStart }],
    });
  }
  if (!totals.vertexCount) {
    bounds.min = [0, 0, 0];
    bounds.max = [0, 0, 0];
  }
  const glb = buildGlb(chunks, totals, bounds);
  return { tileId, glb, entries, bounds, totals };
}

function concatenate(TypedArray, chunks, property, totalLength) {
  const result = new TypedArray(totalLength);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk[property], offset);
    offset += chunk[property].length;
  }
  return result;
}

function padToFour(bytes, padding) {
  const paddedLength = (bytes.length + 3) & ~3;
  if (paddedLength === bytes.length) return bytes;
  const padded = Buffer.alloc(paddedLength, padding);
  bytes.copy(padded);
  return padded;
}

function buildGlb(chunks, totals, bounds) {
  const positions = concatenate(Float32Array, chunks, 'positions', totals.vertexCount * 3);
  const normals = concatenate(Float32Array, chunks, 'normals', totals.vertexCount * 3);
  const colors = concatenate(Float32Array, chunks, 'colors', totals.vertexCount * 4);
  const indices = concatenate(Uint32Array, chunks, 'indices', totals.indexCount);
  const positionBytes = Buffer.from(positions.buffer, positions.byteOffset, positions.byteLength);
  const normalBytes = Buffer.from(normals.buffer, normals.byteOffset, normals.byteLength);
  const colorBytes = Buffer.from(colors.buffer, colors.byteOffset, colors.byteLength);
  const indexBytes = Buffer.from(indices.buffer, indices.byteOffset, indices.byteLength);
  const binary = Buffer.concat([positionBytes, normalBytes, colorBytes, indexBytes]);

  const gltf = {
    asset: { version: '2.0', generator: 'vex-render-worker' },
    scene: 0,
    scenes: [{ nodes: totals.vertexCount ? [0] : [] }],
    buffers: [{ byteLength: binary.length }],
  };
  if (totals.vertexCount) {
    gltf.bufferViews = [
      { buffer: 0, byteOffset: 0, byteLength: positionBytes.length, target: 34962 },
      { buffer: 0, byteOffset: positionBytes.length, byteLength: normalBytes.length, target: 34962 },
      { buffer: 0, byteOffset: positionBytes.length + normalBytes.length, byteLength: colorBytes.length, target: 34962 },
      { buffer: 0, byteOffset: positionBytes.length + normalBytes.length + colorBytes.length, byteLength: indexBytes.length, target: 34963 },
    ];
    gltf.accessors = [
      {
        bufferView: 0,
        componentType: 5126,
        count: totals.vertexCount,
        type: 'VEC3',
        min: bounds.min,
        max: bounds.max,
      },
      { bufferView: 1, componentType: 5126, count: totals.vertexCount, type: 'VEC3' },
      { bufferView: 2, componentType: 5126, count: totals.vertexCount, type: 'VEC4' },
      { bufferView: 3, componentType: 5125, count: totals.indexCount, type: 'SCALAR' },
    ];
    gltf.meshes = [{
      primitives: [{
        attributes: { POSITION: 0, NORMAL: 1, COLOR_0: 2 },
        indices: 3,
        mode: 4,
      }],
    }];
    gltf.nodes = [{ mesh: 0 }];
  }

  const json = padToFour(Buffer.from(JSON.stringify(gltf), 'utf8'), 0x20);
  const bin = padToFour(binary, 0);
  const result = Buffer.alloc(12 + 8 + json.length + 8 + bin.length);
  result.writeUInt32LE(GLB_MAGIC, 0);
  result.writeUInt32LE(GLB_VERSION, 4);
  result.writeUInt32LE(result.length, 8);
  result.writeUInt32LE(json.length, 12);
  result.writeUInt32LE(GLB_JSON, 16);
  json.copy(result, 20);
  const binHeader = 20 + json.length;
  result.writeUInt32LE(bin.length, binHeader);
  result.writeUInt32LE(GLB_BIN, binHeader + 4);
  bin.copy(result, binHeader + 8);
  validateGlb(result);
  return result;
}

function validateGlb(glb) {
  if (glb.length < 20 || glb.readUInt32LE(0) !== GLB_MAGIC || glb.readUInt32LE(4) !== GLB_VERSION
    || glb.readUInt32LE(8) !== glb.length) {
    throw new Error('generated GLB header is invalid');
  }
  const jsonLength = glb.readUInt32LE(12);
  if (glb.readUInt32LE(16) !== GLB_JSON || 20 + jsonLength + 8 > glb.length) {
    throw new Error('generated GLB JSON chunk is invalid');
  }
  let document;
  try {
    document = JSON.parse(glb.subarray(20, 20 + jsonLength).toString('utf8').trimEnd());
  } catch {
    throw new Error('generated GLB JSON cannot be parsed');
  }
  const binHeader = 20 + jsonLength;
  if (glb.readUInt32LE(binHeader + 4) !== GLB_BIN
    || binHeader + 8 + glb.readUInt32LE(binHeader) !== glb.length
    || document.asset?.version !== '2.0' || !Array.isArray(document.buffers)) {
    throw new Error('generated GLB structure is invalid');
  }
}

function resource(hash, byteLength, contentType) {
  return {
    uri: `urn:sha256:${hash}`,
    content_type: contentType,
    sha256: hash,
    byte_length: byteLength,
  };
}

function deleteWebIfcObject(value) {
  if (typeof value?.delete === 'function') value.delete();
}

function safeErrorMessage(error) {
  const message = error instanceof Error ? error.message : String(error);
  return message.replace(/[\r\n\t]+/g, ' ').slice(0, 500) || 'render worker failed';
}

async function run(options) {
  const startedAt = performance.now();
  const inputStat = await ensureRegularFile(options.ifc, '--ifc');
  const outputCreated = await ensureOutputDirectory(options.out);
  const state = {
    outputCreated,
    objectDirectoryCreated: false,
    outputPaths: new Set(),
    temporaryPaths: new Set(),
    temporaryModule: undefined,
  };
  let api;
  let modelId;
  let completed = false;

  try {
    if (await pathExists(join(options.out, 'manifest.json'))) {
      throw new Error('refusing to overwrite existing manifest.json');
    }

    const apiSource = options.webIfcApi;
    const wasmPath = join(options.wasmDir, 'web-ifc-node.wasm');
    await ensureRegularFile(apiSource, '--web-ifc-api');
    await ensureRegularFile(wasmPath, '--wasm-dir\\web-ifc-node.wasm');

    state.temporaryModule = join(options.out, `.web-ifc-api.${randomUUID()}.mjs`);
    // web-ifc's Node distribution is CommonJS. Make a temporary `.mjs` copy
    // with only the CommonJS bindings required to import its shipped source.
    const apiBytes = await readFile(apiSource, 'utf8');
    if (!apiBytes.includes('web-ifc-node.wasm') || !apiBytes.includes('ENVIRONMENT_IS_NODE')) {
      throw new Error('--web-ifc-api must be the Node-target web-ifc API, not the browser bundle');
    }
    await writeFile(state.temporaryModule, [
      "import { createRequire } from 'node:module';",
      "import { dirname as pathDirname } from 'node:path';",
      "import { fileURLToPath } from 'node:url';",
      'const require = createRequire(import.meta.url);',
      'const module = { exports: {} };',
      'const exports = module.exports;',
      'const __filename = fileURLToPath(import.meta.url);',
      'const __dirname = pathDirname(__filename);',
      apiBytes,
      'export const IfcAPI = module.exports.IfcAPI;',
      '',
    ].join('\n'), { flag: 'wx' });
    const { IfcAPI } = await import(pathToFileURL(state.temporaryModule).href);
    const importedAt = performance.now();
    api = new IfcAPI();
    await api.Init((file) => join(options.wasmDir, file), true);
    const initializedAt = performance.now();
    const sourceBytes = new Uint8Array(await readFile(options.ifc));
    if (sourceBytes.byteLength === 0) throw new Error('--ifc must not be empty');
    modelId = api.OpenModel(sourceBytes, { COORDINATE_TO_ORIGIN: true, USE_FAST_BOOLS: true });
    const openedAt = performance.now();

    // Storey membership is resolved from the IFC spatial hierarchy before any
    // geometry is grouped, never from geometry Z bounds.
    const spatial = buildSpatialResolver(api, modelId);
    const elementRecords = [];
    const flatMeshes = api.LoadAllGeometry(modelId);
    try {
      for (let meshIndex = 0; meshIndex < flatMeshes.size(); meshIndex += 1) {
        const mesh = flatMeshes.get(meshIndex);
        try {
          const expressId = mesh.expressID;
          if (!Number.isInteger(expressId) || expressId < 0) {
            throw new Error('web-ifc returned an invalid product express ID');
          }

          const placementData = [];
          const placements = mesh.geometries;
          try {
            for (let placementIndex = 0; placementIndex < placements.size(); placementIndex += 1) {
              const placement = placements.get(placementIndex);
              let geometry;
              try {
                geometry = api.GetGeometry(modelId, placement.geometryExpressID);
                const vertices = api.GetVertexArray(geometry.GetVertexData(), geometry.GetVertexDataSize());
                const indices = api.GetIndexArray(geometry.GetIndexData(), geometry.GetIndexDataSize());
                const chunk = transformPlacement(
                  vertices,
                  indices,
                  matrixValues(placement.flatTransformation),
                  placement.color,
                );
                if (chunk.indices.length > 0) placementData.push(chunk);
              } finally {
                deleteWebIfcObject(geometry);
                deleteWebIfcObject(placement);
              }
            }
          } finally {
            deleteWebIfcObject(placements);
          }

          // Spatial-structure nodes (sites, storeys, spaces) and other
          // metadata-only products carry no geometry; they never become a
          // tile or a semantic entry.
          if (placementData.length === 0) continue;

          const globalId = readGlobalId(api, modelId, expressId);
          const storeyExpressId = spatial.resolveElementStorey(expressId);
          const storey = storeyExpressId === undefined
            ? undefined
            : spatial.storeyMeta.get(storeyExpressId);
          elementRecords.push({
            expressId,
            globalId,
            placements: placementData,
            // Elements with no explicit storey containment fall into a single
            // deterministic `unassigned` tile that sorts after every storey.
            tileId: storey ? `storey-${storey.key}` : UNASSIGNED_TILE_ID,
            tileSortKey: storey ? storey.key : '\uffff',
          });
        } finally {
          deleteWebIfcObject(mesh);
        }
      }
    } finally {
      deleteWebIfcObject(flatMeshes);
    }
    const geometryAt = performance.now();

    // Group geometry-bearing elements into one tile per containing storey plus
    // the unassigned tile, then order both tiles and their entries by stable
    // storey/GlobalId identity so the artifact is deterministic.
    const tileGroups = new Map();
    for (const record of elementRecords) {
      let group = tileGroups.get(record.tileId);
      if (!group) {
        group = { tileId: record.tileId, sortKey: record.tileSortKey, elements: [] };
        tileGroups.set(record.tileId, group);
      }
      group.elements.push(record);
    }
    const orderedGroups = Array.from(tileGroups.values()).sort((a, b) => {
      if (a.sortKey !== b.sortKey) return a.sortKey < b.sortKey ? -1 : 1;
      return a.tileId < b.tileId ? -1 : a.tileId > b.tileId ? 1 : 0;
    });
    for (const group of orderedGroups) group.elements.sort(compareElements);

    const assembledTiles = orderedGroups.map(group => assembleTile(group.tileId, group.elements));
    // A model with no geometry (or no spatial hierarchy and no geometry) still
    // publishes one valid, empty unassigned tile.
    if (assembledTiles.length === 0) {
      assembledTiles.push(assembleTile(UNASSIGNED_TILE_ID, []));
    }

    // Content-address every tile GLB and its tile-local semantic index.
    const objectBytesByHash = new Map();
    const manifestTiles = assembledTiles.map(tile => {
      const semanticIndex = {
        schema: SEMANTIC_INDEX_SCHEMA,
        tile_id: tile.tileId,
        coordinate_system: 'web-ifc-y-up',
        triangle_indexing: 'GLB mesh 0 primitive 0; each range is expressed as triangle (not index) offsets.',
        entries: tile.entries,
      };
      const semanticBytes = Buffer.from(JSON.stringify(semanticIndex), 'utf8');
      const glbHash = sha256(tile.glb);
      const semanticHash = sha256(semanticBytes);
      objectBytesByHash.set(glbHash, tile.glb);
      objectBytesByHash.set(semanticHash, semanticBytes);
      tile.glbHash = glbHash;
      tile.semanticHash = semanticHash;
      tile.semanticBytes = semanticBytes;
      return {
        tile_id: tile.tileId,
        lod: 0,
        bounds: tile.bounds,
        geometric_error: 0,
        artifact: resource(glbHash, tile.glb.length, 'model/gltf-binary'),
        semantic_index: {
          schema: SEMANTIC_INDEX_SCHEMA,
          entry_count: tile.entries.length,
          artifact: resource(semanticHash, semanticBytes.length, 'application/json'),
        },
      };
    });
    const encodedAt = performance.now();

    const { path: objectsPath, created: objectDirectoryCreated } = await prepareObjectDirectory(options.out);
    state.objectDirectoryCreated = objectDirectoryCreated;
    // Written in sorted hash order and de-duplicated so identical tile content
    // is stored exactly once under its content address.
    for (const hash of Array.from(objectBytesByHash.keys()).sort()) {
      await writeNewFileAtomically(objectsPath, hash, objectBytesByHash.get(hash), state);
    }

    // The policy identity hashes only renderer configuration (not per-model
    // tiles) so the same policy yields the same identity across models, while
    // a configuration change produces a distinct, non-colliding identity.
    const renderPolicyHash = sha256(Buffer.from(JSON.stringify({
      id: RENDER_POLICY_ID,
      manifest_schema: MANIFEST_SCHEMA,
      semantic_index_schema: SEMANTIC_INDEX_SCHEMA,
      tile_strategy: TILE_STRATEGY,
      spatial_subdivision: false,
      lod_levels: 1,
      coordinate_system: 'web-ifc-y-up',
    }), 'utf8'));
    const artifactId = sha256(Buffer.from(JSON.stringify({
      schema: MANIFEST_SCHEMA,
      project_id: options.projectId,
      commit_hash: options.commit,
      render_policy: { id: RENDER_POLICY_ID, hash: renderPolicyHash },
      tiles: assembledTiles.map(tile => ({
        tile_id: tile.tileId,
        tile: tile.glbHash,
        semantic_index: tile.semanticHash,
      })),
    }), 'utf8'));
    const manifest = {
      schema: MANIFEST_SCHEMA,
      project_id: options.projectId,
      commit_hash: options.commit,
      artifact_id: artifactId,
      generated_at: new Date().toISOString(),
      render_policy: { id: RENDER_POLICY_ID, hash: renderPolicyHash },
      tiles: manifestTiles,
    };
    await writeNewFileAtomically(options.out, 'manifest.json', Buffer.from(JSON.stringify(manifest, null, 2), 'utf8'), state);
    const writtenAt = performance.now();
    completed = true;

    let vertexCount = 0;
    let triangleCount = 0;
    let placedGeometryCount = 0;
    let productCount = 0;
    let outputBytes = 0;
    for (const tile of assembledTiles) {
      vertexCount += tile.totals.vertexCount;
      triangleCount += tile.totals.triangleCount;
      placedGeometryCount += tile.totals.placedGeometryCount;
      productCount += tile.entries.length;
      outputBytes += tile.glb.length + tile.semanticBytes.length;
    }

    const memory = process.memoryUsage();
    return {
      schema: 'vex.render-worker-metrics/1',
      status: 'completed',
      project_id: options.projectId,
      commit_hash: options.commit,
      input_bytes: inputStat.size,
      output_bytes: outputBytes,
      tile_count: assembledTiles.length,
      product_count: productCount,
      placed_geometry_count: placedGeometryCount,
      vertex_count: vertexCount,
      triangle_count: triangleCount,
      tiles: assembledTiles.map(tile => ({
        tile_id: tile.tileId,
        entry_count: tile.entries.length,
        vertex_count: tile.totals.vertexCount,
        triangle_count: tile.totals.triangleCount,
        glb_sha256: tile.glbHash,
        semantic_index_sha256: tile.semanticHash,
      })),
      timings_ms: {
        import_module: Math.round(importedAt - startedAt),
        initialize: Math.round(initializedAt - importedAt),
        open_model: Math.round(openedAt - initializedAt),
        extract_geometry: Math.round(geometryAt - openedAt),
        encode_glb_and_index: Math.round(encodedAt - geometryAt),
        write_artifacts: Math.round(writtenAt - encodedAt),
        total: Math.round(writtenAt - startedAt),
      },
      memory_bytes: { rss: memory.rss, heap_used: memory.heapUsed, external: memory.external },
    };
  } finally {
    if (api?.wasmModule && Number.isInteger(modelId)) {
      await Promise.resolve(api.CloseModel(modelId)).catch(() => {});
    }
    if (api?.wasmModule) await Promise.resolve(api.Dispose()).catch(() => {});
    if (state.temporaryModule) await rm(state.temporaryModule, { force: true }).catch(() => {});
    if (!completed) {
      for (const path of state.temporaryPaths) await rm(path, { force: true }).catch(() => {});
      for (const path of state.outputPaths) await rm(path, { force: true }).catch(() => {});
      if (state.objectDirectoryCreated) await rmdir(join(options.out, 'objects')).catch(() => {});
      if (state.outputCreated) await rmdir(options.out).catch(() => {});
    }
  }
}

try {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    console.log(usage());
  } else {
    console.log(JSON.stringify(await run(options)));
  }
} catch (error) {
  console.error(JSON.stringify({
    schema: 'vex.render-worker-metrics/1',
    status: 'failed',
    error: safeErrorMessage(error),
  }));
  process.exitCode = 1;
}
