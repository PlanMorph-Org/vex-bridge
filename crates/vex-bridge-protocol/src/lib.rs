//! Wire types shared between the `vex-bridge` daemon and CAD-plugin shells.
//!
//! The daemon's HTTP API is intentionally tiny so a 50-line plugin in any
//! language can drive it. Every request and response is JSON; every endpoint
//! is idempotent where it can be.
//!
//! All endpoints are under `http://127.0.0.1:7878/v1/`. Plugins must include
//! the contents of `~/.vex-bridge/access-token` as the
//! `X-Vex-Bridge-Token` header — this is a per-user secret that a malicious
//! webpage cannot read, so it stops drive-by attacks against `localhost`.

use serde::{Deserialize, Serialize};

pub mod schema {
    //! Cross-service contract versions. The bridge is a *consumer* of the
    //! `vex` engine's JSON; asserting the version at the boundary turns a
    //! whole class of silent cross-version drift bugs into one loud, early
    //! error instead of a corrupt payload forwarded to the web UI.

    /// Full schema tag emitted by `vex-visual-diff` (e.g. `vex --json changes`).
    pub const VISUAL_DIFF: &str = "vex.visual-diff/1";

    /// Manifest for derived, commit-exact IFC render artifacts.
    ///
    /// Render artifacts are a cache of the canonical IFC graph, not a source
    /// of semantic truth.
    ///
    /// `v1` carries a single manifest-global semantic index. It stays readable
    /// and valid so already-published artifacts never need regeneration.
    pub const RENDER_MANIFEST: &str = "vex.render-manifest/1";

    /// Second-generation render manifest.
    ///
    /// `v2` names the renderer policy identity (so distinct policies stay
    /// identity-safe) and moves the semantic index to be tile-local. It still
    /// emits only the single full-model tile initially.
    pub const RENDER_MANIFEST_V2: &str = "vex.render-manifest/2";

    /// Base namespace for a renderer policy identity.
    ///
    /// A v2 manifest binds its artifacts to a namespaced policy tag (for
    /// example `vex.render-policy.full-model/1`) plus a content hash of the
    /// policy parameters, so changing the renderer configuration produces a
    /// distinct, non-colliding identity.
    pub const RENDER_POLICY: &str = "vex.render-policy/1";

    /// Status payload for derived IFC render-artifact generation.
    pub const RENDER_STATUS: &str = "vex.render-status/1";

    /// Semantic lookup index associated with a render-artifact manifest.
    pub const RENDER_SEMANTIC_INDEX: &str = "vex.render-semantic-index/1";

    /// Durable, versioned local-first federation coordination-set model.
    ///
    /// A federation is a *derived, saved* set of exact discipline-model commit
    /// references plus placement transforms. It is deliberately not a merged
    /// authoring model, so the schema is versioned independently of the
    /// per-discipline project history it references.
    pub const FEDERATION: &str = "vex.federation/1";

    /// Resolved federation snapshot: exact, immutable member commit identities
    /// plus per-member artifact status, so a viewer can load each discipline
    /// independently without any semantic merge.
    pub const FEDERATION_SNAPSHOT: &str = "vex.federation-snapshot/1";

    /// Split a `name/major` schema tag into `(name, major)`.
    ///
    /// `"vex.visual-diff/1"` → `Some(("vex.visual-diff", 1))`.
    pub fn parse_tag(tag: &str) -> Option<(&str, u32)> {
        let (name, major) = tag.rsplit_once('/')?;
        let major: u32 = major.trim().parse().ok()?;
        Some((name, major))
    }

    /// True when `tag` shares the same schema name and major version as
    /// `expected`. Patch/minor drift (a longer tag with extra fields) is
    /// tolerated; a different name or major is rejected.
    pub fn is_compatible(tag: &str, expected: &str) -> bool {
        match (parse_tag(tag), parse_tag(expected)) {
            (Some((n1, m1)), Some((n2, m2))) => n1 == n2 && m1 == m2,
            _ => tag == expected,
        }
    }
}

/// `GET /v1/health` — daemon liveness + version probe. Unauthenticated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub version: String,
    pub paired: bool,
    /// Path the daemon will exec when shelling out to vex.
    pub vex_bin: String,
    pub vex_version: Option<String>,
    /// Visual diff schema this bridge expects from the bundled engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_visual_diff_schema: Option<String>,
    /// True when the running engine reports a schema compatible with
    /// `expected_visual_diff_schema`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vex_schema_compatible: Option<bool>,
    pub uptime_seconds: u64,
}

/// `GET /v1/update/check` — does a newer bridge release exist?
///
/// The daemon queries the GitHub Releases API and compares the latest release
/// on the requested channel with its own build version. This drives the
/// in-app "update available" UI so users no longer have to uninstall and
/// reinstall to move between builds. Network/parse failures populate `error`
/// instead of failing the request, so a flaky lookup never breaks the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// Version this daemon is built from (`CARGO_PKG_VERSION`).
    pub current_version: String,
    /// Latest version available on the channel, if the lookup succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    /// True when `latest_version` is strictly newer than `current_version`.
    pub update_available: bool,
    /// Release channel that was queried: `stable` or `canary`.
    pub channel: String,
    /// HTML URL of the latest release (for the "view release" CTA).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_url: Option<String>,
    /// Markdown release notes of the latest release, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_notes: Option<String>,
    /// RFC3339 publish timestamp of the latest release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// Download URL of the platform installer asset for the latest release, if
    /// one exists for this OS (e.g. the Windows `VexAtlasSetup-*.exe`). Drives
    /// the one-click "Download & install" apply flow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installer_url: Option<String>,
    /// Lowercase hex SHA-256 of the installer asset, parsed from the release's
    /// published `SHA256SUMS.txt`. The daemon refuses to launch an installer
    /// whose bytes do not match this digest, so apply is integrity-checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installer_sha256: Option<String>,
    /// True when the daemon can perform an in-app apply on this platform (the
    /// installer asset exists, has a verified digest, and the OS is supported).
    #[serde(default)]
    pub can_apply: bool,
    /// RFC3339 time this check was performed (or served from cache).
    pub checked_at: String,
    /// Human-readable reason the lookup could not complete, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `POST /v1/update/apply` — result of an in-app update attempt.
///
/// On success the daemon has downloaded the installer, verified its SHA-256
/// against the release manifest, and launched it detached; the installer then
/// closes the running app, swaps the binaries, and relaunches. `launched` is
/// false when apply is unavailable (unsupported OS, missing/unverified asset),
/// in which case `release_url` lets the UI fall back to a manual download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateApplyResponse {
    /// True when the verified installer was launched.
    pub launched: bool,
    /// Version the installer will move the user to, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_version: Option<String>,
    /// Release page to open when an in-app apply is not possible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_url: Option<String>,
    /// Human-readable reason apply did not launch, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `POST /v1/pair/start` — kick off pairing with the architur API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairStartRequest {
    /// Human label that will appear in the user's account, e.g.
    /// `"Revit on Larry's MacBook"`.
    pub device_label: String,
    /// Ask the native daemon to open the pairing URL. This avoids browser
    /// popup blockers in app-mode desktop windows.
    #[serde(default)]
    pub open_browser: bool,
}

/// Response: shows the user a code and a URL to open in the browser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairStartResponse {
    pub code: String,
    pub pair_url: String,
    pub expires_at: String, // RFC3339
}

/// `GET /v1/pair/status` — current pairing state of the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum PairStatus {
    Unpaired,
    Pending {
        code: String,
        pair_url: String,
        expires_at: String,
    },
    Paired {
        device_label: String,
        key_fingerprint: String,
        paired_at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        account_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        account_email: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        account_name: Option<String>,
    },
}

