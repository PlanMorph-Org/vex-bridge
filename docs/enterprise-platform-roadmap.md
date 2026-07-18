# Vex Atlas / Vex Bridge — Enterprise Cloud & Platform Layer Roadmap

> Scope: the **cloud/platform layer** required to sell and operate Vex Atlas
> (the semantic BIM version-control product) for large construction, AEC, and
> engineering firms. This is a planning document only. It does **not** change
> any code in `vex-bridge` or `vex`.
>
> Audience: platform/infra leads, security, and the eng leads owning `architur`
> (control plane), `vex-serve` (repo daemon), and `vex-bridge` (desktop agent).

---

## 1. How to read this document

- Sections **2–4** establish current state, assumptions, and the target
  architecture so the workstreams are grounded in what exists today.
- Section **5** is the security threat model (trust boundaries + STRIDE) and
  Section **6** the control objectives that every workstream is measured against.
- Section **7** is the detailed workstream design, one subsection per topic the
  brief asked for, each with **design, dependencies, and acceptance criteria**.
- Section **8** sequences the work into **bounded milestones (M0–M6)** with a
  critical-dependency graph.
- Section **9** is the consolidated launch **acceptance gate**.
- Section **10** is the explicit **do-not-attempt-at-launch** list.

Everywhere a decision depends on a component **outside these two repos**, it is
called out as **[ASSUMPTION – architur]** or **[ASSUMPTION – infra]**.

---

## 2. Current state (as observed in the repos)

### 2.1 Components that exist today

| Plane | Component | Repo | Role |
| --- | --- | --- | --- |
| Desktop | `vex-bridge` daemon (+ `vex-tray`, `vex-desktop`) | `vex-bridge` | Local agent on 127.0.0.1:7878; watches IFC inbox, imports/commits via `vex`, user-initiated push over SSH. Holds ed25519 key seed in OS keychain; local API guarded by a 256-bit `access-token`. |
| Engine | `vex` CLI + crates (`vex-core`, `vex-storage`, `vex-graph`, `vex-diff`, …) | `vex` | Semantic IFC version control. Content-addressable object store (redb local; **S3-compatible backend exists** behind the `s3-backend` feature). |
| Repo server | `vex-serve` | `vex` | Server daemon invoked by `sshd` `ForceCommand`. Resolves repos as `<owner>/<name>` under `repo_root`; calls control plane to authorize and to mirror ref updates. |
| Control plane | **`architur`** (.NET / ASP.NET Core Minimal APIs) | **outside both repos** | Device pairing, user/org/repo model, `authorize` + `ref-updated` internal endpoints, studio web UI. |

### 2.2 Contracts already implemented (the integration surface)

- **Device pairing** (`vex-bridge` → architur): `POST /api/device-pairing/start`,
  `GET /api/device-pairing/{code}`, `GET /api/device-pairing/projects`. The
  daemon generates an ed25519 key, registers the OpenSSH public key as a
  `UserSshKey`, and signs project-list requests with headers `X-Vex-Key-Id`,
  `X-Vex-Timestamp`, `X-Vex-Signature`.
- **Push transport**: `ssh://vex@<vex_serve_host>:22/proj/<repo-uuid>`. Push is
  **user-determined** (a local "ready to push" ledger, flushed on the dashboard
  **Push** button / `POST /v1/repo/push`), mirroring Git semantics.
- **Server authorization** (`vex-serve` → architur): `POST /api/internal/vex/authorize`
  (`{userId, repositoryId, operation}` → `{allow, reason}`) and
  `POST /api/internal/vex/ref-updated`. Both HMAC-SHA256 signed over
  `"<unix_ts>.<body>"` with header `t=<ts>,v1=<hex>`, shared secret
  `VEX_INTERNAL_SECRET`. `VEX_FAIL_CLOSED=true` in production.
- **Repo identity**: `vex-serve` hardens `<owner>/<name>` (rejects `..`,
  absolute paths, control chars) and reads `.vex/architur.toml` (`repo_id`) to
  map a filesystem repo to its architur UUID.
- **Object store tenant isolation is anticipated**: `S3Config.key_prefix` is
  documented for `tenants/<orgId>/repos/<repoId>/`; objects are Blake3-sharded
  and framed/zstd-compressed.

### 2.3 Deployment topology today (from CI/CD)

- `vex-serve` → a **single Azure Container Instance** (`aci-vex-serve`,
  `northeurope`), SSH:22, backed by a **single Azure Files share**
  (`vexatlas-repos`) mounted at `/var/lib/vexatlas/repos`. Deploys **recreate**
  the container group (no in-place update ⇒ push downtime), using ACR admin
  credentials and a storage-account key.
- `architur` API + web → Azure Container Apps (`ca-vexatlas-api`,
  `ca-vexatlas-web`). Custom domain `planmorph.software` is expired/parked;
  config currently points straight at the ACA FQDNs.
- Release (`vex-bridge`): GitHub Releases, per-platform bundles + Windows Inno
  Setup installer, **SHA256SUMS** integrity verified by the in-app updater.
- CI (`vex`): `cargo test`, `fmt`, `clippy -D warnings`, `cargo-deny`
  (advisory + license). **No SBOM, no signed provenance, no code signing/
  notarization.**

### 2.4 Enterprise gaps (verified absent in both repos)

