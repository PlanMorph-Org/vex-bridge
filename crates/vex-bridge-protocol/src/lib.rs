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
    pub uptime_seconds: u64,
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
    /// Commits committed locally but not yet pushed to the remote. Drained
    /// by the daemon's background outbox; surfaced so the UI can show
    /// "N changes pending sync" instead of silently losing pushes.
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
}