/// `POST /v1/repo/init` — create a local vex repo for a project.
/// The daemon prompts the user (via tray UI in v2) to confirm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInitRequest {
    pub project_id: String,
    pub project_name: String,
    /// Where on disk to put the repo. If absent, daemon picks the default
    /// under `~/Architur/<project_name>`.
    pub local_path: Option<String>,
    /// `ssh://vex@<host>/<slug>` — the architur-hosted remote.
    pub remote_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInitResponse {
    pub local_path: String,
    pub already_existed: bool,
}

/// `POST /v1/repo/commit` — commit the current state of the repo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRequest {
    pub project_id: String,
    pub message: String,
    /// Optional: name + email to record. Falls back to daemon defaults.
    pub author_name: Option<String>,
    pub author_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitResponse {
    pub commit_hash: String,
    pub files_changed: u32,
}

/// `POST /v1/repo/push` — push the local repo to the architur remote.
/// Streams a sequence of [`PushEvent`] frames as `application/x-ndjson`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRequest {
    pub project_id: String,
    pub branch: Option<String>, // default: current branch
    /// Architur repository GUID selected as this local project's destination.
    #[serde(default)]
    pub cloud_project_id: Option<String>,
}

/// Cloud repository available to the paired account for desktop pushes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudProject {
    pub id: String,
    pub owner_slug: String,
    pub owner_name: String,
    pub slug: String,
    pub full_name: String,
    pub default_branch: String,
    pub has_commits: bool,
}

/// `POST /v1/repo/register` — tell the daemon which local directory backs
/// a given architur project. Idempotent: sending the same `project_id`
/// twice replaces the previous entry. The architur web UI calls this on
/// the user's behalf when they open a project for the first time, so the
/// architect never has to hand-edit `config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRegisterRequest {
    pub project_id: String,
    /// Where on disk to put the repo. If absent, the daemon picks the
    /// default under `<home>/Architur/<project_id>`.
    pub local_path: Option<String>,
    /// File globs to commit. Defaults to `["*.ifc"]` if omitted.
    pub include: Option<Vec<String>>,
    /// Optional IFC `IfcProject.GlobalId` or `fingerprint:<hash>` route.
    #[serde(default)]
    pub ifc_project_guid: Option<String>,
    /// Optional human project name for local UI and generated messages.
    #[serde(default)]
    pub project_name: Option<String>,
    /// When false (default), attempting to remap an existing `project_id`
    /// to a different local path is rejected with a conflict.
    #[serde(default)]
    pub allow_replace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRegisterResponse {
    pub project_id: String,
    pub local_path: String,
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ifc_project_guid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    /// True if a previous entry for this project_id was replaced.
    pub replaced: bool,
    /// True if the daemon has an active watcher for this project now.
    #[serde(default)]
    pub watching: bool,
    /// True when the project id was auto-generated by the daemon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id_auto_generated: Option<bool>,
}

/// `GET /v1/setup/status` — first-run UI state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupStatus {
    pub paired: bool,
    pub pair_status: PairStatus,
    pub default_device_label: String,
    pub inbox_root_path: String,
    pub needs_inbox: bool,
    pub suggested_inbox_path: String,
    pub config_path: String,
    pub state_path: String,
    pub watch: WatchStatus,
}

/// `POST /v1/setup/inbox` — create/update the initial watched IFC inbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupInboxRequest {
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub project_name: Option<String>,
    /// Folder name under `~/VexInbox`. If absent, daemon derives it from the
    /// project name or id.
    #[serde(default)]
    pub folder_name: Option<String>,
    /// Back-compat: treated as a folder name or a path already inside
    /// `~/VexInbox`; external absolute paths are rejected.
    #[serde(default)]
    pub local_path: Option<String>,
    #[serde(default)]
    pub include: Option<Vec<String>>,
    #[serde(default)]
    pub ifc_project_guid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupInboxResponse {
    pub repo: RepoRegisterResponse,
    pub watch: WatchStatus,
}

/// `GET /v1/watch/status` — status payload for tray/menu/dashboard UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchStatus {
    pub active_watchers: usize,
    pub configured_projects: usize,
    pub seen_ifc_hash_count: usize,
    /// Commits committed locally but not yet pushed to the remote. Pushing is
    /// user-determined, so this is the count of unpushed commits waiting for
    /// the user to press "Push" — surfaced so the UI can show "N ready to push".
    #[serde(default)]
    pub pending_push_count: usize,
    pub projects: Vec<ProjectSummary>,
}

/// Project row for local desktop UI surfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    pub local_path: String,
    pub path_exists: bool,
    pub active: bool,
    pub include: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ifc_project_guid: Option<String>,
    pub seen_import_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_imported_at_unix: Option<i64>,
    /// Locally-committed but not-yet-pushed commits for this project. Pushing is
    /// user-determined; the dashboard uses this to badge the "Push" button and
    /// show "N ready to push" per project.
    #[serde(default)]
    pub pending_push_count: usize,
    /// Architur repository GUID chosen independently from the local project id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_project_id: Option<String>,
}

/// `GET /v1/projects/:project_id/history` — commit list for dashboard history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectHistoryResponse {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    pub commits: Vec<CommitSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSummary {
    pub commit: String,
    pub author: String,
    pub email: String,
    pub timestamp: i64,
    pub message: String,
    pub parents: Vec<String>,
}

/// `GET /v1/projects/:project_id/changes` — visual diff for 2D/3D UI.
/// Optional query params: `from=<commit>&to=<commit>`. Without them, the
/// daemon compares the latest commit against its parent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectChangesResponse {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caught_at_unix: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_commit: Option<String>,
    /// Usually the `vex.visual-diff/1` JSON returned by `vex --json changes`.
    pub visual_diff: serde_json::Value,
}

/// A content-addressed, immutable object served by the render-artifact layer.
///
/// `uri` identifies the object endpoint, while `sha256` lets clients verify
/// that a cached or downloaded object is exactly the artifact named by its
/// manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderArtifactResource {
    pub uri: String,
    pub content_type: String,
    /// Lowercase hexadecimal SHA-256 of the response body.
    pub sha256: String,
    pub byte_length: u64,
}

/// Axis-aligned bounds in the render coordinate system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderBounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// One immutable render tile referenced from a [`RenderArtifactManifest`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderTileDescriptor {
    /// Stable manifest-local identifier used by the semantic index.
    pub tile_id: String,
    /// Level of detail. `0` is the exact, pickable base rendition; larger
    /// values are coarser, lower-detail proxies for the same content (the
    /// authoritative fidelity signal is `geometric_error`, where `0` means
    /// exact geometry).
    pub lod: u32,
    /// Stable ownership key shared by every LOD tile that renders the same
    /// content (v2 manifests). When present it distinguishes which tiles are
    /// alternate levels of detail of one logical group (for example a storey)
    /// from the tile's own per-LOD `tile_id`. Absent when a tile is the sole
    /// LOD for its content, preserving the original flat manifest shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub bounds: RenderBounds,
    /// Maximum geometric approximation error for this tile, in model units.
    pub geometric_error: f64,
    pub artifact: RenderArtifactResource,
    /// Tile-local semantic index (v2 manifests). Absent in v1, where the
    /// semantic index is a single manifest-global descriptor instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_index: Option<RenderSemanticIndexDescriptor>,
}