No SSO (OIDC/SAML), no SCIM, no team/role RBAC beyond a boolean `authorize`, no
formal audit log, no key rotation/revocation lifecycle, no data
residency/retention/legal-hold controls, no backups/DR plan, no SLOs, no public
API/webhooks, no formal support-diagnostics tooling. These are the roadmap.

---

## 3. Assumptions

1. **[ASSUMPTION – architur]** The identity/tenant/authorization model lives in
   `architur`, not in these repos. Most workstreams below (SSO, SCIM, RBAC,
   audit, org/team model, webhooks) are **primarily architur work**; `vex-serve`
   and `vex-bridge` consume its decisions through the existing signed contracts.
   Where this doc specifies data models or endpoints, treat them as the target
   contract architur must implement, versioned alongside the existing
   `/api/internal/vex/*` surface.
2. **[ASSUMPTION – infra]** Primary cloud is Azure (observed: ACA, ACI, ACR,
   Azure Files, Entra/OIDC federation for CI). The design stays
   cloud-neutral where practical (the object store's S3 API works against Azure
   Blob via an S3 shim, R2, or MinIO), but concrete SLAs/DR assume Azure.
3. Vex is **early MVP** ("APIs unstable, not for production") — the platform
   layer must ship **behind versioned, backward-compatible contracts** because
   desktop agents in the field cannot be force-upgraded synchronously.
4. Customer data of record = **IFC-derived semantic graph objects + commit
   metadata + derived render artifacts**. Raw uploaded IFC is transient
   (archived locally); the canonical artifact is the object store.
5. "Large construction firms" implies: SSO-mandatory procurement, security
   questionnaires (SIG/CAIQ), data-residency clauses, contractual RPO/RTO, and
   named projects with strict inter-project confidentiality (joint ventures,
   competing bids). Tenant isolation is a **hard** requirement, not a feature.

---

## 4. Target architecture (platform layer)

```
                 ┌─────────────────────────────────────────────────────────┐
                 │  Enterprise IdP (Entra / Okta / PingFederate …)          │
                 │  OIDC + SAML (SSO)          SCIM 2.0 (provisioning)      │
                 └───────────────┬───────────────────────┬──────────────────┘
                                 │ SSO                    │ SCIM push
                                 ▼                        ▼
   Desktop            ┌───────────────────────────────────────────────┐
 ┌──────────┐  pair   │  architur control plane (multi-tenant)          │
 │vex-bridge│────────▶│  • Tenant/Org/Team/Project/Role (RBAC/ABAC)     │
 │ (agent)  │  device │  • Device registry + key lifecycle              │
 └────┬─────┘  keys   │  • AuthZ decision API (/internal/vex/authorize) │
      │               │  • Audit log (append-only, exportable)          │
      │ push (SSH)    │  • Public REST API + Webhooks + API tokens      │
      ▼               │  • Artifact metadata + signed URL broker         │
 ┌──────────┐  authz  └───────┬───────────────────────────┬─────────────┘
 │vex-serve │◀────────────────┘       ref-updated          │
 │ (repo    │  fetch/push over SSH                          │ signed URLs
 │  daemon) │─────────────┐                                 ▼
 └──────────┘             ▼                       ┌───────────────────┐
                 ┌───────────────────┐            │ Object store      │
                 │ Cloud object store │◀──────────│ (S3-compatible)   │
                 │ tenants/<org>/     │            │ per-tenant prefix │
                 │  repos/<repo>/...  │            │ + envelope enc.   │
                 └─────────┬──────────┘            └───────────────────┘
                           │ queue (commit event)
                           ▼
                 ┌───────────────────┐
                 │ Artifact workers   │  render GLB / semantic index / scans
                 │ (queue-driven)     │  → publish under signed manifest
                 └───────────────────┘
```

Key shifts from today:
- `vex-serve` becomes **horizontally scalable + regional**, repos move from a
  single Azure Files share to the **S3-compatible object store** with per-tenant
  prefixes; the filesystem becomes a cache, not the source of truth.
- **Cloud-side artifact workers** (render/index/scan) replace reliance on the
  desktop render worker for shared/large-model artifacts.
- All identity/authz/audit is centralized in architur behind versioned APIs.

---

## 5. Security threat model

### 5.1 Trust boundaries

| # | Boundary | Crossing traffic | Current control | Target control |
| --- | --- | --- | --- | --- |
| B1 | Browser ↔ bridge daemon | Local HTTP :7878 | `access-token` (0600), 127.0.0.1 bind, constant-time compare | Keep; add origin checks + token rotation on pair change |
| B2 | Bridge ↔ architur | Device-pairing HTTPS | ed25519 request signing, timestamped | Add nonce/replay window, cert pinning option, key rotation |
| B3 | Bridge ↔ vex-serve | SSH:22 | ed25519 key auth, `ForceCommand` | Host-key pinning/TOFU-DB, per-tenant SSH CA, rate limits |
| B4 | vex-serve ↔ architur | Internal authorize/ref-updated | HMAC-SHA256, fail-closed | Add key rotation, mTLS, replay nonce, decision caching w/ TTL |
| B5 | Platform ↔ object store | Object read/write | (new) | Per-tenant prefix + IAM scope + envelope encryption + signed URLs |
| B6 | Tenant ↔ tenant | (logical) | `<owner>/<name>` path hardening | Enforced tenant scoping at authz + storage + query layers |
| B7 | Release pipeline ↔ end user | Download/update | SHA256SUMS | + code signing/notarization + SLSA provenance + SBOM |

