# Vex Atlas production-readiness roadmap

## Purpose and product boundary

Vex Atlas is an IFC semantic-versioning and construction-review product. The
canonical Vex graph and commit history remain authoritative; render artifacts,
indexes, previews, and cached geometry are derived and replaceable.

The initial enterprise product is:

- immutable, provenance-rich IFC packages and commit-exact reviews;
- progressive, artifact-backed visualisation of large qualified IFC models;
- named model federations, BCF-linked issues, and approval of information
  containers;
- a managed desktop client with offline submission, diagnostics, and
  organization policy;
- a cloud control plane for tenancy, identity, authorization, audit, and
  package metadata.

It is not initially a native authoring or automatic design-merging product.
Do not claim native Revit, Archicad, Rhino, Navisworks, or arbitrary IFC4x3
round trips until each claimed schema, MVD, exporter, and workflow is qualified.

## Non-negotiable system invariants

1. A render artifact never becomes the source of truth. It must be
   commit-exact, versioned, validated, immutable, and regenerable.
2. Every rendered element retains a mapping from rendered instance to GlobalId,
   Vex identity, and the exact commit.
3. Storey isolation uses IFC containment, not a geometric Z-range heuristic.
   Section clipping is a separate visual function.
4. Tenants are isolated at authorization, API, storage-prefix, encryption-key,
   and query boundaries. Authorization is deny-by-default.
5. Original IFC, canonical Vex checkout, and qualified interoperability export
   are distinct artifacts and must be labelled as such.
6. Desktop agents are local-first and offline-capable; collaboration occurs
   through intentional submitted and published packages, not shared folders.
7. No customer IFC or property data is used for telemetry, benchmarks, or
   support without explicit policy, consent, and retention controls.

## Program sequence

### M0 - Decisions, baseline, and release safety

**Outcome:** a measurable baseline and accountable external dependencies before
large implementation begins.

- Establish benchmark packs that are licensed, anonymized, or synthetic. Record
  IFC schema/MVD/exporter, GlobalId mappings, expected diffs, hardware, OS,
  GPU, memory, and P50/P95 timings.
- Instrument current import, artifact generation, first visible frame, first
  selectable element, active-storey load, commit switch, CPU/GPU memory, and
  draw calls.
- Define quality profiles: `draft`, `balanced` (default), `accurate`, and
  `review`. A profile is included in an immutable renderer policy hash.
- Obtain owners for code-signing certificates, Apple Developer notarization,
  Azure tenancy/storage/queue/region policy, SSO/SCIM provider, privacy and
  telemetry policy, data retention/residency, and pilot firms.
- Repair release prerequisites: signed Windows packages, notarized macOS
  packages, WebView2 bootstrap on Windows (the installer now detects an
  existing Evergreen Runtime and, when the release pipeline stages Microsoft's
  official bootstrapper, provisions it silently — see
  `docs/early-access-distribution.md`), release provenance/SBOM, and a
  staged canary-to-stable release process.

**Exit criteria**

- Baseline measurements exist for small, medium, and large benchmark models.
- A named security, product, and operations owner approves every external
  decision above.
- No unsigned artifact is published as a production release.

### M1 - Storey-first rendering foundation

**Outcome:** large models become useful before all geometry is loaded.

1. Introduce `vex.render-manifest/2` while retaining read compatibility for
   manifest v1 for one release.
2. Put the renderer policy hash in the artifact cache namespace:
   `commit/policy_hash`, preventing a renderer or quality change from
   overwriting an older artifact.
3. Emit per-storey GLB tiles and per-tile semantic indexes. Resolve storey
   membership from `IfcRelContainedInSpatialStructure` and spatial aggregation;
   retain an explicit unassigned/site bucket.
4. Recenter large site coordinates with a recorded origin offset, record units
   and coordinate convention in the manifest, and keep LOD0 unquantized for
   accurate review.
5. Add deterministic fixture coverage for multiple storeys, unassigned
   products, openings, multi-storey elements, units, large coordinates, and
   invalid geometry.

**Exit criteria**

- A multi-storey fixture produces deterministic content-addressed tiles whose
  element membership matches IFC containment.
- Tile selection maps to the same GlobalId as the raw IFC fallback.
- Artifacts retain current validation, digest verification, symlink rejection,
  atomic publication, and immutable serving guarantees.

### M2 - Progressive viewer and artifact parity

**Outcome:** smooth navigation with bounded resource use and no semantic
regression from artifact mode.

- Replace sequential tile loading with a scheduler using frustum visibility,
  screen-space error, coarse-first priority, bounded concurrent fetches, and
  cancellation on commit switch or eviction.