/// Immutable semantic lookup data for selection and element-to-tile mapping.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderSemanticIndexDescriptor {
    /// Schema of the index object itself.
    pub schema: String,
    /// Number of indexed semantic IFC entities.
    pub entry_count: u64,
    pub artifact: RenderArtifactResource,
}

/// Namespaced, content-hashed identity of the renderer policy that produced a
/// v2 artifact set.
///
/// `id` is a namespaced `name/major` tag (for example
/// `vex.render-policy.full-model/1`) so policies can be introduced without
/// clashing, and `hash` is a lowercase SHA-256 over the policy parameters so a
/// configuration change yields a distinct, identity-safe artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderPolicy {
    pub id: String,
    pub hash: String,
}

/// Commit-exact manifest for derived IFC render artifacts.
///
/// The manifest only describes derived content. `commit_hash` remains the
/// authoritative semantic model identity; each referenced object is immutable
/// and independently integrity-checkable.
///
/// The manifest is versioned by its `schema` tag. `v1` carries a single
/// manifest-global `semantic_index`; `v2` names a `render_policy` and moves the
/// semantic index to be tile-local (`RenderTileDescriptor::semantic_index`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderArtifactManifest {
    pub schema: String,
    pub project_id: String,
    /// Full resolved Vex commit hash used to derive this artifact set.
    pub commit_hash: String,
    /// Content-addressed identifier for this manifest revision.
    pub artifact_id: String,
    /// RFC3339 time the artifact set was generated.
    pub generated_at: String,
    /// Renderer policy identity (v2 manifests). Absent in v1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_policy: Option<RenderPolicy>,
    pub tiles: Vec<RenderTileDescriptor>,
    /// Manifest-global semantic index (v1 manifests). Absent in v2, where the
    /// semantic index is tile-local instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_index: Option<RenderSemanticIndexDescriptor>,
}

/// Lifecycle state of a derived IFC render-artifact build.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum RenderArtifactStatus {
    /// No derived artifact has been requested or published for this commit.
    NotRequested,
    Queued,
    Building {
        /// Number of immutable tile objects successfully written so far.
        completed_tiles: u32,
        /// Total tiles expected, when the renderer can determine it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        total_tiles: Option<u32>,
    },
    Ready {
        manifest: Box<RenderArtifactManifest>,
    },
    Failed {
        message: String,
        #[serde(default)]
        retryable: bool,
    },
}

/// Versioned response envelope for render-artifact status endpoints.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderArtifactStatusResponse {
    pub schema: String,
    pub project_id: String,
    pub commit_hash: String,
    #[serde(flatten)]
    pub status: RenderArtifactStatus,
}

/// Validation failure for a render-artifact manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderArtifactValidationError {
    IncompatibleSchema {
        field: &'static str,
        expected: &'static str,
        actual: String,
    },
    MissingField(&'static str),
    /// A field is present but not permitted by the manifest's schema version
    /// (for example a v1 manifest-global `semantic_index` in a v2 manifest).
    UnexpectedField {
        field: &'static str,
        schema: String,
    },
    /// A renderer policy identity is not a namespaced `name/major` tag.
    InvalidPolicyId {
        actual: String,
    },
    DuplicateTileId(String),
    InvalidSha256 {
        resource: String,
    },
    InvalidBounds {
        tile_id: String,
    },
    InvalidGeometricError {
        tile_id: String,
    },
}

impl std::fmt::Display for RenderArtifactValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IncompatibleSchema {
                field,
                expected,
                actual,
            } => write!(
                f,
                "{field} schema {actual:?} is not compatible with {expected:?}"
            ),
            Self::MissingField(field) => write!(f, "{field} must not be empty"),
            Self::UnexpectedField { field, schema } => {
                write!(f, "{field} is not permitted by manifest schema {schema:?}")
            }
            Self::InvalidPolicyId { actual } => {
                write!(f, "render policy id {actual:?} is not a namespaced tag")
            }
            Self::DuplicateTileId(tile_id) => write!(f, "tile_id {tile_id:?} is duplicated"),
            Self::InvalidSha256 { resource } => {
                write!(f, "resource {resource:?} does not have a lowercase SHA-256")
            }
            Self::InvalidBounds { tile_id } => write!(f, "tile {tile_id:?} has invalid bounds"),
            Self::InvalidGeometricError { tile_id } => {
                write!(f, "tile {tile_id:?} has an invalid geometric error")
            }
        }
    }
}

impl std::error::Error for RenderArtifactValidationError {}

impl RenderArtifactManifest {
    /// Iterate every immutable resource declared anywhere in the manifest:
    /// each tile artifact, each tile-local semantic index (v2), and the
    /// manifest-global semantic index (v1). Centralizing enumeration keeps
    /// validation, publication, and serving in agreement about exactly which
    /// objects a manifest declares.
    pub fn resources(&self) -> impl Iterator<Item = &RenderArtifactResource> {
        self.tiles
            .iter()
            .flat_map(|tile| {
                std::iter::once(&tile.artifact)
                    .chain(tile.semantic_index.iter().map(|index| &index.artifact))
            })
            .chain(self.semantic_index.iter().map(|index| &index.artifact))
    }

    /// Mutable counterpart to [`resources`](Self::resources), used to rewrite
    /// worker-supplied URIs into canonical, bridge-owned object endpoints.
    pub fn resources_mut(&mut self) -> impl Iterator<Item = &mut RenderArtifactResource> {
        self.tiles
            .iter_mut()
            .flat_map(|tile| {
                std::iter::once(&mut tile.artifact).chain(
                    tile.semantic_index
                        .iter_mut()
                        .map(|index| &mut index.artifact),
                )
            })
            .chain(
                self.semantic_index
                    .iter_mut()
                    .map(|index| &mut index.artifact),
            )
    }