### 5.2 STRIDE (highest-priority threats)

- **Spoofing** — stolen device key impersonates a user push. *Mitigate:* device
  registry with per-device identity, revocation, key expiry/rotation, and
  authorize decisions bound to `keyId → userId → tenant`.
- **Tampering** — malicious object/manifest injected into the store or a tampered
  release. *Mitigate:* content-addressed objects (hash = name), manifest
  SHA-256 validation (already in bridge render contract), signed releases +
  provenance.
- **Repudiation** — "I never pushed that / deleted that project". *Mitigate:*
  append-only audit log with actor, device, IP, decision, and object hashes;
  tamper-evident (hash-chained) export.
- **Information disclosure** — cross-tenant read of another firm's model (JV /
  competing-bid confidentiality). *Mitigate:* tenant scoping enforced at every
  layer (B5/B6); per-tenant encryption keys; deny-by-default authz.
- **Denial of service** — a huge IFC or push storm exhausts a shared repo node.
  *Mitigate:* body-size limits (512 MiB already in bridge), per-tenant quotas,
  push concurrency limits, autoscaling repo daemons, timeouts (push already
  bounded at 45s in bridge).
- **Elevation of privilege** — a project member escalates to org admin, or
  path-traversal escapes `repo_root`. *Mitigate:* hardened repo-name parser
  (exists), least-privilege roles, server-side role checks (never client-trusted),
  signed internal calls.

### 5.3 Abuse/edge scenarios to design for

Offboarded employee's paired laptop; contractor with time-boxed access;
shared workstation; export of a model to a competitor project; legal-hold on a
disputed project mid-litigation; region-locked customer whose data must never
leave the EU; a compromised `VEX_INTERNAL_SECRET`.

---

## 6. Control objectives (SOC 2 / GDPR alignment)

| ID | Objective | Maps to |
| --- | --- | --- |
| CO-1 | Every access to tenant data is authenticated, authorized deny-by-default, and attributable to a principal + device. | SOC2 CC6.1/CC6.2/CC6.3; GDPR Art.32 |
| CO-2 | Tenant data is logically isolated end-to-end; cross-tenant access is impossible by construction, not just by policy. | SOC2 CC6.1; GDPR Art.32 |
| CO-3 | Data is encrypted in transit and at rest with managed, rotatable keys; optional per-tenant keys. | SOC2 CC6.7; GDPR Art.32 |
| CO-4 | All security-relevant events are logged to an append-only, time-synced, exportable audit trail. | SOC2 CC7.2/CC7.3 |
| CO-5 | Access is provisioned/deprovisioned via SSO+SCIM; revocation is effective within a bounded SLA. | SOC2 CC6.1/CC6.2; GDPR Art.5 |
| CO-6 | Backups exist, are tested/restorable, and meet contractual RPO/RTO. | SOC2 A1.2/A1.3 |
| CO-7 | Data subject rights (access, export, deletion) and retention/residency/legal-hold are enforceable per tenant. | GDPR Art.15/17/28/30/44 |
| CO-8 | Software supply chain is verifiable: SBOM, signed artifacts, build provenance. | SOC2 CC8.1; SLSA |
| CO-9 | Change management, incident response, and on-call are defined and evidenced. | SOC2 CC7.4/CC8.1 |
| CO-10 | Availability and performance are measured against published SLOs with error budgets. | SOC2 A1.1 |

Each workstream in §7 tags the CO(s) it satisfies.

---

## 7. Workstreams

Each subsection: **Goal → Design → Dependencies → Acceptance criteria (AC)**.

### 7.1 Multi-tenancy & isolation  *(CO-1, CO-2, CO-3)*

**Goal.** A hard tenant boundary such that one firm can never observe, enumerate,
or affect another's data.

**Design.**
- Introduce a first-class **Tenant** (customer/firm) as the top of the hierarchy
  in architur. Every Org/Project/Repo/User/Device/Object/AuditEvent carries a
  `tenant_id`. **[ASSUMPTION – architur]**
- **Storage isolation**: adopt the object store's `key_prefix` convention
  (`tenants/<tenantId>/repos/<repoId>/`), already designed in `s3_backend.rs`.
  Enforce that `vex-serve`/workers are handed credentials scoped to a single
  tenant prefix per request (short-lived, brokered by architur), so a bug cannot
  read another prefix.
- **Compute isolation model** (choose per plan): start with **pool + row-level
  tenant scoping**; offer **dedicated repo daemon + dedicated bucket/keys** as an
  enterprise tier for firms with contractual isolation needs.
- **Query isolation**: all control-plane queries filtered by `tenant_id` via a
  mandatory scoping layer (not ad-hoc `WHERE`), enforced in one place.
- `vex-serve` repo resolution extends `<owner>/<name>` → tenant-scoped path;
  authorize responses must include the resolved `tenant_id` and the daemon must
  refuse operations whose resolved repo tenant ≠ authorized tenant.

**Dependencies.** Object-store workstream (7.7); authz (7.5).

**AC.**
- A red-team test attempting cross-tenant read/list/push via crafted repo names,
  reused signed URLs, and stolen-but-wrong-tenant device keys is **denied at
  every layer** and logged.
