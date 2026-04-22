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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRegisterResponse {
    pub project_id: String,
    pub local_path: String,
    pub include: Vec<String>,
    /// True if a previous entry for this project_id was replaced.
    pub replaced: bool,
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