    /// Reject malformed manifests before caching or serving their immutable
    /// artifacts. This deliberately validates the envelope, not remote bytes,
    /// and enforces the structural policy of the manifest's schema version.
    ///
    /// Every declared resource (tile, tile-local index, and manifest-global
    /// index) is validated, so an unsafe or mismatched object cannot slip in
    /// under a newer schema shape.
    pub fn validate(&self) -> Result<(), RenderArtifactValidationError> {
        let major = validate_manifest_schema(&self.schema)?;
        validate_non_empty("project_id", &self.project_id)?;
        validate_non_empty("commit_hash", &self.commit_hash)?;
        validate_non_empty("artifact_id", &self.artifact_id)?;
        validate_non_empty("generated_at", &self.generated_at)?;

        match major {
            1 => {
                // v1 carries one manifest-global semantic index and no policy.
                if self.render_policy.is_some() {
                    return Err(RenderArtifactValidationError::UnexpectedField {
                        field: "render_policy",
                        schema: self.schema.clone(),
                    });
                }
                let index = self.semantic_index.as_ref().ok_or(
                    RenderArtifactValidationError::MissingField("semantic_index"),
                )?;
                validate_semantic_index("semantic_index", index)?;
            }
            _ => {
                // v2 names a policy identity and moves the index to be
                // tile-local.
                let policy = self
                    .render_policy
                    .as_ref()
                    .ok_or(RenderArtifactValidationError::MissingField("render_policy"))?;
                validate_render_policy(policy)?;
                if self.semantic_index.is_some() {
                    return Err(RenderArtifactValidationError::UnexpectedField {
                        field: "semantic_index",
                        schema: self.schema.clone(),
                    });
                }
            }
        }

        let mut tile_ids = std::collections::HashSet::with_capacity(self.tiles.len());
        for tile in &self.tiles {
            validate_non_empty("tile_id", &tile.tile_id)?;
            if !tile_ids.insert(&tile.tile_id) {
                return Err(RenderArtifactValidationError::DuplicateTileId(
                    tile.tile_id.clone(),
                ));
            }
            if !tile.geometric_error.is_finite() || tile.geometric_error < 0.0 {
                return Err(RenderArtifactValidationError::InvalidGeometricError {
                    tile_id: tile.tile_id.clone(),
                });
            }
            if !tile
                .bounds
                .min
                .iter()
                .chain(tile.bounds.max.iter())
                .all(|v| v.is_finite())
                || tile
                    .bounds
                    .min
                    .iter()
                    .zip(tile.bounds.max.iter())
                    .any(|(min, max)| min > max)
            {
                return Err(RenderArtifactValidationError::InvalidBounds {
                    tile_id: tile.tile_id.clone(),
                });
            }
            validate_resource(&format!("tile:{}", tile.tile_id), &tile.artifact)?;

            match major {
                1 => {
                    if tile.semantic_index.is_some() {
                        return Err(RenderArtifactValidationError::UnexpectedField {
                            field: "tile.semantic_index",
                            schema: self.schema.clone(),
                        });
                    }
                    if tile.group.is_some() {
                        return Err(RenderArtifactValidationError::UnexpectedField {
                            field: "tile.group",
                            schema: self.schema.clone(),
                        });
                    }
                }
                _ => {
                    let index = tile.semantic_index.as_ref().ok_or(
                        RenderArtifactValidationError::MissingField("tile.semantic_index"),
                    )?;
                    validate_semantic_index(
                        &format!("tile:{}:semantic_index", tile.tile_id),
                        index,
                    )?;
                    if let Some(group) = tile.group.as_ref() {
                        validate_non_empty("tile.group", group)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Accept either supported manifest schema version, returning its major.
fn validate_manifest_schema(actual: &str) -> Result<u32, RenderArtifactValidationError> {
    if schema::is_compatible(actual, schema::RENDER_MANIFEST) {
        Ok(1)
    } else if schema::is_compatible(actual, schema::RENDER_MANIFEST_V2) {
        Ok(2)
    } else {
        Err(RenderArtifactValidationError::IncompatibleSchema {
            field: "manifest",
            expected: schema::RENDER_MANIFEST_V2,
            actual: actual.to_string(),
        })
    }
}

fn validate_semantic_index(
    label: &str,
    index: &RenderSemanticIndexDescriptor,
) -> Result<(), RenderArtifactValidationError> {
    validate_schema(
        "semantic_index",
        &index.schema,
        schema::RENDER_SEMANTIC_INDEX,
    )?;
    validate_resource(label, &index.artifact)
}

fn validate_render_policy(policy: &RenderPolicy) -> Result<(), RenderArtifactValidationError> {
    validate_non_empty("render_policy.id", &policy.id)?;
    match schema::parse_tag(&policy.id) {
        Some((name, _)) if !name.trim().is_empty() => {}
        _ => {
            return Err(RenderArtifactValidationError::InvalidPolicyId {
                actual: policy.id.clone(),
            })
        }
    }
    if policy.hash.len() != 64
        || !policy
            .hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RenderArtifactValidationError::InvalidSha256 {
            resource: "render_policy.hash".to_string(),
        });
    }
    Ok(())
}

fn validate_schema(
    field: &'static str,
    actual: &str,
    expected: &'static str,
) -> Result<(), RenderArtifactValidationError> {
    if schema::is_compatible(actual, expected) {
        Ok(())
    } else {
        Err(RenderArtifactValidationError::IncompatibleSchema {
            field,
            expected,
            actual: actual.to_string(),
        })
    }
}

fn validate_non_empty(
    field: &'static str,
    value: &str,
) -> Result<(), RenderArtifactValidationError> {
    if value.trim().is_empty() {
        Err(RenderArtifactValidationError::MissingField(field))
    } else {
        Ok(())
    }
}

fn validate_resource(
    label: &str,
    resource: &RenderArtifactResource,
) -> Result<(), RenderArtifactValidationError> {
    validate_non_empty("resource.uri", &resource.uri)?;
    validate_non_empty("resource.content_type", &resource.content_type)?;
    if resource.sha256.len() != 64
        || !resource
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RenderArtifactValidationError::InvalidSha256 {
            resource: label.to_string(),
        });
    }
    Ok(())
}

/// `DELETE /v1/projects/:project_id` — remove a watched project from Vex
/// Desktop and stop its watcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteProjectRequest {
    /// `keep_folder` leaves files alone, `archive_folder` renames the folder,
    /// and `delete_folder` permanently removes it after server-side safety
    /// checks.
    #[serde(default = "default_delete_project_policy")]
    pub deletion_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteProjectResponse {
    pub project_id: String,
    pub local_folder: String,
    pub deletion_policy: String,
    pub removed_from_config: bool,
    pub watcher_stopped: bool,
    pub folder_action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resulting_folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_error: Option<String>,
}

fn default_delete_project_policy() -> String {
    "keep_folder".to_string()
}

/// `GET /v1/activity/recent` — recent daemon activity for tray/dashboard UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentActivityResponse {
    /// Newest event first.
    pub events: Vec<ActivityEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityKind {
    ProcessingStarted,
    CommitCreated,
    DuplicateSkipped,
    RouteSkipped,
    NoChanges,
    /// Commit landed locally but the push failed and was queued in the
    /// durable outbox for retry. The local history is safe; sync is pending.
    PushQueued,
    /// A previously queued push was retried successfully by the outbox.
    PushSynced,
    Error,
}

/// One locally observed event in the IFC intake pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub id: String,
    pub kind: ActivityKind,
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub caught_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PushEvent {
    Started,
    Progress { phase: String, percent: u8 },
    Done { commit_hash: String },
    Error { message: String },
}

/// Standard JSON error envelope. Used for any non-2xx response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
    /// Stable machine-readable code (snake_case).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Suggested user action shown by desktop clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Whether a retry has a reasonable chance of succeeding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    /// Per-response id to correlate UI failures with daemon logs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Federation: local-first, derived model-coordination sets.
//
// A federation binds several *independent* local discipline projects together
// as a saved coordination set. Each member is an immutable reference to an
// exact commit of a source project plus the affine transform that positions it
// in the federation's shared coordinate space. Federations never merge the
// underlying models and make **no semantic-merge guarantee**: they are purely a
// placement-and-versioning overlay so a viewer can load each discipline
// independently at its pinned commit. No member ever references a remote URL or
// an arbitrary filesystem path — only a local project id and a commit hash.
// ---------------------------------------------------------------------------

/// A 4x4, row-major affine coordinate transform applied to a discipline model
/// as it is placed into a federation's shared coordinate space.
///
/// The transform is affine only: the bottom row must equal `[0, 0, 0, 1]` so it
/// carries no perspective component, and its upper-left 3x3 linear block must
/// be invertible so a placement can be inverted (for example to map a picked
/// federation-space point back into a member model). Every entry must be
/// finite.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FederationTransform {
    /// Row-major 4x4 matrix. `rows[3]` (the bottom row) must equal
    /// `[0.0, 0.0, 0.0, 1.0]`.
    pub rows: [[f64; 4]; 4],
}

impl Default for FederationTransform {
    fn default() -> Self {
        Self::identity()
    }
}

impl FederationTransform {
    /// Tolerance for the affine bottom-row check.
    const PERSPECTIVE_EPSILON: f64 = 1e-9;
    /// A linear block whose determinant magnitude is at or below this is
    /// treated as singular (non-invertible).
    const DETERMINANT_EPSILON: f64 = 1e-9;

