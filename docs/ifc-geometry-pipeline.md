# IFC Geometry Pipeline

## Purpose

This document describes what happens when a user imports an IFC file through
Vex Atlas or drops it into a Vex Bridge inbox. It distinguishes the two
purposes served by the system:

- **Vex** creates a durable, semantic version of the IFC model and detects
  meaningful geometry changes.
- **The dashboard viewer** creates transient Three.js meshes so a user can
  inspect the model in 2D and 3D.

Vex does not persist GPU-ready meshes. Persisting IFC semantics instead keeps
version history compact, deterministic, and suitable for semantic diffing.

## System Components

```text
                       raw IFC bytes
CAD export / dashboard ───────────────► Vex Bridge inbox
                                              │
                                              │ stable-file watcher
                                              ▼
                                         vex import
                                              │
                           IFC parser → IFC graph → canonical geometry hashes
                                              │
                                              ▼
                                   Vex tree/blob objects + commit
                                              │
                             checkout canonical IFC for a selected commit
                                              ▼
                             web-ifc → Three.js geometry → WebGL viewer
```

| Component | Responsibility |
| --- | --- |
| Dashboard | Uploads IFC files, shows a local preview, requests a committed model, and overlays diff results. |
| Vex Bridge | Validates and stores uploads, watches inboxes, routes files, invokes the `vex` CLI, records activity, and serves committed IFC snapshots. |
| `vex` CLI | Parses IFC, constructs and hashes the semantic graph, writes staged objects, creates commits, produces diffs, and materializes commits with `checkout`. |
| `vex-ifc-parser` | Streams ISO 10303-21 / STEP entities from the source file. |
| `vex-graph` and `vex-geometry` | Convert entities and references to a graph, compute canonical geometry hashes, and propagate them into semantic identity. |
| web-ifc / IFCLoader | Parses IFC again in the browser and tessellates it into Three.js buffers for display and picking. |

## Ingestion Paths

There are two user-facing entry points, but both converge on the same
background import pipeline.

### Dashboard upload

1. The user chooses an `.ifc` file.
2. The dashboard starts `loadLocalFile()` immediately. It reads the file into
   an `ArrayBuffer` and sends it to web-ifc for a local, temporary preview.
3. In parallel, the dashboard posts the raw bytes to:

   ```text
   POST /v1/projects/:project_id/inbox
   ```

4. The request includes an access token and the original filename in
   `X-Vex-Filename`.

### Watched folder

An IFC copied or exported directly into a configured project inbox is detected
by the filesystem watcher. The watcher invokes the same pipeline used after a
dashboard upload.

## Upload Validation and Safe Handoff

`handle_project_inbox_upload` protects the watcher from partial or invalid
uploads:

1. It resolves the project to its configured inbox.
2. It restricts request bodies to 512 MiB.
3. It rejects empty payloads.
4. It verifies that the beginning of the payload contains the required
   `ISO-10303-21;` STEP header.
5. It writes the upload to a hidden temporary file, then atomically renames it
   to an `.ifc` filename in the inbox.

The temporary name does not match the watcher's IFC pattern, so the watcher
only sees a complete file.

## Background Import and Commit

After a watcher event, Vex Bridge:

1. Waits for the file to settle. It checks metadata at 250 ms intervals, for
   up to four attempts.
2. Computes a content hash and skips a duplicate already imported into the
   same project.
3. Reads IFC intake metadata using `vex ifc-intake`. Project GUID routing is
   authoritative: if the configured project requires a GUID and intake fails,
   Bridge refuses to route the file rather than using its local fallback.
4. Initializes the local Vex repository and remote configuration if required.
5. Runs:

   ```text
   vex --json import <file>
   vex --json commit -m <message> --author <name> --email <email>
   ```

6. Archives the processed source IFC, records its content hash and commit in
   Bridge state, and places the commit in the local ready-to-push ledger.

Importing creates a local commit. Pushing that commit to the remote is a
separate, user-controlled action.

## Semantic IFC Representation

`vex import` opens the IFC as a buffered stream. The parser reads the STEP
preamble and then emits entities one at a time. `GraphBuilder` turns this into
an `IfcGraph`:

- every retained IFC entity becomes a graph node;
- scalar STEP values become node properties;
- STEP references become typed graph edges with their argument slot and list
  position preserved;
- an IFC `GlobalId`, when present, is captured as the durable rooted identity;
- normalization-profile rules can omit configured entity types and references
  to them.

The resulting graph is persisted as Vex tree and blob objects. Blobs retain
entity type, STEP ID, GlobalId, and properties; tree edges retain the semantic
relationships. This is not a saved triangulation or a serialized Three.js
scene.

## Geometry Canonicalization

