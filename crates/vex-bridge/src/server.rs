//! Local HTTP server that plugins call. Binds to 127.0.0.1 only.
//!
//! Auth model: every request must carry `X-Vex-Bridge-Token` matching the
//! contents of `<config_dir>/access-token`. The token is generated once on
//! daemon start with `mode 0600`, so:
//!   * a cooperating process running as the same user can read it;
//!   * a malicious webpage in the user's browser cannot read it cross-origin;
//!   * a different OS user cannot.

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tokio::sync::RwLock;
use tracing::{info, warn};

use vex_bridge_protocol as proto;

use crate::config::{Config, Paths};
use crate::dashboard;
use crate::errors::BridgeError;
use crate::pairing;
use crate::pipeline::{self, WatchPipeline};
use crate::state::{now_unix, PairingState, State as DaemonState};
use crate::vex_cli;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub state: Arc<RwLock<DaemonState>>,
    pub paths: Arc<Paths>,
    pub access_token: Arc<String>,
    pub started_at: Instant,
    pub watchers: Arc<RwLock<Vec<WatchPipeline>>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ChangeQuery {
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handle_dashboard))
        .route("/ui", get(handle_dashboard))
        .route("/v1/health", get(handle_health))
        .route("/v1/pair/status", get(handle_pair_status))
        .route("/v1/pair/start", post(handle_pair_start))
        .route("/v1/setup/status", get(handle_setup_status))
        .route("/v1/setup/inbox", post(handle_setup_inbox))
        .route("/v1/watch/status", get(handle_watch_status))
        .route("/v1/activity/recent", get(handle_recent_activity))
        .route("/v1/projects", get(handle_projects))
        .route(
            "/v1/projects/:project_id/history",
            get(handle_project_history),
        )
        .route(
            "/v1/projects/:project_id/changes",
            get(handle_project_changes),
        )
        .route("/v1/repo/push", post(handle_repo_push))
        .route("/v1/repo/register", post(handle_repo_register))
        .with_state(state)
}

pub async fn serve(state: AppState, port: u16) -> anyhow::Result<()> {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "vex-bridge listening");
    axum::serve(listener, router(state)).await?;
    Ok(())
}