    /// The identity placement: no translation, rotation, scale, or shear.
    pub const fn identity() -> Self {
        Self {
            rows: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    fn all_finite(&self) -> bool {
        self.rows.iter().flatten().all(|value| value.is_finite())
    }

    fn has_affine_bottom_row(&self) -> bool {
        let [a, b, c, d] = self.rows[3];
        a.abs() <= Self::PERSPECTIVE_EPSILON
            && b.abs() <= Self::PERSPECTIVE_EPSILON
            && c.abs() <= Self::PERSPECTIVE_EPSILON
            && (d - 1.0).abs() <= Self::PERSPECTIVE_EPSILON
    }

    /// Determinant of the upper-left 3x3 linear block.
    fn linear_determinant(&self) -> f64 {
        let m = &self.rows;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    /// True when the transform is a finite, invertible affine map with no
    /// perspective component.
    pub fn is_valid_affine(&self) -> bool {
        if !self.all_finite() || !self.has_affine_bottom_row() {
            return false;
        }
        let determinant = self.linear_determinant();
        determinant.is_finite() && determinant.abs() > Self::DETERMINANT_EPSILON
    }
}

/// One discipline model placed in a federation.
///
/// A member is an immutable reference to an exact commit of an independent local
/// project plus the transform that positions it. `member_id` is stable across
/// federation updates so a viewer can track a member's identity over time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederationMember {
    /// Stable, daemon-generated identity, unique within the federation.
    pub member_id: String,
    /// Local project id this member draws from. Never a remote URL or path.
    pub source_project_id: String,
    /// Immutable, complete (64 hex character) commit hash pinned for this
    /// member. Resolved from the source project HEAD only at create/update.
    pub commit_hash: String,
    /// Human-facing label shown in coordination UIs.
    pub display_name: String,
    /// Optional discipline tag (for example `architecture`, `structure`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discipline: Option<String>,
    /// Placement transform into the federation's shared coordinate space.
    #[serde(default)]
    pub transform: FederationTransform,
    /// Whether the member is shown by default when the federation loads.
    pub visible: bool,
}

/// A derived, saved coordination set of exact discipline-model commits and
/// their placement transforms.
///
/// A federation is **not** a merged authoring model and provides no
/// semantic-merge guarantee; it only records which exact commit of each source
/// project participates and where it is placed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Federation {
    /// Model schema tag. Old records without this field default to the current
    /// version on read so already-persisted federations stay loadable.
    #[serde(default = "default_federation_schema")]
    pub schema: String,
    /// Stable, daemon-generated federation identity.
    pub federation_id: String,
    pub name: String,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
    pub members: Vec<FederationMember>,
}

fn default_federation_schema() -> String {
    schema::FEDERATION.to_string()
}

impl Federation {
    /// Maximum members in a single federation. Keeps a federation — and any
    /// snapshot derived from it — bounded regardless of client input.
    pub const MAX_MEMBERS: usize = 64;
    /// Maximum length (in characters) of a federation or member display name.
    pub const MAX_NAME_LEN: usize = 200;
    /// Maximum length (in characters) of an optional discipline tag.
    pub const MAX_DISCIPLINE_LEN: usize = 80;
    /// Maximum length of a daemon-issued or client-supplied identifier.
    pub const MAX_ID_LEN: usize = 120;

    /// Reject a structurally invalid or unsafe federation before it is
    /// persisted or served. This validates the model envelope only; it does
    /// not (and cannot) check that a source project still exists locally or
    /// that a commit is in its history — those are the server's semantic
    /// checks, performed against live repository state.
    pub fn validate(&self) -> Result<(), FederationValidationError> {
        if !schema::is_compatible(&self.schema, schema::FEDERATION) {
            return Err(FederationValidationError::IncompatibleSchema {
                expected: schema::FEDERATION,
                actual: self.schema.clone(),
            });
        }
        if !is_safe_identifier(&self.federation_id) {
            return Err(FederationValidationError::UnsafeId {
                field: "federation_id",
            });
        }
        validate_safe_name("name", &self.name, Self::MAX_NAME_LEN)?;

        if self.members.is_empty() {
            return Err(FederationValidationError::NoMembers);
        }
        if self.members.len() > Self::MAX_MEMBERS {
            return Err(FederationValidationError::TooManyMembers {
                max: Self::MAX_MEMBERS,
            });
        }

        let mut member_ids = std::collections::HashSet::with_capacity(self.members.len());
        let mut source_commits = std::collections::HashSet::with_capacity(self.members.len());
        for member in &self.members {
            if !is_safe_identifier(&member.member_id) {
                return Err(FederationValidationError::UnsafeId { field: "member_id" });
            }
            if !member_ids.insert(member.member_id.as_str()) {
                return Err(FederationValidationError::DuplicateMemberId(
                    member.member_id.clone(),
                ));
            }
            if !is_safe_identifier(&member.source_project_id) {
                return Err(FederationValidationError::UnsafeId {
                    field: "source_project_id",
                });
            }
            if !is_full_commit_hash(&member.commit_hash) {
                return Err(FederationValidationError::InvalidCommitHash {
                    member_id: member.member_id.clone(),
                });
            }
            validate_safe_name("display_name", &member.display_name, Self::MAX_NAME_LEN)?;
            if let Some(discipline) = member.discipline.as_deref() {
                validate_safe_name("discipline", discipline, Self::MAX_DISCIPLINE_LEN)?;
            }
            if !member.transform.is_valid_affine() {
                return Err(FederationValidationError::InvalidTransform {
                    member_id: member.member_id.clone(),
                });
            }
            if !source_commits.insert((
                member.source_project_id.as_str(),
                member.commit_hash.as_str(),
            )) {
                return Err(FederationValidationError::DuplicateSourceCommit {
                    source_project_id: member.source_project_id.clone(),
                    commit_hash: member.commit_hash.clone(),
                });
            }
        }
        Ok(())
    }