- Decode compressed geometry off the UI thread and enforce configurable CPU,
  GPU, tile-count, triangle-count, and cache budgets. Dispose evicted Three.js
  resources immediately.
- Drive storey visibility from containment-aware tile membership. Retain visual
  clipping planes for sections only.
- Keep LOD0 authoritative for picking and review. If a coarse tile is clicked,
  fetch its LOD0 representation before resolving the precise triangle range.
- Add artifact-mode property hydration and GlobalId-based visual diffs,
  including ghosts from the prior commit for removed elements. Raw IFC is a
  fallback only when an artifact is unavailable or failed.
- Add browser performance metrics and CI gates for first frame, interaction,
  cancellation, and memory-budget eviction.

**Initial service targets**

| Model class | Browser first useful frame | First selectable object | First rendered storey |
| --- | ---: | ---: | ---: |
| Small, under 1M triangles | under 1.5 s | under 3 s | under 10 s |
| Medium, 1-20M triangles | under 2.5 s | under 6 s | under 30 s |
| Large, 20-150M triangles | under 4 s | under 10 s | under 60 s |

These targets are validated against agreed hardware and benchmark corpus, not
assumed from a developer machine.

### M3 - Render scale and reliability

**Outcome:** artifact generation is fair, resumable, and capable of handling
large teams and models.

- Generate LOD chains per tile without simplifying across element identity
  boundaries. Use LOD0 for changed elements in review mode.
- Add per-project fair queues, memory-aware worker concurrency, bounded Node
  heaps, shard progress, superseded-commit coalescing, and resumable segmented
  publication.
- Add local artifact cache LRU/size caps, HTTP Range support, and structured
  worker/browser metrics.
- In the Vex engine, add partial spatial checkout, authoritative
  containment/property export, and geometry-hash exposure for incremental tile
  reuse.
- Move shared artifact generation to a cloud queue and worker farm only after
  M1-M3 demonstrate fidelity and SLOs locally. Serve immutable, digest-verified
  objects through approved storage/CDN; keep manifests private and
  authorization-gated.

**Exit criteria**

- Rendering a superseded intermediate commit does not delay the current head.
- A restart retains completed shard outputs and resumes only unfinished work.
- The viewer stays inside configured GPU memory and has no stale geometry after
  repeatedly switching commits mid-stream.
- Artifact selection, property display, and diff highlighting match the raw IFC
  parity fixtures.

### M4 - Managed desktop operations

**Outcome:** the standalone desktop app is safe to deploy and support in a
large firm.

- Add panic capture, redacted crash artifacts, native error reporting, and
  support-bundle export that excludes keys, tokens, and home paths. (Panic
  capture, redacted crash artifacts under the app data directory, and native
  failure surfacing in `vex-desktop`/`vex-tray` are implemented — see
  `crash_report.rs` and `docs/early-access-distribution.md`; support-bundle
  export remains open.)
- Provide managed daemon lifecycle appropriate to each OS, including restart
  behavior and installer/uninstaller coverage.
- Add rollback after a failed update health check, channel persistence, and
  policy-controlled update deployment.
- Add an IT-managed policy precedence layer for endpoints, allowed inbox roots,
  cloud push, update channel, and telemetry restrictions.
- Implement non-fast-forward conflict recovery as an explicit user workflow,
  not an endlessly retried transient push.
- Persist window position and monitor context; complete keyboard, screen-reader,
  high-DPI, multi-monitor, installer, service-lifecycle, and UI automation
  testing.
- Expand CI to Windows, macOS, and Linux, and include dependency checks,
  end-to-end watcher-to-push tests, and installer smoke tests.

**Exit criteria**

- A fresh managed Windows image launches successfully with no preinstalled
  WebView runtime.
- Forced crashes produce a redacted support artifact and clear recovery path.
- A rejected push tells the user how to recover without corrupting the local
  outbox.
- Enterprise policy cannot be bypassed through user configuration.

### M5 - Enterprise control plane

**Outcome:** cloud collaboration has the security and operational controls
required for real project data.

- Make Tenant, Organization, Project, Team, Role, Device, Repository, Artifact,
  and AuditEvent first-class, tenant-scoped resources in the control plane.
- Implement OIDC/SAML SSO, SCIM lifecycle management, group-based RBAC,
  device-key rotation/revocation, authorization decision caching with bounded
  TTL, and append-only exportable audit events.
- Migrate repositories and artifacts from the single Azure Files/ACI topology
  to tenant-prefixed object storage with scoped IAM, encryption, backups,
  restore drills, quotas, and region/residency controls.
- Define SLOs, RPO/RTO, incident response, on-call, key rotation, security
  review, and security evidence required for SOC 2/GDPR obligations.