- Deleting/suspending a tenant makes all its repos and artifacts inaccessible
  within the SLA and leaves no cross-tenant residue.

### 7.2 Org / Team / Project / Role model  *(CO-1)*

**Goal.** An RBAC model AEC firms recognize.

**Design.** Hierarchy: `Tenant → Org(s) → Team(s) → Project(s) → Repository(ies)`.
- **Built-in roles** (least privilege, deny-by-default):
  `Owner`, `Org Admin`, `Security Admin`, `Auditor` (read audit only),
  `Project Admin`, `Contributor` (push/commit), `Reviewer` (read + comment),
  `Viewer` (read), `Guest/Contractor` (time-boxed, project-scoped),
  `Billing Admin`.
- **Assignment** at org/team/project scope; teams map to SCIM groups.
- **Permission → operation matrix** must resolve the existing `vex-serve`
  operations (`push`, `fetch`, and future `delete-ref`, `read-artifact`,
  `admin`). architur's `authorize` returns allow/deny per (user, repo, op) — the
  role engine sits behind it. **[ASSUMPTION – architur]**
- **ABAC overlays** for enterprise: project confidentiality labels (e.g.
  "JV-walled"), contractor expiry, IP-range/network conditions.

**Dependencies.** SSO/SCIM (7.3) for group-driven assignment; audit (7.6).

**AC.** Permission matrix documented and tested; changing a user's role takes
effect on the next authorize decision (bounded cache TTL); contractor role
auto-expires; no operation is allowed without an explicit granting role.

### 7.3 SSO — OIDC / SAML / SCIM  *(CO-5)*

**Goal.** Enterprise-mandatory login + lifecycle provisioning.

**Design.** *(primarily architur)* **[ASSUMPTION – architur]**
- **OIDC** (Auth Code + PKCE) and **SAML 2.0** SP, per-tenant IdP config
  (metadata URL, cert, entity IDs, JIT provisioning toggle, allowed domains).
- **SCIM 2.0** `/Users` + `/Groups` for provisioning/deprovisioning; Group→Team
  mapping drives role assignment; deprovision → disable user + revoke devices +
  invalidate sessions/tokens.
- **Desktop pairing under SSO**: the browser `/pair` flow authenticates via the
  tenant's IdP before approving a device key. The device-key model (already
  built) remains the machine credential; SSO gates *who* may approve it and
  *which tenant* the device is bound to.
- **Session policy**: max lifetime, idle timeout, MFA delegated to IdP, optional
  step-up for admin actions.

**Dependencies.** Org/role model (7.2); device lifecycle (7.4).

**AC.** A tenant configures Entra and Okta independently; JIT creates users with
correct role from group; SCIM deprovision revokes access + devices within the
SLA (target ≤ 5 min); SP-initiated and IdP-initiated SAML both work; no local
password path for SSO-enforced tenants.

### 7.4 Device registration / revocation / key rotation  *(CO-1, CO-5)*

**Goal.** Full lifecycle for the ed25519 device keys the bridge already issues.

**Design.**
- **Registry** (architur): `{deviceId, tenantId, userId, keyId, publicKey,
  fingerprint, label, platform, createdAt, lastSeenAt, expiresAt, status}`.
  The bridge already sends `deviceLabel` + OpenSSH public key at pairing.
- **Revocation**: admin- or SCIM-driven; on revoke, remove the key from the
  authorized set consulted at SSH auth time and fail future `authorize` calls.
  vex-serve must consult device status (via authorize) — a revoked key's push is
  denied even if SSH auth is briefly cached.
- **Rotation**: bridge grows a `rotate-key` flow (generate new seed in keychain,
  register new key, deprecate old after overlap window). Add **key expiry**
  (e.g. 180 days) with proactive renewal so stale keys age out.
- **Replay/nonce**: extend the signed device requests with a server-issued nonce
  or tightened timestamp window (currently timestamp-only).
- **Bind to tenant**: a device key authorizes exactly one tenant/user pairing.

**Dependencies.** Authz (7.5); SSO/SCIM (7.3) for automated revocation.

**AC.** Revoking a device blocks its next push within the SLA and is audited;
rotation completes with zero downtime and overlapping validity; expired keys are
refused; a replayed signed request outside the window is rejected.

### 7.5 Authorization & policy engine  *(CO-1, CO-2)*

**Goal.** A single, deny-by-default decision point powering the existing
`/api/internal/vex/authorize` contract.

**Design.** *(primarily architur)*
- Central **policy decision** service: input `(tenantId, userId, deviceId,
  repoId, operation, attributes)` → `allow/deny + reason + obligations`
  (e.g. read-only, TTL). Keep the existing signed request/response shape;
  **extend** it additively (new fields optional) to preserve field compatibility.
- **Decision caching** at vex-serve with short TTL + explicit invalidation on
  revoke; keep `VEX_FAIL_CLOSED=true` in prod.
- **Secret rotation** for `VEX_INTERNAL_SECRET` (dual-secret overlap window);
  consider mTLS between vex-serve and architur to complement HMAC.
- Policy is **server-authoritative**; the bridge/UI never make trust decisions.

**Dependencies.** RBAC (7.2), device registry (7.4), audit (7.6).