Geometry is represented during import by stable hashes rather than render
meshes. `vex-graph` recognizes supported shape-bearing IFC entities, including:

- rectangular, circular, and arbitrary closed profiles;
- blocks and right circular cylinders;
- extruded area solids;
- triangulated and polygonal face sets;
- indexed polygonal faces, including voids;
- faceted BReps;
- 2D and 3D Cartesian point lists.

The geometry functions normalize representation details that should not affect
semantic identity:

- linear dimensions and coordinates are quantized using the active tolerance;
- closed rings are made invariant to starting vertex and winding direction;
- point sets are hashed as sorted quantized multisets;
- faces are represented from their resolved points rather than raw,
  exporter-specific indices;
- void rings and face collections are sorted canonically;
- extrusion directions are normalized and angularly quantized.

If a supported shape is malformed or unrecognized, geometry extraction returns
no geometry hash. Vex then safely falls back to normal structural/property
hashing rather than treating invalid geometry as a successful canonical shape.

## Geometry in Semantic Identity and Diffs

For every recognized shape node, its canonical geometry hash is folded into
the node's seed hash. Properties already represented by the canonical shape
hash, such as raw coordinate or index lists, are excluded from the raw property
hash. This prevents harmless exporter changes, such as reordered vertex
tables, from appearing as model changes.

The seed hashes are then refined through the IFC graph with a
Weisfeiler-Lehman-style Merkle pass. A local geometry edit therefore changes
the shape node hash and propagates through referenced relationships to the
owning IFC product.

The visual-diff layer suppresses anonymous internal entities, such as point
lists and face loops, from the primary element list. Their changes are instead
attributed to the rooted product that owns the changed geometry and exposed as
a shape/geometry change.

## Commit-Exact Rendering

When the user selects a commit, the dashboard does not render whichever source
IFC happens to remain in the inbox. It requests:

```text
GET /v1/projects/:project_id/ifc/:commit
```

Bridge resolves the requested commit and runs `vex checkout` into a cache under:

```text
<project>/.vex/cache/ifc/
```

Full 64-character commit hashes are immutable and may reuse the cached IFC.
Short references, branches, tags, and abbreviated hashes are materialized
again. The endpoint serves the resulting IFC bytes only after validating that
the resolved file remains inside the project directory and has an `.ifc`
extension.

`vex checkout` materializes a canonical IFC text representation from the
committed semantic graph. It is semantically commit-exact, but it is not
byte-identical to the originally uploaded IFC: its headers are minimal and
formatting/order may differ.

## Browser Geometry Loading

The dashboard passes the returned IFC bytes to `IFCLoader`:

1. `IFCLoader` loads web-ifc and configures its bundled WASM assets.
2. It attempts to use the bundled `IFCWorker`, avoiding main-thread parsing
   when worker setup succeeds.
3. It enables coordinate-to-origin and fast boolean settings.
4. web-ifc parses the IFC and tessellates its geometry into Three.js buffers.
5. The viewer rotates the loaded model by 90 degrees about X because web-ifc
   produces Y-up geometry while the dashboard is authored as Z-up.
6. The viewer fits cameras to the model bounds, derives storey views, applies
   plan/model clipping, and uses web-ifc Express IDs for raycast selection and
   property lookup.

The same parsing path is used for the immediate local preview and for a
committed model. The local preview is replaced by the selected commit's model
after import completion.

## Timing and User Experience

There is no universal import duration. The main variables are source file
size, STEP entity count, shape complexity, local disk throughput, CPU speed,
and browser/WebGL capacity.

| Phase | Timing behavior |
| --- | --- |
| File settling | Usually about 250 ms; at most about one second under the current four-attempt policy. |
| Semantic import | Reported by `vex import` as `parse`, `persist`, and `total` milliseconds. Bridge logs these values. |
| Commit | Runs after import; normally small compared with parsing and object persistence. |
| Dashboard completion notice | The UI polls recent activity every two seconds, so it can report a completed import up to two seconds after the commit exists. |
| Local or committed viewer load | Depends on web-ifc parsing, tessellation, Three.js buffer allocation, and GPU upload. It is independent of semantic import and can complete before or after it. |

Ordinary IFC models should normally feel like a seconds-scale operation. Large
or highly detailed models can require minutes, particularly when the browser
must tessellate and upload a large amount of geometry. The dashboard's
two-hour tracking deadline is a safeguard for unusually long jobs, not an
expected processing target.

## Failure Boundaries