    /// Compact, list-friendly projection of this federation.
    pub fn summary(&self) -> FederationSummary {
        FederationSummary {
            federation_id: self.federation_id.clone(),
            name: self.name.clone(),
            member_count: self.members.len(),
            visible_member_count: self.members.iter().filter(|member| member.visible).count(),
            created_at_unix: self.created_at_unix,
            updated_at_unix: self.updated_at_unix,
        }
    }
}

/// True when `value` is a complete, lowercase-or-uppercase 64 hex character
/// commit hash. Federations only ever pin complete commit hashes; a branch
/// name, abbreviated hash, or `HEAD` is never accepted as a member identity.
pub fn is_full_commit_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Identifier safety shared by federation, member, and source-project ids:
/// non-empty, bounded length, and restricted to ASCII alphanumerics plus `-`
/// and `_`, so an id can never encode a path traversal or shell metacharacter.
fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= Federation::MAX_ID_LEN
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

/// A user-facing name is safe when it is non-empty after trimming, is within
/// its length bound, and contains no control characters (which would let a
/// crafted value smuggle newlines or terminal escapes into logs and UIs).
fn validate_safe_name(
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<(), FederationValidationError> {
    if value.trim().is_empty() {
        return Err(FederationValidationError::MissingField(field));
    }
    if value.chars().count() > max {
        return Err(FederationValidationError::TextTooLong { field, max });
    }
    if value.chars().any(|ch| ch.is_control()) {
        return Err(FederationValidationError::UnsafeText { field });
    }
    Ok(())
}

/// Structural / safety validation failure for a [`Federation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FederationValidationError {
    IncompatibleSchema {
        expected: &'static str,
        actual: String,
    },
    MissingField(&'static str),
    UnsafeText {
        field: &'static str,
    },
    TextTooLong {
        field: &'static str,
        max: usize,
    },
    UnsafeId {
        field: &'static str,
    },
    NoMembers,
    TooManyMembers {
        max: usize,
    },
    InvalidCommitHash {
        member_id: String,
    },
    InvalidTransform {
        member_id: String,
    },
    DuplicateMemberId(String),
    DuplicateSourceCommit {
        source_project_id: String,
        commit_hash: String,
    },
}

impl std::fmt::Display for FederationValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IncompatibleSchema { expected, actual } => write!(
                f,
                "federation schema {actual:?} is not compatible with {expected:?}"
            ),
            Self::MissingField(field) => write!(f, "{field} must not be empty"),
            Self::UnsafeText { field } => {
                write!(f, "{field} must not contain control characters")
            }
            Self::TextTooLong { field, max } => {
                write!(f, "{field} is too long (max {max} characters)")
            }
            Self::UnsafeId { field } => write!(
                f,
                "{field} may only contain ASCII letters, numbers, '-' or '_'"
            ),
            Self::NoMembers => write!(f, "a federation must have at least one member"),
            Self::TooManyMembers { max } => {
                write!(f, "a federation may have at most {max} members")
            }
            Self::InvalidCommitHash { member_id } => write!(
                f,
                "member {member_id:?} must pin a complete 64 hex character commit hash"
            ),
            Self::InvalidTransform { member_id } => write!(
                f,
                "member {member_id:?} has a non-finite, singular, or perspective transform"
            ),
            Self::DuplicateMemberId(member_id) => {
                write!(f, "member_id {member_id:?} is duplicated")
            }
            Self::DuplicateSourceCommit {
                source_project_id,
                commit_hash,
            } => write!(
                f,
                "source project {source_project_id:?} commit {commit_hash:?} appears more than once"
            ),
        }
    }
}

impl std::error::Error for FederationValidationError {}

/// Input describing one member when creating or updating a federation.
///
/// The daemon owns identity assignment and never accepts object paths or remote
/// URLs. `member_id` is optional: on update, supplying an existing id preserves
/// that member's stable identity while replacing its fields; omitting it mints a
/// new member.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationMemberInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_id: Option<String>,
    pub source_project_id: String,
    /// Explicit immutable commit hash. When omitted, the daemon resolves the
    /// source project's current HEAD once, at create/update time, and pins it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discipline: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<FederationTransform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

/// `POST /v1/federations` — create a new coordination set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFederationRequest {
    pub name: String,
    #[serde(default)]
    pub members: Vec<FederationMemberInput>,
}

/// `PATCH /v1/federations/:federation_id` — rename and/or replace the member
/// set. When `members` is present it fully replaces the current set: this is
/// the only way to add or remove members, so a removal is always explicit and
/// never silent. When `members` is absent, only the name (if given) changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateFederationRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub members: Option<Vec<FederationMemberInput>>,
}

/// Compact federation row for `GET /v1/federations`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederationSummary {
    pub federation_id: String,
    pub name: String,
    pub member_count: usize,
    pub visible_member_count: usize,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
}

/// `DELETE /v1/federations/:federation_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteFederationResponse {
    pub federation_id: String,
    pub removed: bool,
}

/// Per-member artifact resolution status in a federation snapshot.
///
/// This is a compact hint so a viewer can render a loading state; the exact,
/// authoritative artifact status is always available from the per-project
/// render endpoints using the member's `source_project_id` and `commit_hash`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "render")]
pub enum FederationRenderStatus {
    /// The source project is no longer configured locally, or the pinned commit
    /// is not present in its history — nothing can be resolved for this member.
    Unavailable,
    /// The commit is present but no derived render artifact was requested; a
    /// viewer should fall back to the canonical IFC endpoint.
    NotRequested,
    Queued,
    Building,
    Ready,
    Failed,
}

/// One resolved member in a [`FederationSnapshot`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederationSnapshotMember {
    pub member_id: String,
    pub source_project_id: String,
    pub commit_hash: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discipline: Option<String>,
    pub transform: FederationTransform,
    pub visible: bool,
    /// The source project is still configured locally.
    pub project_available: bool,
    /// The pinned commit is present in the source project's history.
    pub commit_available: bool,
    #[serde(flatten)]
    pub render_status: FederationRenderStatus,
}