**AC.** Every push/fetch produces an authorize decision that is logged;
disabling a user/role/device flips subsequent decisions to deny; a rotated
internal secret is accepted during overlap and the old one is refused after.

### 7.6 Audit logging  *(CO-4)*

**Goal.** Tamper-evident, exportable audit trail for security + compliance.

**Design.**
- **Append-only** event store (architur) capturing: authN (SSO login, device
  pair/revoke/rotate), authZ decisions, repo lifecycle (create/rename/delete,
  ref updates via existing `ref-updated`), artifact access (signed-URL issue),
  admin/role/config changes, data-subject/retention/legal-hold actions.
- Each event: `tenantId, actor(user/service), deviceId, ip, ts (NTP-synced),
  action, target, decision, request-id, prev-hash` (hash-chained for tamper
  evidence).
- **Retention & export**: per-tenant retention config; export to customer
  SIEM (S3/Blob drop or webhook); immutable storage tier for the audit bucket.
- vex-serve/workers emit structured events to the same pipeline (they already
  use `tracing`); correlate by request-id.

**Dependencies.** Observability pipeline (7.10) shares transport but audit is a
separate, integrity-protected stream.

**AC.** Auditor role can query/export a tenant's trail; the hash chain verifies;
gaps/edits are detectable; a full push→commit→artifact flow appears as a
correlated event chain; audit store writes are WORM/immutable.

### 7.7 Repository / commit / artifact authorization  *(CO-1, CO-2)*

**Goal.** Authorization consistently applied to repos, individual ref updates,
and cloud artifacts.

**Design.**
- **Repo/commit**: keep vex-serve's authorize-before-push and ref-updated
  mirror; add operations for `delete-ref`, branch protection (e.g. only
  `Project Admin` moves `main`), and per-ref checks.
- **Artifact authorization**: render/index/scan artifacts are tenant+repo+commit
  scoped. Clients never get raw store credentials; architur brokers **short-TTL
  signed URLs** scoped to a single object after an authorize check. The bridge's
  existing manifest+SHA-256 validation contract carries over to cloud artifacts.
- **Immutability**: complete 64-char commit hashes are immutable; artifacts keyed
  by full commit hash + object SHA-256 (already the bridge design) so an
  artifact can never be reused for the wrong revision.

**Dependencies.** Object store (7.8), authz (7.5), audit (7.6).

**AC.** A viewer cannot fetch an artifact for a repo they lack read on; a signed
URL is single-object, short-lived, and rejected after expiry/for another tenant;
branch-protection blocks unauthorized `main` moves; every artifact read is
authorized + audited.

### 7.8 Cloud artifact storage / queue / worker architecture  *(CO-2, CO-3, CO-10)*

**Goal.** Move the source of truth off the single Azure Files share; generate
shared artifacts server-side, reliably and at scale.

**Design.**
- **Object store as source of truth**: promote `vex-storage`'s **S3-compatible
  backend** (Azure Blob via S3 shim / R2 / MinIO / S3) to primary for hosted
  repos, per-tenant `key_prefix`. The repo daemon's local disk becomes a cache.
- **Repo daemon scale-out**: replace the single recreated ACI with a
  horizontally scalable, health-checked service (Container Apps/AKS) so pushes
  are HA and deploys are zero-downtime (rolling), with per-tenant/ per-repo
  concurrency limits and quotas.
- **Event → queue**: on `ref-updated`, enqueue an **artifact job**
  (render GLB + semantic index; later: virus/malware scan of uploads, LOD tiles,
  property hydration). Use a durable queue (Azure Service Bus/Storage Queues)
  with visibility timeout, DLQ, idempotency keyed by `(repoId, commitHash,
  artifactType)`.
- **Workers**: stateless, autoscaled by queue depth; reuse the **existing
  publish contract** (staging dir → validate schema + per-object SHA-256 →
  atomic publish) so a partial/tampered artifact can never be served. The doc's
  note that the Node web-ifc worker can be swapped for a native engine applies
  server-side too.
- **Idempotency & retries**: workers must be safe to re-run; publish is atomic;
  failures go to DLQ with alerting.

**Dependencies.** Multi-tenancy (7.1), artifact authz (7.7).

**AC.** A pushed commit produces a validated cloud artifact via the queue with
no partial publishes; worker autoscaling holds p95 artifact latency within SLO
under a defined load; a repo daemon rolling deploy causes **no** push failures;
losing a worker node re-drives jobs from the queue with no data loss.

### 7.9 Data residency / encryption / retention / legal hold  *(CO-3, CO-7)*

**Goal.** Contractual data-handling controls per tenant.

**Design.**
- **Residency**: per-tenant home region; all buckets, repo daemons, workers, and
  audit stores for that tenant pinned to region; block cross-region replication
  unless contractually enabled. Start with EU (`northeurope`) + one alternate.
- **Encryption**: TLS everywhere in transit; at rest via managed keys with
  **envelope encryption**; enterprise tier gets **per-tenant CMK** (customer-
  managed key) with rotation. Objects are already framed/compressed — add an
  encryption layer at the store or via CMK-backed bucket encryption.
- **Retention**: per-tenant policies for commits/artifacts/audit; lifecycle
  rules on derived artifacts (regenerable) vs. canonical objects (retain).