- Create a versioned public cloud API using OAuth service identities,
  idempotency keys, pagination, signed replayable webhooks, and delivery logs.
  Keep Bridge's loopback `/v1` API private.

**Exit criteria**

- A revoked employee or device cannot access, push, enumerate, or download
  tenant content within the committed revocation SLA.
- Tenant isolation is exercised in integration tests at API and storage layers.
- Restore drills meet the contracted RPO/RTO.
- Security-relevant events are attributable to an actor, device, project, and
  immutable object/commit identity.

### M6 - Construction workflows and qualified integrations

**Outcome:** Vex Atlas supports controlled, auditable coordination without
overclaiming CAD interoperability.

- Introduce a `ModelPackage` manifest with raw-file hash/source retention,
  application/version, schema/MVD, units, coordinates, exporter profile,
  discipline, container metadata, Vex/import profile version, warnings, and
  validation status.
- Qualify a narrow initial matrix: IFC2x3 Coordination View and IFC4 Reference
  View from named Revit and Archicad exporters. Unsupported profiles are
  explicitly rejected or quarantined.
- Productize signed managed publish connectors that validate profile, units,
  coordinates, GlobalId preservation, and destination before submission.
- Implement immutable federation sets composed of named published discipline
  versions and transforms, not a merged authoring model.
  - Local-first groundwork **landed**: a versioned federation data model,
    durable storage with stable federation/member ids, strict validation
    (full commit hashes, invertible affine transforms, bounded member counts,
    project/commit membership checks), token-gated CRUD/list/snapshot routes
    under `/v1/federations`, and project-deletion safety. Members currently
    reference exact commits of **local** projects; binding to cloud-*published*
    discipline versions remains future cloud work.
- Implement BCF 3 issue import/export, viewpoints, attachments, selected
  GlobalIds, comments, assignment, and immutable lifecycle events.
- Implement container approval:
  `Draft -> Open -> Assigned -> In progress -> Ready for review ->
  Accepted/Rejected -> Closed/Reopened`, with publication controlled by
  validation, review, and designated approval.
- Apply ISO 19650-aligned templates for identifiers, discipline, revision,
  suitability, classification, milestones, ownership, and retention. Do not
  make ISO certification claims.

**Exit criteria**

- A qualified author publishes a package with explicit source provenance and
  recoverable validation outcomes.
- A coordinator creates a locked federation set and a BCF-linked issue from an
  exact model version and viewpoint.
- An approver can accept or reject an information container with immutable
  rationale and a traceable status transition.

## Cross-cutting release gates

No milestone is released without:

- security validation for untrusted IFC/artifact inputs, path traversal,
  tenant isolation, and secret redaction;
- deterministic or policy-versioned artifact generation, schema compatibility,
  and raw-IFC parity tests;
- benchmark regression gates appropriate to the changed model class;
- documented upgrade, rollback, observability, and support paths;
- user-facing documentation that distinguishes preview, canonical checkout,
  and qualified delivery artifacts.

## Pilot plan

Run a 12-week controlled pilot with one project, 3-4 disciplines, 30-60 users,
two qualified authoring systems, one coordinator, and one information manager.

1. Weeks 1-2: map EIR/BEP requirements, configure templates and roles, and
   establish benchmark/corpus baselines.
2. Weeks 3-6: run publish, validation, comparison, desktop support, and
   performance workflows in parallel with the incumbent process.
3. Weeks 7-9: use immutable federation sets and BCF issues for coordination.
4. Weeks 10-12: run approvals in parallel, assess measurable outcomes, and
   decide whether to expand the qualified exporter matrix.

Pilot feedback must include first-use completion, publish failure recovery,
render SLOs, crash rate, issue closure traceability, and administrator
deployment experience. Telemetry remains opt-in or policy-controlled.

## Explicit deferrals

Do not begin these before the program gates above are satisfied:

- automatic building-model design merges or live co-authoring;
- semantic ingestion/editing/round-trip of proprietary native CAD formats;
- broad, unqualified IFC4x3 or design-transfer support;
- active-active multi-region operation, per-element ACLs, or on-premises
  deployment;
- a CDN before object integrity, tenant authorization, and manifest privacy
  controls are proven.

## Immediate implementation backlog

The highest-impact code sequence is:

1. Manifest v2 and per-tile semantic index.
2. Deterministic storey-first artifact generation.
3. Bounded progressive tile scheduler with cancellation and budgets.
4. Artifact-mode properties and diff parity.
5. LOD refinement, render-worker fairness, segmented publish, and SLO gates.
6. Desktop crash/support/WebView2/conflict/policy hardening in parallel.

Cloud tenancy, signed releases, managed identity, qualified connectors, and
federation/approval workflows begin after their external ownership decisions
are approved; they cannot be safely invented inside this repository alone.