/// `GET /v1/federations/:federation_id/snapshot` — the resolved coordination
/// set. Returns each member's exact, immutable commit identity plus its current
/// artifact status so a viewer can load every discipline independently. This is
/// a placement overlay only and performs no semantic merge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederationSnapshot {
    pub schema: String,
    pub federation_id: String,
    pub name: String,
    pub created_at_unix: i64,
    pub updated_at_unix: i64,
    pub members: Vec<FederationSnapshotMember>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn resource(uri: &str) -> RenderArtifactResource {
        RenderArtifactResource {
            uri: uri.to_string(),
            content_type: "application/vnd.vex.render-tile".to_string(),
            sha256: "a".repeat(64),
            byte_length: 42,
        }
    }

    fn manifest() -> RenderArtifactManifest {
        RenderArtifactManifest {
            schema: schema::RENDER_MANIFEST.to_string(),
            project_id: "project-123".to_string(),
            commit_hash: "e".repeat(64),
            artifact_id: "render-abc".to_string(),
            generated_at: "2026-07-17T13:38:07Z".to_string(),
            render_policy: None,
            tiles: vec![RenderTileDescriptor {
                tile_id: "0/0/0".to_string(),
                lod: 0,
                group: None,
                bounds: RenderBounds {
                    min: [0.0, 0.0, 0.0],
                    max: [10.0, 5.0, 3.0],
                },
                geometric_error: 0.0,
                artifact: resource("/v1/render-artifacts/render-abc/tiles/0-0-0"),
                semantic_index: None,
            }],
            semantic_index: Some(RenderSemanticIndexDescriptor {
                schema: schema::RENDER_SEMANTIC_INDEX.to_string(),
                entry_count: 1,
                artifact: resource("/v1/render-artifacts/render-abc/semantic-index"),
            }),
        }
    }

    fn semantic_index() -> RenderSemanticIndexDescriptor {
        RenderSemanticIndexDescriptor {
            schema: schema::RENDER_SEMANTIC_INDEX.to_string(),
            entry_count: 1,
            artifact: resource("/v1/render-artifacts/render-abc/tiles/full-model/semantic-index"),
        }
    }

    fn manifest_v2() -> RenderArtifactManifest {
        RenderArtifactManifest {
            schema: schema::RENDER_MANIFEST_V2.to_string(),
            project_id: "project-123".to_string(),
            commit_hash: "e".repeat(64),
            artifact_id: "render-def".to_string(),
            generated_at: "2026-07-17T13:38:07Z".to_string(),
            render_policy: Some(RenderPolicy {
                id: "vex.render-policy.full-model/1".to_string(),
                hash: "b".repeat(64),
            }),
            tiles: vec![RenderTileDescriptor {
                tile_id: "full-model".to_string(),
                lod: 0,
                group: None,
                bounds: RenderBounds {
                    min: [0.0, 0.0, 0.0],
                    max: [10.0, 5.0, 3.0],
                },
                geometric_error: 0.0,
                artifact: resource("/v1/render-artifacts/render-def/tiles/full-model"),
                semantic_index: Some(semantic_index()),
            }],
            semantic_index: None,
        }
    }

    #[test]
    fn render_status_uses_a_snake_case_status_tag() {
        let status = RenderArtifactStatus::Building {
            completed_tiles: 3,
            total_tiles: Some(8),
        };

        let value = serde_json::to_value(&status).unwrap();

        assert_eq!(
            value,
            json!({
                "status": "building",
                "completed_tiles": 3,
                "total_tiles": 8,
            })
        );
        assert_eq!(
            serde_json::from_value::<RenderArtifactStatus>(value).unwrap(),
            status
        );
    }

    #[test]
    fn render_manifest_validates_immutable_tile_descriptors() {
        let manifest = manifest();

        assert!(manifest.validate().is_ok());
        assert_eq!(
            serde_json::to_value(&manifest).unwrap()["schema"],
            schema::RENDER_MANIFEST
        );
    }

    #[test]
    fn v1_manifest_round_trips_and_omits_v2_only_fields() {
        // A published v1 manifest must stay readable, valid, and serialize
        // without the v2-only render policy or tile-local index fields.
        let manifest = manifest();
        assert!(manifest.validate().is_ok());

        let value = serde_json::to_value(&manifest).unwrap();
        assert!(value.get("render_policy").is_none());
        assert!(value["tiles"][0].get("semantic_index").is_none());
        assert!(value.get("semantic_index").is_some());

        let decoded: RenderArtifactManifest = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, manifest);
    }

    #[test]
    fn render_manifest_rejects_duplicate_tiles_and_incompatible_index_schema() {
        let mut duplicate = manifest();
        duplicate.tiles.push(duplicate.tiles[0].clone());
        assert!(matches!(
            duplicate.validate(),
            Err(RenderArtifactValidationError::DuplicateTileId(tile_id)) if tile_id == "0/0/0"
        ));

        let mut incompatible_index = manifest();
        incompatible_index.semantic_index.as_mut().unwrap().schema =
            "vex.render-semantic-index/2".to_string();
        assert!(matches!(
            incompatible_index.validate(),
            Err(RenderArtifactValidationError::IncompatibleSchema {
                field: "semantic_index",
                ..
            })
        ));
    }

    #[test]
    fn v2_manifest_validates_with_tile_local_index_and_policy() {
        let manifest = manifest_v2();
        assert!(manifest.validate().is_ok());
        assert_eq!(
            serde_json::to_value(&manifest).unwrap()["schema"],
            schema::RENDER_MANIFEST_V2
        );
        // Resource enumeration reaches every declared object.
        assert_eq!(manifest.resources().count(), 2);
    }

    #[test]
    fn v2_manifest_requires_policy_and_tile_local_index() {
        let mut missing_policy = manifest_v2();
        missing_policy.render_policy = None;
        assert!(matches!(
            missing_policy.validate(),
            Err(RenderArtifactValidationError::MissingField("render_policy"))
        ));

        let mut missing_index = manifest_v2();
        missing_index.tiles[0].semantic_index = None;
        assert!(matches!(
            missing_index.validate(),
            Err(RenderArtifactValidationError::MissingField(
                "tile.semantic_index"
            ))
        ));

        let mut bad_policy = manifest_v2();
        bad_policy.render_policy.as_mut().unwrap().id = "full-model".to_string();
        assert!(matches!(
            bad_policy.validate(),
            Err(RenderArtifactValidationError::InvalidPolicyId { .. })
        ));

        let mut bad_policy_hash = manifest_v2();
        bad_policy_hash.render_policy.as_mut().unwrap().hash = "NOT-HEX".to_string();
        assert!(matches!(
            bad_policy_hash.validate(),
            Err(RenderArtifactValidationError::InvalidSha256 { resource }) if resource == "render_policy.hash"
        ));
    }

    #[test]
    fn v2_manifest_rejects_v1_shaped_fields() {
        // A manifest-global semantic index is not permitted alongside a v2
        // schema, and neither is a tile that omits its own index.
        let mut global_index = manifest_v2();
        global_index.semantic_index = Some(semantic_index());
        assert!(matches!(
            global_index.validate(),
            Err(RenderArtifactValidationError::UnexpectedField {
                field: "semantic_index",
                ..
            })
        ));

        let mut policy_in_v1 = manifest();
        policy_in_v1.render_policy = Some(RenderPolicy {
            id: "vex.render-policy.full-model/1".to_string(),
            hash: "b".repeat(64),
        });
        assert!(matches!(
            policy_in_v1.validate(),
            Err(RenderArtifactValidationError::UnexpectedField {
                field: "render_policy",
                ..
            })
        ));
    }

    #[test]
    fn v2_manifest_rejects_tampered_tile_index_resource() {
        // A tile-local index object with a non-SHA-256 digest is rejected the
        // same way a tampered tile artifact is.
        let mut tampered = manifest_v2();
        tampered.tiles[0]
            .semantic_index
            .as_mut()
            .unwrap()
            .artifact
            .sha256 = "not-a-valid-sha".to_string();
        assert!(matches!(
            tampered.validate(),
            Err(RenderArtifactValidationError::InvalidSha256 { .. })
        ));
    }

    #[test]
    fn v2_manifest_accepts_multi_lod_group_ownership() {
        // A storey group may own a coarse (lod > 0) tile plus its exact base
        // tile; both carry the same non-empty `group` key while keeping
        // distinct per-LOD tile ids.
        let mut grouped = manifest_v2();
        grouped.tiles[0].group = Some("storey-a".to_string());
        grouped.tiles[0].tile_id = "storey-a".to_string();
        let mut coarse = grouped.tiles[0].clone();
        coarse.tile_id = "storey-a/coarse".to_string();
        coarse.group = Some("storey-a".to_string());
        coarse.lod = 1;
        coarse.geometric_error = 4.0;
        grouped.tiles.insert(0, coarse);
        assert!(grouped.validate().is_ok());

        // The group key survives a serialization round trip and is only
        // emitted when actually set.
        let value = serde_json::to_value(&grouped).unwrap();
        assert_eq!(value["tiles"][0]["group"], "storey-a");
        assert_eq!(value["tiles"][0]["lod"], 1);
        let decoded: RenderArtifactManifest = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, grouped);
    }

    #[test]
    fn v2_manifest_rejects_empty_group_key() {
        let mut empty_group = manifest_v2();
        empty_group.tiles[0].group = Some(String::new());
        assert!(matches!(
            empty_group.validate(),
            Err(RenderArtifactValidationError::MissingField("tile.group"))
        ));
    }

    #[test]
    fn v1_manifest_rejects_tile_group_key() {
        // The stable-ownership `group` key is a v2-only concept; a v1 tile that
        // carries one is rejected the same way other v2-only tile fields are.
        let mut grouped_v1 = manifest();
        grouped_v1.tiles[0].group = Some("storey-a".to_string());
        assert!(matches!(
            grouped_v1.validate(),
            Err(RenderArtifactValidationError::UnexpectedField {
                field: "tile.group",
                ..
            })
        ));
    }

    // ---- Federation model ----

    fn federation_member(source: &str, commit: &str) -> FederationMember {
        FederationMember {
            member_id: format!("mem-{source}"),
            source_project_id: source.to_string(),
            commit_hash: commit.to_string(),
            display_name: "Architecture".to_string(),
            discipline: Some("architecture".to_string()),
            transform: FederationTransform::identity(),
            visible: true,
        }
    }

    fn federation() -> Federation {
        Federation {
            schema: schema::FEDERATION.to_string(),
            federation_id: "fed-001".to_string(),
            name: "Tower coordination".to_string(),
            created_at_unix: 1_700_000_000,
            updated_at_unix: 1_700_000_000,
            members: vec![
                federation_member("arch", &"a".repeat(64)),
                federation_member("struct", &"b".repeat(64)),
            ],
        }
    }

    #[test]
    fn identity_transform_is_a_valid_affine() {
        assert!(FederationTransform::identity().is_valid_affine());
        assert_eq!(
            FederationTransform::default(),
            FederationTransform::identity()
        );
    }

    #[test]
    fn transform_rejects_perspective_non_finite_and_singular() {
        // A non-[0,0,0,1] bottom row is a perspective transform: rejected.
        let mut perspective = FederationTransform::identity();
        perspective.rows[3] = [0.1, 0.0, 0.0, 1.0];
        assert!(!perspective.is_valid_affine());

        // Any non-finite entry is rejected.
        let mut non_finite = FederationTransform::identity();
        non_finite.rows[0][3] = f64::NAN;
        assert!(!non_finite.is_valid_affine());
        non_finite.rows[0][3] = f64::INFINITY;
        assert!(!non_finite.is_valid_affine());

        // A singular linear block (zero column) is not invertible: rejected.
        let mut singular = FederationTransform::identity();
        singular.rows[0][0] = 0.0;
        singular.rows[1][0] = 0.0;
        singular.rows[2][0] = 0.0;
        assert!(!singular.is_valid_affine());
    }

    #[test]
    fn transform_accepts_translation_rotation_and_scale() {
        // A translation + 90° rotation about Z + uniform scale is invertible.
        let transform = FederationTransform {
            rows: [
                [0.0, -2.0, 0.0, 10.0],
                [2.0, 0.0, 0.0, -5.0],
                [0.0, 0.0, 2.0, 3.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        assert!(transform.is_valid_affine());
    }

    #[test]
    fn valid_federation_passes_validation_and_summarizes() {
        let federation = federation();
        assert!(federation.validate().is_ok());
        let summary = federation.summary();
        assert_eq!(summary.member_count, 2);
        assert_eq!(summary.visible_member_count, 2);
        assert_eq!(summary.federation_id, "fed-001");
    }

    #[test]
    fn federation_round_trips_and_defaults_schema() {
        let federation = federation();
        let value = serde_json::to_value(&federation).unwrap();
        assert_eq!(value["schema"], schema::FEDERATION);
        let decoded: Federation = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, federation);

        // A record persisted before the schema field existed still loads,
        // defaulting to the current federation schema.
        let legacy = json!({
            "federation_id": "fed-legacy",
            "name": "Legacy",
            "created_at_unix": 1,
            "updated_at_unix": 2,
            "members": [{
                "member_id": "mem-1",
                "source_project_id": "arch",
                "commit_hash": "a".repeat(64),
                "display_name": "Arch",
                "visible": true
            }]
        });
        let decoded: Federation = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded.schema, schema::FEDERATION);
        // A member without an explicit transform defaults to identity.
        assert_eq!(
            decoded.members[0].transform,
            FederationTransform::identity()
        );
    }

    #[test]
    fn federation_rejects_incompatible_schema() {
        let mut federation = federation();
        federation.schema = "vex.federation/2".to_string();
        assert!(matches!(
            federation.validate(),
            Err(FederationValidationError::IncompatibleSchema { .. })
        ));
    }

    #[test]
    fn federation_requires_at_least_one_member_and_bounds_the_maximum() {
        let mut empty = federation();
        empty.members.clear();
        assert_eq!(empty.validate(), Err(FederationValidationError::NoMembers));

        let mut too_many = federation();
        too_many.members = (0..(Federation::MAX_MEMBERS + 1))
            .map(|index| {
                let mut member = federation_member(&format!("p{index}"), &"c".repeat(64));
                member.member_id = format!("mem-{index}");
                member
            })
            .collect();
        assert!(matches!(
            too_many.validate(),
            Err(FederationValidationError::TooManyMembers { .. })
        ));
    }

    #[test]
    fn federation_rejects_duplicate_member_id() {
        let mut federation = federation();
        federation.members[1].member_id = federation.members[0].member_id.clone();
        assert!(matches!(
            federation.validate(),
            Err(FederationValidationError::DuplicateMemberId(_))
        ));
    }

    #[test]
    fn federation_rejects_duplicate_source_commit_pair() {
        // Two members may share a source project only at distinct commits.
        let commit = "a".repeat(64);
        let mut federation = federation();
        federation.members[0].source_project_id = "arch".to_string();
        federation.members[0].commit_hash = commit.clone();
        federation.members[1].source_project_id = "arch".to_string();
        federation.members[1].commit_hash = commit;
        assert!(matches!(
            federation.validate(),
            Err(FederationValidationError::DuplicateSourceCommit { .. })
        ));
    }

    #[test]
    fn federation_rejects_partial_or_non_hex_commit() {
        let mut short = federation();
        short.members[0].commit_hash = "abc".to_string();
        assert!(matches!(
            short.validate(),
            Err(FederationValidationError::InvalidCommitHash { .. })
        ));

        let mut non_hex = federation();
        non_hex.members[0].commit_hash = "g".repeat(64);
        assert!(matches!(
            non_hex.validate(),
            Err(FederationValidationError::InvalidCommitHash { .. })
        ));
    }

    #[test]
    fn federation_rejects_unsafe_names_and_ids() {
        // Control characters in a name could smuggle log/terminal escapes.
        let mut bad_name = federation();
        bad_name.name = "line1\nline2".to_string();
        assert!(matches!(
            bad_name.validate(),
            Err(FederationValidationError::UnsafeText { field: "name" })
        ));

        let mut empty_name = federation();
        empty_name.name = "   ".to_string();
        assert!(matches!(
            empty_name.validate(),
            Err(FederationValidationError::MissingField("name"))
        ));

        let mut long_name = federation();
        long_name.name = "x".repeat(Federation::MAX_NAME_LEN + 1);
        assert!(matches!(
            long_name.validate(),
            Err(FederationValidationError::TextTooLong { field: "name", .. })
        ));

        // A traversal-style id is rejected before it can ever reach a filename.
        let mut traversal = federation();
        traversal.federation_id = "../../etc/passwd".to_string();
        assert!(matches!(
            traversal.validate(),
            Err(FederationValidationError::UnsafeId {
                field: "federation_id"
            })
        ));

        let mut bad_source = federation();
        bad_source.members[0].source_project_id = "arch/../secret".to_string();
        assert!(matches!(
            bad_source.validate(),
            Err(FederationValidationError::UnsafeId {
                field: "source_project_id"
            })
        ));
    }

    #[test]
    fn federation_rejects_invalid_member_transform() {
        let mut federation = federation();
        federation.members[0].transform.rows[3] = [0.0, 0.0, 0.0, 0.0];
        assert!(matches!(
            federation.validate(),
            Err(FederationValidationError::InvalidTransform { .. })
        ));
    }

    #[test]
    fn snapshot_member_flattens_render_status() {
        let member = FederationSnapshotMember {
            member_id: "mem-1".to_string(),
            source_project_id: "arch".to_string(),
            commit_hash: "a".repeat(64),
            display_name: "Arch".to_string(),
            discipline: None,
            transform: FederationTransform::identity(),
            visible: true,
            project_available: true,
            commit_available: true,
            render_status: FederationRenderStatus::Ready,
        };
        let value = serde_json::to_value(&member).unwrap();
        assert_eq!(value["render"], "ready");
        let decoded: FederationSnapshotMember = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, member);
    }
}