- **Deletion / DSR**: tenant/project/user deletion tooling that purges objects
  across store + caches + backups (documented crypto-shred via CMK destroy for
  fast effective deletion). Support GDPR access/export/erasure requests.
- **Legal hold**: a hold flag on a project/tenant that **overrides** retention
  and deletion (WORM/immutability lock on the relevant prefixes) until released;
  every hold/release is audited.

**Dependencies.** Object store (7.8), audit (7.6), backups (7.10-DR).

**AC.** A tenant pinned to EU has no artifact/audit/backup byte outside EU
(evidenced); CMK rotation works without data loss; a deletion request provably
removes data (including from backups per policy); a legal hold blocks deletion
and lifecycle expiry and is logged; DSR export produces the subject's data.

### 7.10 Backups / DR / RPO / RTO + Observability / SLOs / On-call  *(CO-6, CO-9, CO-10)*

**Goal.** Survive failures within contractual targets; know the system's health.

**Design — Backups/DR.**
- Content-addressed objects make the store **backup-friendly** (immutable, dedup).
  Enable versioning + cross-AZ redundancy in-region; scheduled, **restore-tested**
  backups of object store + control-plane DB + audit store.
- Define tiers, e.g. **RPO ≤ 15 min / RTO ≤ 4 h** (standard) with an enterprise
  option for tighter targets; document manual failover runbook first, automate
  later.
- Replace the single Azure Files share (single point of failure) as source of
  truth (see 7.8); keep it only as a warm cache.

**Design — Observability/SLOs/On-call.**
- **Telemetry**: structured logs (already `tracing`), metrics, and traces with
  request-id correlation across bridge → vex-serve → architur → workers.
- **SLIs/SLOs** (publish + error budgets): push success rate & p95 latency;
  authorize decision latency; artifact-job success & p95 end-to-end;
  control-plane API availability; SSO login success.
- **Alerting + on-call**: paging on SLO burn, DLQ growth, fail-closed authorize
  spikes, backup failures, cert/secret expiry.
- **Health/readiness** endpoints for every service (bridge already has
  `/v1/health`).

**Dependencies.** Object store (7.8) for backup design; audit (7.6) shares
transport.

**AC.** A documented DR game-day restores the control plane + a tenant's repos
within RTO and loses ≤ RPO; SLO dashboards exist with alerts wired to on-call;
killing the primary repo daemon/worker/region is exercised and recovers per
target; backup restore is tested (not just taken).

### 7.11 Secure updates / release provenance / SBOM  *(CO-8)*

**Goal.** Supply-chain integrity for the desktop agent + server images.

**Design.**
- **Code signing/notarization**: Windows Authenticode for the Inno Setup
  installer + binaries; macOS Developer ID sign + **notarize + staple** (today
  the mac bundle is raw/unsigned). This is procurement-blocking for enterprises.
- **Build provenance**: emit **SLSA provenance** (build attestation) for release
  artifacts and server images; publish to the release + registry.
- **SBOM**: generate CycloneDX/SPDX SBOMs (e.g. `cargo-cyclonedx`) per release,
  attach to GitHub Release and container images; extend CI (which already runs
  `cargo-deny`) to fail on new criticals.
- **Updater hardening**: keep SHA256SUMS verification; add **signature**
  verification of the manifest (not just hash) and downgrade protection.
- **Server images**: sign vex-serve/worker images (cosign), verify at deploy;
  move ACI deploy off ACR admin creds + storage-account keys to
  managed-identity/workload-identity + Key Vault.

**Dependencies.** Release pipeline; infra IAM.

**AC.** Windows + macOS artifacts are signed/notarized and install without
SmartScreen/Gatekeeper prompts; each release has an SBOM + verifiable
provenance; the updater rejects an unsigned/downgraded manifest; deploy verifies
image signatures; no long-lived registry/storage keys in CI.

### 7.12 Compliance readiness (SOC 2 / GDPR)  *(CO-1…CO-10)*

**Goal.** Be audit-ready and answer enterprise security questionnaires.

**Design.**
- Treat §6 control objectives as the SOC 2 control set; map each to evidence
  produced by the workstreams (audit logs, backup tests, access reviews, change
  management via CI/PR, incident runbooks).
- **GDPR**: DPA template, ROPA (Art.30), sub-processor list, DSR process (7.9),
  residency (7.9), breach-notification runbook.
- **Access reviews**: periodic, SSO/SCIM-driven, evidenced.
- **Policies**: change mgmt, incident response, vendor mgmt, SDLC/secure coding.
- Sequence: **readiness → Type I → Type II** (Type II needs a 3–6 month
  observation window, so start evidence capture early).

**Dependencies.** Essentially all workstreams; audit (7.6) is the evidence spine.

**AC.** A completed SIG-lite/CAIQ; documented controls with automated evidence;
a successful Type I readiness assessment; DSR and breach runbooks rehearsed.

### 7.13 Public APIs / webhooks / integrations  *(CO-1, CO-4)*

**Goal.** Let firms integrate Vex with their toolchain (CDE, PM, BI).

**Design.** *(primarily architur)*
- **Public REST API** (versioned `/api/v1`, additive changes only) over the
  tenant/project/commit/artifact model; **scoped API tokens** + OAuth client-
  credentials for machine access, tenant-scoped, least-privilege, revocable,
  audited.