| Failure | Result |
| --- | --- |
| Upload is empty, too large, or not STEP IFC | HTTP error; the file is not placed in the inbox. |
| File is still being written | The watcher waits for it to settle before import. |
| Same content already imported for this project | The import is skipped and the file is archived. |
| Project GUID does not match the configured route | The file is skipped; it is not imported into the wrong project. |
| `vex ifc-intake` fails while GUID routing is required | The import fails closed to avoid incorrect routing. |
| `vex import` or commit fails | Bridge records an error activity; no successful commit is claimed. |
| Browser preview fails | Upload and semantic import continue; the dashboard reports that preview failed. |
| Committed IFC cannot be materialized or loaded | The viewer shows an IFC rendering error; semantic history remains intact. |

## Design Consequences

- Rendering and versioning are deliberately decoupled. A renderer limitation
  does not invalidate a semantic commit, and a semantic import does not depend
  on a user's GPU.
- Geometry comparison is based on canonical IFC semantics, not unstable
  triangle order or browser-generated buffers.
- A historical commit can be rendered after the original inbox file has been
  archived because it is reconstructed from committed Vex objects.
- A selected commit uses a validated derived render artifact when available;
  the raw IFC/web-ifc path remains the fallback while an artifact is queued,
  failed, or needed for visual-diff overlays.
- Render artifacts remain derived data and never replace the canonical IFC
  graph as the source of truth.

## Derived Render Artifact Foundation

Vex Bridge generates a derived artifact asynchronously after a semantic commit
without affecting that commit. The worker checks out the exact canonical IFC,
uses the bundled Node-target `web-ifc` 0.0.77 runtime to tessellate it, then
publishes a validated set of GLB tiles and semantic indexes. Node.js must be
available as `node` or configured through `node_bin`; a missing runtime safely
leaves the raw IFC fallback available.

The worker emits a v2 (`vex.render-manifest/2`) artifact using the
`vex.render-policy.storey-first/1` policy. It preserves placement transforms,
normals, placement colors, triangle ranges, Express IDs, and GlobalIds exactly
as the earlier full-model rendition did. The dashboard loads these tiles before
it downloads or tessellates the raw IFC, and resolves artifact clicks through
the tile-local semantic triangle ranges.

#### Storey-first tiling

The worker groups geometry into one GLB tile per building storey, plus a single
`unassigned` tile, instead of one full-model tile:

- **Membership is spatial, never geometric.** Each geometry-bearing product is
  mapped to its storey by walking the IFC spatial hierarchy through
  `IfcRelContainedInSpatialStructure` (which element sits in which spatial node)
  and `IfcRelAggregates` (how spatial nodes and assemblies decompose). An
  element contained in a space is attributed to the storey that aggregates the
  space, and an assembly part is attributed through its aggregating parent.
  Storey ownership is **never** inferred from a mesh's Z bounds.
- **Elements stay whole.** A tall or multi-storey element belongs entirely to
  its single explicit containment storey and is never split across tiles.
- **Unassigned geometry is isolated.** Geometry with no resolvable storey (no
  spatial hierarchy, or containment only in a site/building) is collected into
  the one `unassigned` tile.
- **Deterministic identity and ordering.** A tile id is `storey-<GlobalId>`
  (falling back to `storey-express-<id>` only when a storey lacks a GlobalId),
  or `unassigned`. Tiles are ordered by that stable storey identity with the
  unassigned tile last, and entries within a tile are ordered by GlobalId then
  Express ID. The same commit therefore produces byte-identical,
  content-addressed tiles and the same `artifact_id` on every run.
- **Per-tile correctness.** Each tile carries its own local bounds, its own GLB
  mesh with rebased indices, and a tile-local semantic index
  (`RenderTileDescriptor::semantic_index`) whose triangle ranges reference that
  tile's mesh. Vertex and triangle counts in the semantic entries reflect the
  tile GLB, and material/color output matches the previous full-model rendition.

#### Limitations

- **No spatial subdivision.** Tiling is storey-granular only; there is no
  quadtree/octree spatial partitioning within a storey.
- **No level of detail.** Every tile is emitted at LOD 0 with geometric error
  `0`; there is no mesh simplification or LOD pyramid yet.
- **No property hydration.** Only Express IDs, GlobalIds, and triangle ranges
  are indexed; richer property payloads are not embedded.
- **Empty and hierarchy-free models are safe.** Storeys that contain no
  geometry produce no tile, and a model with no geometry (or no spatial
  hierarchy at all) still publishes exactly one valid, empty `unassigned` tile.

Only v2 artifacts are emitted. Published v1 (`vex.render-manifest/1`) artifacts
with a single manifest-global semantic index remain readable and valid, so they
never need regeneration.

### Current API

For a complete 64-character commit hash, Bridge exposes:

| Endpoint | Meaning |
| --- | --- |
| `GET /v1/projects/:id/render/:commit/status` | Returns `queued`, `building`, `failed`, `not_requested`, or a validated `ready` manifest. |
| `GET /v1/projects/:id/render/:commit/manifest` | Returns the validated manifest when an artifact is ready. |
| `GET /v1/projects/:id/render/:commit/objects/:sha256` | Returns one manifest-declared, SHA-256-verified binary object. Supports a single HTTP byte range (see [Range requests](#range-requests)). |

Artifacts live below:

```text
<project>/.vex/cache/render/<full-commit-hash>/
  manifest.json                 # worker input, retained for diagnostics
  manifest.validated.json       # Bridge-owned manifest served to clients
  objects/<sha256>
  .accessed                     # Bridge-owned last-serve timestamp (LRU input)
```

The manifest and semantic-index schemas are versioned. Each tile describes
its bounds, LOD, geometric error, content type, byte length, and digest. Tile
and semantic-index objects are served only if they are declared by a valid
manifest and their bytes match the declared SHA-256. Complete commit hashes,
immutable cache headers, and ETags prevent a derived render result from being
mistakenly reused for another revision.

### Publishing contract

A worker obtains a staging directory from
`render_artifact::create_artifact_staging_dir`, writes the manifest and object
files, then calls `publish_staged_artifact`. Publishing verifies:

- the manifest schema and requested project/commit identity;
- unique tile identifiers and finite bounds;
- every declared object's byte length and SHA-256;
- staging-directory ownership inside Bridge's cache.

Only after validation does Bridge atomically rename the directory into the
immutable commit cache. A malformed or interrupted worker cannot publish a
partial artifact, and no render-generation failure changes the IFC import or
commit outcome.

The production worker is deliberately isolated behind this contract. Its
Node-target web-ifc implementation can be replaced by a native engine after
profiling representative models, provided replacement output preserves the
same artifact identity and GlobalId/Vex mappings.

### Range requests

Render objects are immutable and content-addressed, so the object endpoint
supports HTTP range requests for progressive tile streaming and interrupted
download resumption. Every object response — full or partial — advertises
`Accept-Ranges: bytes`, the immutable `"<sha256>"` ETag, and long-lived
immutable cache headers.

- The object's bytes are always read in full and re-verified against the
  manifest-declared SHA-256 *before* any slice is returned, so a range serve
  carries exactly the same integrity, token, and manifest-membership
  guarantees as a full fetch.
- A single satisfiable `Range: bytes=start-end` (including open-ended
  `bytes=start-` and suffix `bytes=-N` forms) returns `206 Partial Content`
  with a `Content-Range: bytes start-end/total` and the exact slice length. An
  end past the object is clamped to the final byte.
- A syntactically valid but unsatisfiable range (start at or beyond the length,
  a zero-length suffix, or any range against an empty object) returns
  `416 Range Not Satisfiable` with `Content-Range: bytes */total`.
- Multi-range sets (`bytes=a-b,c-d`), unknown units, and malformed ranges are
  made safe by ignoring the header and returning the full `200 OK` body, as
  permitted by RFC 7233. `multipart/byteranges` is intentionally not emitted.

Full-object fetches keep their existing behavior and API: a request with no
`Range` header returns the whole object with `200 OK`.

### Cache capacity and eviction

Each watched project's render cache is bounded by
`render_cache_max_bytes` in `config.toml` (default 2 GiB; values below a 64 MiB
floor are clamped up so a typo cannot make the cache thrash). After a worker
atomically publishes a new artifact, Bridge measures total completed-artifact
usage and, if it exceeds the cap, evicts the **least-recently-served completed
artifacts** until usage is back under the limit.

Least-recently-used ordering is driven by the Bridge-owned `.accessed` marker,
which is updated **only after a valid, permitted serve that returns object
bytes** (a `200` or `206`; never a `416`, and never an unauthorized or
integrity-failed request). Artifacts that have never been served fall back to
their `manifest.validated.json` modification time.

Eviction is deliberately conservative — it never removes:

- an artifact with an in-flight serve (guarded for the duration of the read);
- the project's current commit (its latest recorded IFC snapshot, i.e. HEAD);
- the just-published commit, or any commit a worker is still building
  (`queued`/`building`);
- staging directories (`.<hash>-<uuid>.partial`) or the embedded render
  runtime.

Only directories named by a complete commit hash that contain the Bridge-owned
validated manifest are eviction candidates, symlinks are never followed, and
each directory's containment inside the cache root is re-verified immediately
before removal. Because protected artifacts are never deleted, the cap is a
target rather than a hard ceiling: if live data alone exceeds it, usage stays
above the cap rather than evicting an in-use artifact. Eviction runs after —
and never interferes with — the atomic publish rename, and a pruning failure
never changes the published artifact, the commit, or the job outcome. When an
evicted commit is requested again, the object endpoint returns `404` and the
raw IFC/web-ifc fallback path regenerates it on demand.