#[allow(clippy::result_large_err)]
fn require_token(headers: &HeaderMap, expected: &str) -> Result<(), Response> {
    match headers
        .get("x-vex-bridge-token")
        .and_then(|value| value.to_str().ok())
    {
        Some(token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => Ok(()),
        _ => Err((
            StatusCode::UNAUTHORIZED,
            Json(proto::ApiError {
                error: "unauthorized".into(),
                message: "missing or invalid X-Vex-Bridge-Token header".into(),
            }),
        )
            .into_response()),
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

fn err_response(status: StatusCode, error: BridgeError) -> Response {
    warn!(error = %error, "request failed");
    (
        status,
        Json(proto::ApiError {
            error: format!("{error:?}")
                .split_whitespace()
                .next()
                .unwrap_or("error")
                .to_lowercase(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

async fn handle_dashboard(State(state): State<AppState>) -> Html<String> {
    Html(dashboard::render(&state.access_token))
}

async fn handle_health(State(state): State<AppState>) -> Json<proto::Health> {
    let cfg = state.config.read().await.clone();
    let daemon_state = state.state.read().await.clone();
    let vex_version = vex_cli::version(&cfg.vex_bin).await.ok().flatten();
    Json(proto::Health {
        version: env!("CARGO_PKG_VERSION").to_string(),
        paired: matches!(daemon_state.pairing, PairingState::Paired { .. }),
        vex_bin: cfg.vex_bin,
        vex_version,
        uptime_seconds: state.started_at.elapsed().as_secs(),
    })
}

async fn handle_pair_status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<proto::PairStatus>, Response> {
    require_token(&headers, &state.access_token)?;
    let daemon_state = state.state.read().await.clone();
    let body = match daemon_state.pairing {
        PairingState::Unpaired => proto::PairStatus::Unpaired,
        PairingState::Pending {
            code,
            pair_url,
            expires_at_unix,
            ..
        } => proto::PairStatus::Pending {
            code,
            pair_url,
            expires_at: rfc3339_from_unix(expires_at_unix),
        },
        PairingState::Paired {
            device_label,
            key_fingerprint,
            paired_at_unix,
            ..
        } => proto::PairStatus::Paired {
            device_label,
            key_fingerprint,
            paired_at: rfc3339_from_unix(paired_at_unix),
        },
    };
    Ok(Json(body))
}

async fn handle_pair_start(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<proto::PairStartRequest>,
) -> Result<Json<proto::PairStartResponse>, Response> {
    require_token(&headers, &state.access_token)?;
    let cfg = state.config.read().await.clone();
    let outcome = pairing::start(&cfg, &req.device_label)
        .await
        .map_err(|error| err_response(StatusCode::BAD_GATEWAY, error))?;

    {
        let mut daemon_state = state.state.write().await;
        daemon_state.pairing = PairingState::Pending {
            code: outcome.code.clone(),
            pair_url: outcome.pair_url.clone(),
            expires_at_unix: now_unix() + 600,
            device_label: req.device_label.clone(),
        };
        daemon_state
            .save(&state.paths)
            .map_err(|error| err_response(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    }

    Ok(Json(proto::PairStartResponse {
        code: outcome.code,
        pair_url: outcome.pair_url,
        expires_at: outcome.expires_at,
    }))
}

async fn handle_setup_status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<proto::SetupStatus>, Response> {
    require_token(&headers, &state.access_token)?;
    let cfg = state.config.read().await.clone();
    let daemon_state = state.state.read().await.clone();
    let active = active_project_ids(&state).await;
    let watch = watch_status_from(&cfg, &daemon_state, &active);
    Ok(Json(proto::SetupStatus {
        paired: matches!(daemon_state.pairing, PairingState::Paired { .. }),
        needs_inbox: cfg.watch.is_empty(),
        suggested_inbox_path: default_inbox_path("default", None)
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|_| "VexInbox/default".to_string()),
        config_path: state.paths.config_file.to_string_lossy().to_string(),
        state_path: state.paths.state_file.to_string_lossy().to_string(),
        watch,
    }))
}

async fn handle_setup_inbox(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<proto::SetupInboxRequest>,
) -> Result<Json<proto::SetupInboxResponse>, Response> {
    require_token(&headers, &state.access_token)?;
    let repo = register_watch(
        &state,
        proto::RepoRegisterRequest {
            project_id: req.project_id,
            local_path: req.local_path,
            include: req.include,
            ifc_project_guid: req.ifc_project_guid,
            project_name: req.project_name,
        },
    )
    .await?;
    let cfg = state.config.read().await.clone();
    let daemon_state = state.state.read().await.clone();
    let active = active_project_ids(&state).await;
    Ok(Json(proto::SetupInboxResponse {
        repo,
        watch: watch_status_from(&cfg, &daemon_state, &active),
    }))
}

async fn handle_watch_status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<proto::WatchStatus>, Response> {
    require_token(&headers, &state.access_token)?;
    Ok(Json(current_watch_status(&state).await))
}

async fn handle_projects(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Vec<proto::ProjectSummary>>, Response> {
    require_token(&headers, &state.access_token)?;
    Ok(Json(current_watch_status(&state).await.projects))
}

async fn handle_recent_activity(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<proto::RecentActivityResponse>, Response> {
    require_token(&headers, &state.access_token)?;
    let daemon_state = state.state.read().await;
    Ok(Json(proto::RecentActivityResponse {
        events: daemon_state.recent_activity(50),
    }))
}

async fn handle_project_history(
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<proto::ProjectHistoryResponse>, Response> {
    require_token(&headers, &state.access_token)?;
    let cfg = state.config.read().await.clone();
    let entry =
        find_watch_entry(&cfg, &project_id).ok_or_else(|| unknown_project_response(&project_id))?;
    let dir = PathBuf::from(&entry.path);
    let log = vex_cli::log_json(&cfg.vex_bin, &dir)
        .await
        .map_err(|error| err_response(StatusCode::BAD_GATEWAY, error))?;
    Ok(Json(proto::ProjectHistoryResponse {
        project_id,
        project_name: entry.project_name.clone(),
        commits: commits_from_log(log),
    }))
}

async fn handle_project_changes(
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
    Query(query): Query<ChangeQuery>,
    State(state): State<AppState>,
) -> Result<Json<proto::ProjectChangesResponse>, Response> {
    require_token(&headers, &state.access_token)?;
    let cfg = state.config.read().await.clone();
    let daemon_state = state.state.read().await.clone();
    let entry =
        find_watch_entry(&cfg, &project_id).ok_or_else(|| unknown_project_response(&project_id))?;
    let dir = PathBuf::from(&entry.path);
    let log = vex_cli::log_json(&cfg.vex_bin, &dir)
        .await
        .map_err(|error| err_response(StatusCode::BAD_GATEWAY, error))?;
    let commits = commits_from_log(log);
    let latest_commit = query
        .to
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .or_else(|| commits.first().map(|commit| commit.commit.clone()));
    let previous_commit = if query
        .to
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        query
            .from
            .filter(|value| !value.trim().is_empty())
            .or_else(|| parent_for_commit(&commits, latest_commit.as_deref()))
    } else {
        query
            .from
            .filter(|value| !value.trim().is_empty())
            .or_else(|| parent_for_commit(&commits, latest_commit.as_deref()))
            .or_else(|| commits.get(1).map(|commit| commit.commit.clone()))
    };
    let caught_at_unix = caught_at_for_commit(&daemon_state, &project_id, latest_commit.as_deref());
    let visual_diff = if let (Some(from), Some(to)) = (&previous_commit, &latest_commit) {
        vex_cli::compare_json(&cfg.vex_bin, &dir, from, to)
            .await
            .map_err(|error| err_response(StatusCode::BAD_GATEWAY, error))?
    } else if previous_commit.is_some() {
        vex_cli::changes_json(&cfg.vex_bin, &dir)
            .await
            .map_err(|error| err_response(StatusCode::BAD_GATEWAY, error))?
    } else {
        serde_json::json!({"status": "no-previous-version"})
    };
    Ok(Json(proto::ProjectChangesResponse {
        project_id,
        project_name: entry.project_name.clone(),
        caught_at_unix,
        latest_commit,
        previous_commit,
        visual_diff,
    }))
}

async fn handle_repo_push(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<proto::PushRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    require_token(&headers, &state.access_token)?;
    let cfg = state.config.read().await.clone();
    let branch = req.branch.unwrap_or_else(|| "main".to_string());

    match pipeline::run_manual_push(&cfg, &req.project_id, &branch).await {
        Ok(commit_hash) => Ok(Json(serde_json::json!({
            "commit_hash": commit_hash,
            "project_id": req.project_id,
            "branch": branch,
        }))),
        Err(error) => {
            let status = match &error {
                BridgeError::Config(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_GATEWAY,
            };
            Err(err_response(status, error))
        }
    }
}

async fn handle_repo_register(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<proto::RepoRegisterRequest>,
) -> Result<Json<proto::RepoRegisterResponse>, Response> {
    require_token(&headers, &state.access_token)?;
    register_watch(&state, req).await.map(Json)
}

async fn register_watch(
    state: &AppState,
    req: proto::RepoRegisterRequest,
) -> Result<proto::RepoRegisterResponse, Response> {
    if req.project_id.trim().is_empty() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            BridgeError::Config("project_id must not be empty".into()),
        ));
    }

    let local_path = match req.local_path {
        Some(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => default_inbox_path(&req.project_id, req.project_name.as_deref()).map_err(|error| {
            err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                BridgeError::Config(error),
            )
        })?,
    };

    if let Err(error) = std::fs::create_dir_all(&local_path) {
        return Err(err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::Io(error),
        ));
    }

    let include = req
        .include
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| vec!["*.ifc".to_string()]);

    let entry = crate::config::WatchEntry {
        project_id: req.project_id.clone(),
        path: local_path.to_string_lossy().to_string(),
        include: include.clone(),
        ifc_project_guid: req.ifc_project_guid.clone(),
        project_name: req.project_name.clone(),
    };

    let replaced = {
        let mut cfg = state.config.write().await;
        match cfg
            .watch
            .iter()
            .position(|watch| watch.project_id == req.project_id)
        {
            Some(index) => {
                cfg.watch[index] = entry.clone();
                true
            }
            None => {
                cfg.watch.push(entry.clone());
                false
            }
        }
    };

    let cfg_snapshot = {
        let cfg = state.config.read().await;
        if let Err(error) = cfg.save(&state.paths) {
            return Err(err_response(StatusCode::INTERNAL_SERVER_ERROR, error));
        }
        cfg.clone()
    };

    let watching = match pipeline::spawn_entry(
        &cfg_snapshot,
        tokio::runtime::Handle::current(),
        entry,
        state.state.clone(),
        state.paths.clone(),
    ) {
        Ok(pipeline) => {
            let mut watchers = state.watchers.write().await;
            watchers.retain(|watcher| watcher.project_id != req.project_id);
            watchers.push(pipeline);
            true
        }
        Err(error) => {
            warn!(project_id = %req.project_id, error = %error, "registered project but watch activation failed");
            false
        }
    };

    info!(project_id = %req.project_id, path = %local_path.display(), replaced, watching, "registered project");

    Ok(proto::RepoRegisterResponse {
        project_id: req.project_id,
        local_path: local_path.to_string_lossy().to_string(),
        include,
        ifc_project_guid: req.ifc_project_guid,
        project_name: req.project_name,
        replaced,
        watching,
    })
}

fn parent_for_commit(commits: &[proto::CommitSummary], commit: Option<&str>) -> Option<String> {
    let commit = commit?;
    commits
        .iter()
        .find(|item| item.commit == commit || item.commit.starts_with(commit))
        .and_then(|item| item.parents.first().cloned())
}

fn caught_at_for_commit(
    state: &DaemonState,
    project_id: &str,
    commit: Option<&str>,
) -> Option<i64> {
    let exact = commit.and_then(|commit| {
        state
            .recent_activity
            .iter()
            .rev()
            .find(|event| {
                event.project_id == project_id
                    && event
                        .commit_hash
                        .as_deref()
                        .map(|hash| hash == commit || hash.starts_with(commit))
                        .unwrap_or(false)
            })
            .map(|event| event.caught_at_unix)
    });
    exact.or_else(|| {
        state
            .seen_ifc_hashes
            .iter()
            .filter(|seen| seen.project_id == project_id)
            .map(|seen| seen.imported_at_unix)
            .max()
    })
}

async fn current_watch_status(state: &AppState) -> proto::WatchStatus {
    let cfg = state.config.read().await.clone();
    let daemon_state = state.state.read().await.clone();
    let active = active_project_ids(state).await;
    watch_status_from(&cfg, &daemon_state, &active)
}

async fn active_project_ids(state: &AppState) -> HashSet<String> {
    state
        .watchers
        .read()
        .await
        .iter()
        .map(|watcher| watcher.project_id.clone())
        .collect()
}

fn watch_status_from(
    cfg: &Config,
    state: &DaemonState,
    active: &HashSet<String>,
) -> proto::WatchStatus {
    let mut counts: HashMap<&str, (usize, Option<i64>)> = HashMap::new();
    for seen in &state.seen_ifc_hashes {
        let entry = counts.entry(&seen.project_id).or_insert((0, None));
        entry.0 += 1;
        entry.1 = Some(
            entry
                .1
                .unwrap_or(seen.imported_at_unix)
                .max(seen.imported_at_unix),
        );
    }

    let projects = cfg
        .watch
        .iter()
        .map(|watch| {
            let (seen_import_count, last_imported_at_unix) = counts
                .get(watch.project_id.as_str())
                .copied()
                .unwrap_or((0, None));
            proto::ProjectSummary {
                project_id: watch.project_id.clone(),
                project_name: watch.project_name.clone(),
                local_path: watch.path.clone(),
                path_exists: Path::new(&watch.path).is_dir(),
                active: active.contains(&watch.project_id),
                include: watch.include.clone(),
                ifc_project_guid: watch.ifc_project_guid.clone(),
                seen_import_count,
                last_imported_at_unix,
            }
        })
        .collect();

    proto::WatchStatus {
        active_watchers: active.len(),
        configured_projects: cfg.watch.len(),
        seen_ifc_hash_count: state.seen_ifc_hashes.len(),
        projects,
    }
}

fn find_watch_entry(cfg: &Config, project_id: &str) -> Option<crate::config::WatchEntry> {
    cfg.watch
        .iter()
        .find(|watch| watch.project_id == project_id)
        .cloned()
}

fn unknown_project_response(project_id: &str) -> Response {
    err_response(
        StatusCode::NOT_FOUND,
        BridgeError::Config(format!("unknown project_id `{project_id}`")),
    )
}

fn commits_from_log(value: serde_json::Value) -> Vec<proto::CommitSummary> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some(proto::CommitSummary {
                commit: item.get("commit")?.as_str()?.to_string(),
                author: item
                    .get("author")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                email: item
                    .get("email")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                timestamp: item
                    .get("timestamp")
                    .and_then(|value| value.as_i64())
                    .unwrap_or_default(),
                message: item
                    .get("message")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                parents: item
                    .get("parents")
                    .and_then(|value| value.as_array())
                    .map(|parents| {
                        parents
                            .iter()
                            .filter_map(|parent| parent.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn rfc3339_from_unix(ts: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let datetime = UNIX_EPOCH + Duration::from_secs(ts.max(0) as u64);
    let seconds = datetime
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, min, sec) = unix_to_civil(seconds as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn unix_to_civil(seconds: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = seconds.div_euclid(86_400);
    let time = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (
        year,
        month,
        day,
        (time / 3600) as u32,
        ((time % 3600) / 60) as u32,
        (time % 60) as u32,
    )
}

fn default_inbox_path(project_id: &str, project_name: Option<&str>) -> Result<PathBuf, String> {
    let home = directories::UserDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .ok_or_else(|| "could not resolve user home".to_string())?;
    let label = project_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(project_id);
    Ok(home.join("VexInbox").join(safe_path_segment(label)))
}

fn safe_path_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed.to_string()
    }
}