- **Webhooks**: events (`commit.pushed`, `ref.updated`, `artifact.ready`,
  `member.changed`, `device.revoked`) with HMAC-signed payloads, retries + DLQ,
  and per-tenant endpoint config; reuse the `ref-updated` internal event as the
  source.
- **Rate limiting + quotas** per token/tenant.
- Keep internal `/api/internal/vex/*` separate from public API; never expose
  internal HMAC surface publicly.

**Dependencies.** Authz (7.5), audit (7.6), tenancy (7.1).

**AC.** A tenant creates a scoped token, calls the public API, and receives a
signed `artifact.ready` webhook with verified signature + retry on failure;
tokens are least-privilege, revocable, rate-limited, and audited.

### 7.14 Support diagnostics  *(CO-4, CO-9)*

**Goal.** Diagnose customer issues without weakening tenant isolation.

**Design.**
- **Bridge diagnostics bundle**: a `vex-bridge` command that packages logs
  (redacted), config (secrets stripped), daemon-lock/version, and connectivity
  checks (architur/vex-serve reachability, host-key, token presence) into a file
  the user can send. (Bridge already has logs, `daemon.lock`, `/v1/health`, and
  a Repair flow to build on.)
- **Correlation IDs** end-to-end so a support ticket maps to server-side events.
- **Break-glass support access**: time-boxed, approved, fully audited,
  least-privilege access to a tenant's control-plane data — never raw model
  bytes without explicit consent; every access logged and reported to the tenant.
- **Status page** + incident comms.

**Dependencies.** Observability/audit (7.10/7.6).

**AC.** A support engineer resolves a simulated push failure from a diagnostics
bundle + correlated server logs; break-glass access requires approval, expires,
and is fully audited and tenant-visible; no diagnostic path exposes another
tenant's data.

### 7.15 Migration & versioning  *(CO-8, CO-9)*

**Goal.** Evolve safely with un-upgradable agents in the field.

**Design.**
- **Wire/API versioning**: keep `PROTOCOL_VERSION` (vex protocol) and the
  `vex.visual-diff/N`, manifest, and semantic-index schema versions that already
  exist; make all cloud/API/webhook contracts **additive & backward-compatible**;
  negotiate capabilities (server capabilities already exist in the protocol).
- **Agent compatibility policy**: support the last N minor versions; the bridge
  already surfaces engine/bridge schema-skew errors — extend to a
  server-driven "minimum supported agent" with a clear upgrade prompt.
- **Data migrations**: object formats are content-addressed + framed/versioned;
  define a re-index/migration job pattern (queue-driven, idempotent) for derived
  artifacts; canonical objects are immutable so migrations are additive.
- **Config migration**: the bridge already migrates expired endpoints — formalize
  a config schema version + migration on load.
- **Tenant onboarding/offboarding** and **existing-repo import** (Azure Files →
  object store) runbooks with dry-run + verification.

**Dependencies.** Object store (7.8), release (7.11).

**AC.** An old agent within the support window keeps working against the new
platform or receives a clear upgrade prompt; a contract change ships without
breaking field agents; an Azure Files → object store repo migration verifies
byte/hash-for-hash with no data loss and is reversible.

---

## 8. Milestones (bounded) & critical dependencies

Milestones are **capability-bounded**, not date-bound (staff to taste). Each is
independently shippable and gated by the prior where noted.

### M0 — Platform foundations & hardening *(prereq for everything)*
- Tenant entity + `tenant_id` everywhere (7.1); RBAC model + role→operation
  matrix (7.2); authorize engine extended additively + secret rotation (7.5);
  audit spine (7.6); observability baseline + health/readiness + correlation IDs
  (7.10-obs).
- **Exit:** deny-by-default authz with audit on every decision; tenant scoping
  enforced in control plane; dashboards live.

### M1 — Identity & device lifecycle
- SSO OIDC + SAML, SCIM (7.3); device registry + revocation + rotation +
  replay hardening (7.4).
- **Depends on:** M0 (RBAC, audit).
- **Exit:** SSO-enforced tenant; SCIM deprovision revokes access + devices ≤ SLA.

### M2 — Cloud data plane
- Object store as source of truth + per-tenant prefixes (7.7/7.8); scale-out
  repo daemon (zero-downtime deploy, HA) (7.8); queue + artifact workers with
  publish contract (7.8); artifact authorization via signed URLs (7.7).
- **Depends on:** M0 (tenancy, authz).
- **Exit:** push→queue→validated cloud artifact; no single-share SPOF; artifact
  reads authorized + audited.

### M3 — Data governance
- Residency pinning, envelope + optional per-tenant CMK, retention, DSR/deletion,
  legal hold (7.9); backups + DR game-day + RPO/RTO (7.10-DR).
- **Depends on:** M2 (object store is the substrate).
- **Exit:** EU-pinned tenant evidenced; tested restore within RTO/RPO; legal hold
  blocks deletion; DSR export works.

### M4 — Supply chain & release integrity
- Code signing/notarization, SLSA provenance, SBOM, updater signature check,
  signed server images, CI deploy off long-lived keys (7.11).
- **Depends on:** none hard (can run parallel to M1–M3); gates GA to enterprises.
- **Exit:** signed/notarized installers; SBOM + provenance per release; deploy
  verifies signatures.

### M5 — Ecosystem & operability
- Public API + scoped tokens + webhooks (7.13); support diagnostics bundle +
  break-glass + status page (7.14); SLOs formalized with on-call/error budgets
  (7.10-obs); migration/versioning policy + Azure Files→store importer (7.15).
- **Depends on:** M2 (data plane), M0 (authz/audit).
- **Exit:** tenant integrates via API/webhooks; support runbook exercised; agent
  compatibility policy enforced.

### M6 — Compliance certification
- SOC 2 readiness → Type I; GDPR artifacts (DPA/ROPA/sub-processors); access
  reviews; policies; then Type II observation window (7.12).
- **Depends on:** M0–M5 (they produce the evidence).
- **Exit:** Type I complete; Type II window started; security questionnaires
  answerable.

### Critical dependency graph
```
M0 ──┬─▶ M1 ──────────────┐
     ├─▶ M2 ─▶ M3          ├─▶ M6 (evidence from all)
     │        └─▶ M5       │
     └───────── M4 ────────┘   (M4 parallel; gates enterprise GA)
```
**Hard prerequisites:** M0 before all; M2 before M3 and M5; audit (M0) + backups
(M3) + supply chain (M4) all feed M6. **Do not** start M6 certification before
M0–M4 controls are operational — you cannot evidence controls that do not run.

### Cross-cutting critical dependencies / risks
- **architur ownership**: most of M0/M1/M5/M6 is control-plane work outside these
  repos; align the versioned `/api/internal/vex/*` contract early so vex-serve/
  bridge changes are additive.
- **Single-share/single-ACI SPOF** blocks any real availability SLO — M2 must
  land before promising RPO/RTO.
- **Expired `planmorph.software` domain** + hard-coded ACA FQDNs: fix DNS/branding
  before enterprise launch (cert/trust + email deliverability).
- **`VEX_INTERNAL_SECRET` / ACR admin creds / storage-account keys** in the
  current deploy are rotation/leak risks — address in M0/M4.

---

## 9. Consolidated launch acceptance gate

Enterprise production launch requires **all** of:
1. Deny-by-default authz with per-decision audit; cross-tenant isolation
   red-team passed (7.1/7.5/7.6).
2. SSO (OIDC+SAML) + SCIM with ≤5-min effective deprovision (7.3).
3. Device revocation + rotation effective within SLA, audited (7.4).
4. Object store as source of truth, HA repo daemon (zero-downtime deploy),
   queue-driven artifact workers with atomic validated publish (7.8).
5. Artifact access via short-TTL, single-object signed URLs, authorized +
   audited (7.7).
6. Residency pinning + at-rest encryption + retention + legal hold + DSR (7.9).
7. Tested backup/restore meeting a published RPO/RTO; DR game-day passed (7.10).
8. Signed/notarized installers + SBOM + provenance + hardened updater (7.11).
9. SLOs published with alerting + on-call; correlation IDs end-to-end (7.10).
10. Support diagnostics + audited break-glass; status page (7.14).
11. Backward-compatible versioned contracts + agent compatibility policy (7.15).
12. SOC 2 Type I complete + GDPR artifacts in place; Type II window running
    (7.12).

---

## 10. Explicitly NOT for the initial production launch

Deliberately out of scope to keep the first enterprise launch shippable and
low-risk. Each is a conscious deferral, not an oversight.

- **Self-hosted / on-prem / air-gapped deployments.** Single-tenant *dedicated*
  cloud (bucket/keys/daemon) is the isolation escape hatch instead. On-prem
  multiplies the ops/support/compliance surface enormously.
- **Bring-your-own-KMS / HSM / full customer-managed encryption stack** beyond
  a single per-tenant CMK option. No per-object customer keys at launch.
- **Real-time multi-user collaboration / live co-editing / CRDT merge.** Vex is
  push/pull (Git-like) by design; keep it that way initially.
- **Automatic / continuous background push.** Push stays user-determined (as
  built) — do not add silent auto-sync.
- **Self-modifying / auto-updating engine that rewrites binaries** — explicitly
  rejected in the repo's design rationale; keep signed static artifacts.
- **Native CAD plugins (Revit/AutoCAD/etc.) as launch dependencies.** The IFC
  inbox (Tier 1) is the MVP path; plugins remain examples, and Autodesk-store
  review must not gate enterprise conversations.
- **Server-side heavy geometry / native render engine rewrite.** Ship the
  existing web-ifc worker behind the publish contract; swap engines later after
  profiling — do not block launch on it.
- **Global multi-region active-active + automated cross-region failover.** Start
  with per-tenant region pinning + documented manual failover + tested restore.
- **SOC 2 Type II certificate (as opposed to readiness/Type I).** Type II needs
  a multi-month observation window; start it, don't gate launch on the cert.
- **Marketplace/partner ecosystem, billing/metering platform, and a broad
  public plugin/app store.** Ship scoped API + webhooks first; commercialize the
  ecosystem later.
- **Fine-grained per-element/property ACLs inside a model.** Authorization is at
  repo/project granularity at launch; sub-model ACLs are a large, separate effort.
- **Customer-configurable custom roles/policy DSL.** Ship the fixed built-in role
  set (7.2) first; custom roles later.
- **Mobile clients / browser-only (no-agent) upload at enterprise scale.** The
  desktop agent remains the supported ingestion path initially.

---

*End of roadmap.*
