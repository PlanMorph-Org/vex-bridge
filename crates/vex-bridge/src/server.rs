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
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::{to_bytes, Body},
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use tokio::sync::RwLock;
use tracing::{info, warn};
use uuid::Uuid;

use vex_bridge_protocol as proto;

use crate::config::{Config, Paths};
use crate::dashboard;
use crate::device::default_device_label;
use crate::errors::BridgeError;
use crate::ifc::parse_preview_elements;
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
    /// Notified when the daemon should shut down gracefully (e.g. a desktop
    /// client detected a version mismatch and requested a clean restart).
    pub shutdown: Arc<tokio::sync::Notify>,
    /// Per-channel cache of the last GitHub update lookup, so the in-app
    /// update check doesn't hammer the GitHub API (or its rate limit) on every
    /// dashboard poll.
    pub update_cache: Arc<tokio::sync::Mutex<HashMap<String, (Instant, proto::UpdateInfo)>>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ChangeQuery {
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct IfcSnapshotQuery {
    #[serde(default)]
    commit: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct LimitQuery {
    #[serde(default)]
    limit: Option<usize>,
}

impl LimitQuery {
    const DEFAULT: usize = 50;
    const MAX: usize = 200;

    /// Resolve the requested page size, clamped to a sane range so a client
    /// cannot ask the daemon to serialize an unbounded activity log.
    fn resolved(&self) -> usize {
        self.limit.unwrap_or(Self::DEFAULT).clamp(1, Self::MAX)
    }
}

struct PathValidationError {
    status: StatusCode,
    error: BridgeError,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(handle_dashboard))
        .route("/ui", get(handle_dashboard))
        .route("/assets/viewer/*path", get(handle_viewer_asset))
        .route("/v1/health", get(handle_health))
        .route("/v1/pair/status", get(handle_pair_status))
        .route("/v1/pair/start", post(handle_pair_start))
        .route("/v1/pair/poll", post(handle_pair_poll))
        .route("/v1/pair/forget", post(handle_pair_forget))
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
        .route(
            "/v1/projects/:project_id/ifc/latest",
            get(handle_project_ifc_latest),
        )
        .route(
            "/v1/projects/:project_id/ifc/:commit",
            get(handle_project_ifc_commit),
        )
        .route(
            "/v1/projects/:project_id/inbox",
            post(handle_project_inbox_upload),
        )
        .route("/v1/projects/:project_id", delete(handle_delete_project))
        .route("/v1/repo/push", post(handle_repo_push))
        .route("/v1/repo/register", post(handle_repo_register))
        .route("/v1/daemon/shutdown", post(handle_daemon_shutdown))
        .route("/v1/update/check", get(handle_update_check))
        .with_state(state)
}

/// Bind the loopback listen socket. Split out from [`serve`] so the caller can
/// claim the port *before* doing any other startup work (spawning watchers,
/// etc.) and react explicitly to "address already in use" — the common case
/// where a stale daemon is still holding the port — instead of half-starting
/// and then dying on the raw OS error.
pub async fn bind(port: u16) -> std::io::Result<tokio::net::TcpListener> {
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    tokio::net::TcpListener::bind(addr).await
}

pub async fn serve(state: AppState, listener: tokio::net::TcpListener) -> anyhow::Result<()> {
    let addr = listener.local_addr()?;
    info!(%addr, "vex-bridge listening");
    let shutdown = state.shutdown.clone();
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async move {
            shutdown.notified().await;
            info!("graceful shutdown requested");
        })
        .await?;
    Ok(())
}

#[allow(clippy::result_large_err)]
fn require_token(headers: &HeaderMap, expected: &str) -> Result<(), Response> {
    match headers
        .get("x-vex-bridge-token")
        .and_then(|value| value.to_str().ok())
    {
        Some(token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => Ok(()),
        _ => {
            let correlation_id = Uuid::now_v7().to_string();
            Err((
                StatusCode::UNAUTHORIZED,
                Json(proto::ApiError {
                    error: "unauthorized".into(),
                    message: "missing or invalid X-Vex-Bridge-Token header".into(),
                    code: Some("unauthorized".into()),
                    hint: Some("Open Vex Atlas from the local desktop app and retry.".into()),
                    retryable: Some(false),
                    correlation_id: Some(correlation_id),
                }),
            )
                .into_response())
        }
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
    let correlation_id = Uuid::now_v7().to_string();
    let (code, hint, retryable) = classify_error(status, &error);
    warn!(
        error = %error,
        %correlation_id,
        code,
        retryable,
        "request failed"
    );
    (
        status,
        Json(proto::ApiError {
            error: format!("{error:?}")
                .split_whitespace()
                .next()
                .unwrap_or("error")
                .to_lowercase(),
            message: error.to_string(),
            code: Some(code.to_string()),
            hint: hint.map(str::to_string),
            retryable: Some(retryable),
            correlation_id: Some(correlation_id),
        }),
    )
        .into_response()
}

fn classify_error(
    status: StatusCode,
    error: &BridgeError,
) -> (&'static str, Option<&'static str>, bool) {
    if status == StatusCode::CONFLICT {
        return (
            "project_id_conflict",
            Some("Choose a different project id or retry with allow_replace=true."),
            false,
        );
    }
    match error {
        BridgeError::NotPaired => (
            "not_paired",
            Some("Pair this device before syncing projects."),
            false,
        ),
        BridgeError::UpstreamApi(_) => (
            "upstream_api_error",
            Some("Check your network connection and retry."),
            true,
        ),
        BridgeError::VexCli(_) => (
            "vex_cli_error",
            Some("Ensure the bundled vex engine is available and compatible."),
            true,
        ),
        BridgeError::Config(_) if status == StatusCode::BAD_REQUEST => (
            "invalid_request",
            Some("Review the input and try again."),
            false,
        ),
        BridgeError::Config(_) => (
            "config_error",
            Some("Review Vex Atlas settings and project mappings."),
            false,
        ),
        BridgeError::Keychain(_) => (
            "keychain_error",
            Some("Unlock your OS keychain and retry."),
            false,
        ),
        BridgeError::Io(_) => (
            "io_error",
            Some("Check filesystem permissions and disk space, then retry."),
            true,
        ),
        BridgeError::Serde(_) => (
            "serialization_error",
            Some("The daemon received malformed JSON data."),
            false,
        ),
        BridgeError::Reqwest(_) => (
            "network_error",
            Some("Check your network connection and retry."),
            true,
        ),
    }
}

fn viewer_asset(path: &str) -> Option<(&'static [u8], &'static str)> {
    match path {
        "three/three.module.js" => Some((
            include_bytes!("../assets/viewer/three/three.module.js"),
            "text/javascript; charset=utf-8",
        )),
        "three/examples/jsm/utils/BufferGeometryUtils.js" => Some((
            include_bytes!("../assets/viewer/three/examples/jsm/utils/BufferGeometryUtils.js"),
            "text/javascript; charset=utf-8",
        )),
        "three/examples/jsm/controls/OrbitControls.js" => Some((
            include_bytes!("../assets/viewer/three/examples/jsm/controls/OrbitControls.js"),
            "text/javascript; charset=utf-8",
        )),
        "three/LICENSE" => Some((
            include_bytes!("../assets/viewer/three/LICENSE"),
            "text/plain; charset=utf-8",
        )),
        "web-ifc/web-ifc-api.js" => Some((
            include_bytes!("../assets/viewer/web-ifc/web-ifc-api.js"),
            "text/javascript; charset=utf-8",
        )),
        "web-ifc/web-ifc.wasm" => Some((
            include_bytes!("../assets/viewer/web-ifc/web-ifc.wasm"),
            "application/wasm",
        )),
        "web-ifc/web-ifc-mt.wasm" => Some((
            include_bytes!("../assets/viewer/web-ifc/web-ifc-mt.wasm"),
            "application/wasm",
        )),
        "web-ifc/LICENSE.md" => Some((
            include_bytes!("../assets/viewer/web-ifc/LICENSE.md"),
            "text/markdown; charset=utf-8",
        )),
        "web-ifc-three/IFCLoader.js" => Some((
            include_bytes!("../assets/viewer/web-ifc-three/IFCLoader.js"),
            "text/javascript; charset=utf-8",
        )),
        "NOTICE.md" => Some((
            include_bytes!("../assets/viewer/NOTICE.md"),
            "text/markdown; charset=utf-8",
        )),
        _ => None,
    }
}

async fn handle_dashboard(State(state): State<AppState>) -> Html<String> {
    Html(dashboard::render(&state.access_token))
}

async fn handle_viewer_asset(AxumPath(path): AxumPath<String>) -> Result<Response, Response> {
    let Some((bytes, content_type)) = viewer_asset(&path) else {
        return Err(err_response(
            StatusCode::NOT_FOUND,
            BridgeError::Config(format!("unknown viewer asset `{path}`")),
        ));
    };
    Ok(([(header::CONTENT_TYPE, content_type)], bytes.to_vec()).into_response())
}

async fn handle_health(State(state): State<AppState>) -> Json<proto::Health> {
    let cfg = state.config.read().await.clone();
    let daemon_state = state.state.read().await.clone();
    let vex_version = vex_cli::version(&cfg.vex_bin).await.ok().flatten();
    let vex_schema_compatible = vex_version
        .as_deref()
        .and_then(|version| version_at_least(version, 0, 1, 3));
    Json(proto::Health {
        version: env!("CARGO_PKG_VERSION").to_string(),
        paired: matches!(daemon_state.pairing, PairingState::Paired { .. }),
        vex_bin: cfg.vex_bin,
        vex_version,
        expected_visual_diff_schema: Some(proto::schema::VISUAL_DIFF.to_string()),
        vex_schema_compatible,
        uptime_seconds: state.started_at.elapsed().as_secs(),
    })
}

/// Request a clean, graceful shutdown of the daemon. This exists so a desktop
/// or tray client that detects a version mismatch (a stale daemon left over
/// from a previous build) can retire it deterministically and relaunch a fresh
/// one, instead of forcibly killing the process and risking a half-flushed
/// object store. Token-gated and localhost-only like every other mutating
/// route, so a hostile web page cannot stop the daemon.
async fn handle_daemon_shutdown(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, Response> {
    require_token(&headers, &state.access_token)?;
    info!("daemon shutdown requested via API");
    // Notify after a short delay so this response is flushed to the caller
    // before the listener stops accepting connections.
    let shutdown = state.shutdown.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        shutdown.notify_waiters();
    });
    Ok(Json(serde_json::json!({ "stopping": true })))
}

/// Default GitHub repo whose releases drive the in-app update check. Overridable
/// via `VEX_BRIDGE_UPDATE_REPO` for testing or forks.
const UPDATE_REPO_DEFAULT: &str = "PlanMorph-Org/vex-bridge";
/// How long a successful (or failed) GitHub lookup is cached per channel.
const UPDATE_CACHE_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, serde::Deserialize)]
struct UpdateQuery {
    #[serde(default)]
    channel: Option<String>,
    /// Bypass the cache and force a fresh GitHub lookup.
    #[serde(default)]
    refresh: bool,
}

/// `GET /v1/update/check?channel=stable|canary` — report whether a newer bridge
/// build is available. Results are cached per channel so the dashboard can poll
/// freely. A lookup failure is reported in the payload's `error` field rather
/// than failing the request, so transient GitHub hiccups never break the UI.
async fn handle_update_check(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<UpdateQuery>,
) -> Result<Json<proto::UpdateInfo>, Response> {
    require_token(&headers, &state.access_token)?;
    let channel = match query.channel.as_deref() {
        Some("canary") => "canary",
        _ => "stable",
    }
    .to_string();

    if !query.refresh {
        let cache = state.update_cache.lock().await;
        if let Some((fetched, info)) = cache.get(&channel) {
            if fetched.elapsed() < UPDATE_CACHE_TTL {
                return Ok(Json(info.clone()));
            }
        }
    }

    let info = compute_update_info(&channel).await;
    state
        .update_cache
        .lock()
        .await
        .insert(channel, (Instant::now(), info.clone()));
    Ok(Json(info))
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

async fn compute_update_info(channel: &str) -> proto::UpdateInfo {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let mut info = proto::UpdateInfo {
        current_version: current.clone(),
        latest_version: None,
        update_available: false,
        channel: channel.to_string(),
        release_url: None,
        release_notes: None,
        published_at: None,
        checked_at: rfc3339_from_unix(now_unix()),
        error: None,
    };
    match fetch_latest_release(channel).await {
        Ok(Some(release)) => {
            let latest = release.tag_name.trim_start_matches('v').to_string();
            info.update_available = version_greater(&latest, &current).unwrap_or(false);
            info.latest_version = Some(latest);
            info.release_url = Some(release.html_url);
            info.release_notes = release.body;
            info.published_at = release.published_at;
        }
        Ok(None) => info.error = Some("no published releases found".into()),
        Err(message) => info.error = Some(message),
    }
    info
}

/// Fetch the newest release matching `channel` from the GitHub Releases API.
/// `stable` skips prereleases; `canary` includes them. Drafts are always
/// skipped. Returns `Ok(None)` when no matching release exists.
async fn fetch_latest_release(channel: &str) -> Result<Option<GithubRelease>, String> {
    let repo = std::env::var("VEX_BRIDGE_UPDATE_REPO")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| UPDATE_REPO_DEFAULT.to_string());
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=20");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| error.to_string())?;
    let mut request = client
        .get(&url)
        .header(
            reqwest::header::USER_AGENT,
            concat!("vex-bridge/", env!("CARGO_PKG_VERSION")),
        )
        .header(reqwest::header::ACCEPT, "application/vnd.github+json");
    // A token lifts the unauthenticated rate limit; entirely optional.
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.trim().is_empty() {
            request = request.bearer_auth(token.trim().to_string());
        }
    }
    let response = request.send().await.map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("GitHub API returned HTTP {}", response.status()));
    }
    let releases: Vec<GithubRelease> = response.json().await.map_err(|error| error.to_string())?;
    let want_prerelease = channel == "canary";
    Ok(releases
        .into_iter()
        .filter(|release| !release.draft)
        .find(|release| want_prerelease || !release.prerelease))
}

/// True when `candidate` is a strictly newer semantic version than `baseline`.
/// Prerelease/build metadata is ignored for the comparison.
fn version_greater(candidate: &str, baseline: &str) -> Option<bool> {
    Some(semver_tuple(candidate)? > semver_tuple(baseline)?)
}

fn semver_tuple(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parse_version_part(parts.next()?)?;
    let minor = parse_version_part(parts.next().unwrap_or("0"))?;
    let patch = parse_version_part(parts.next().unwrap_or("0"))?;
    Some((major, minor, patch))
}

async fn handle_pair_status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<proto::PairStatus>, Response> {
    require_token(&headers, &state.access_token)?;
    let daemon_state = state.state.read().await.clone();
    Ok(Json(pair_status_from_state(&daemon_state)))
}

fn pair_status_from_state(state: &DaemonState) -> proto::PairStatus {
    match &state.pairing {
        PairingState::Unpaired => proto::PairStatus::Unpaired,
        PairingState::Pending {
            code,
            pair_url,
            expires_at_unix,
            ..
        } => proto::PairStatus::Pending {
            code: code.clone(),
            pair_url: pair_url.clone(),
            expires_at: rfc3339_from_unix(*expires_at_unix),
        },
        PairingState::Paired {
            device_label,
            key_fingerprint,
            paired_at_unix,
            account_id,
            account_email,
            account_name,
            ..
        } => proto::PairStatus::Paired {
            device_label: device_label.clone(),
            key_fingerprint: key_fingerprint.clone(),
            paired_at: rfc3339_from_unix(*paired_at_unix),
            account_id: account_id.clone(),
            account_email: account_email.clone(),
            account_name: account_name.clone(),
        },
    }
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

    if req.open_browser {
        if let Err(error) = open::that(&outcome.pair_url) {
            warn!(error = %error, url = %outcome.pair_url, "could not open pairing URL");
        }
    }

    {
        let mut daemon_state = state.state.write().await;
        daemon_state.pairing = PairingState::Pending {
            code: outcome.code.clone(),
            pair_url: outcome.pair_url.clone(),
            expires_at_unix: now_unix() + 600,
            device_label: req.device_label.clone(),
            key_fingerprint: outcome.key_fingerprint.clone(),
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

async fn handle_pair_poll(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<proto::PairStatus>, Response> {
    require_token(&headers, &state.access_token)?;

    let pending = {
        let daemon_state = state.state.read().await.clone();
        match daemon_state.pairing {
            PairingState::Pending {
                code,
                expires_at_unix,
                device_label,
                key_fingerprint,
                ..
            } => Some((code, expires_at_unix, device_label, key_fingerprint)),
            _ => return Ok(Json(pair_status_from_state(&daemon_state))),
        }
    };

    let Some((code, expires_at_unix, device_label, key_fingerprint)) = pending else {
        let daemon_state = state.state.read().await.clone();
        return Ok(Json(pair_status_from_state(&daemon_state)));
    };

    if now_unix() >= expires_at_unix {
        let mut daemon_state = state.state.write().await;
        daemon_state.pairing = PairingState::Unpaired;
        daemon_state
            .save(&state.paths)
            .map_err(|error| err_response(StatusCode::INTERNAL_SERVER_ERROR, error))?;
        return Ok(Json(proto::PairStatus::Unpaired));
    }

    let cfg = state.config.read().await.clone();
    if let Some(approval) = pairing::poll(&cfg, &code)
        .await
        .map_err(|error| err_response(StatusCode::BAD_GATEWAY, error))?
    {
        let mut daemon_state = state.state.write().await;
        daemon_state.pairing = PairingState::Paired {
            device_label,
            key_fingerprint,
            key_id: approval.key_id,
            paired_at_unix: now_unix(),
            account_id: approval.account_id,
            account_email: approval.account_email,
            account_name: approval.account_name,
        };
        daemon_state
            .save(&state.paths)
            .map_err(|error| err_response(StatusCode::INTERNAL_SERVER_ERROR, error))?;
        return Ok(Json(pair_status_from_state(&daemon_state)));
    }

    let daemon_state = state.state.read().await.clone();
    Ok(Json(pair_status_from_state(&daemon_state)))
}

async fn handle_pair_forget(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<proto::PairStatus>, Response> {
    require_token(&headers, &state.access_token)?;
    {
        let mut daemon_state = state.state.write().await;
        daemon_state.pairing = PairingState::Unpaired;
        daemon_state
            .save(&state.paths)
            .map_err(|error| err_response(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    }
    if let Err(error) = crate::keychain::forget() {
        warn!(error = %error, "could not remove device key from keychain on sign-out");
    }
    Ok(Json(proto::PairStatus::Unpaired))
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
    let inbox_root = default_inbox_root().map_err(|error| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::Config(error),
        )
    })?;
    Ok(Json(proto::SetupStatus {
        paired: matches!(daemon_state.pairing, PairingState::Paired { .. }),
        pair_status: pair_status_from_state(&daemon_state),
        default_device_label: default_device_label(),
        inbox_root_path: inbox_root.to_string_lossy().to_string(),
        needs_inbox: cfg.watch.is_empty(),
        suggested_inbox_path: default_inbox_path("default", None, None)
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
    let (project_id, project_id_auto_generated) = setup_project_id(&req);
    let mut repo = register_watch(
        &state,
        proto::RepoRegisterRequest {
            project_id,
            local_path: setup_local_path(&req),
            include: req.include,
            ifc_project_guid: req.ifc_project_guid,
            project_name: req.project_name,
            allow_replace: true,
        },
    )
    .await?;
    repo.project_id_auto_generated = Some(project_id_auto_generated);
    let cfg = state.config.read().await.clone();
    let daemon_state = state.state.read().await.clone();
    let active = active_project_ids(&state).await;
    Ok(Json(proto::SetupInboxResponse {
        repo,
        watch: watch_status_from(&cfg, &daemon_state, &active),
    }))
}

/// Largest IFC upload we accept in a single request (512 MiB). IFC exports are
/// text STEP files; even large building models sit comfortably under this, and
/// the cap stops a misbehaving client from exhausting memory.
const MAX_INBOX_UPLOAD_BYTES: usize = 512 * 1024 * 1024;

/// Accept an IFC file straight from the desktop app (or any client) and drop it
/// into the project's watched inbox folder. The existing file watcher then
/// picks it up and runs the normal import + commit pipeline — i.e. the workflow
/// is identical to the user manually copying a file into the inbox, only now it
/// can be done from inside Vex Atlas.
///
/// The raw request body is the file's bytes; the desired name is passed via the
/// `X-Vex-Filename` header (falling back to a timestamped default). We write to
/// a hidden temp file first and atomically rename it into place so the watcher
/// never observes a half-written `.ifc`.
async fn handle_project_inbox_upload(
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
    State(state): State<AppState>,
    body: Body,
) -> Result<Json<serde_json::Value>, Response> {
    require_token(&headers, &state.access_token)?;

    // Resolve the project's inbox folder from config.
    let inbox_dir = {
        let cfg = state.config.read().await;
        match cfg.watch.iter().find(|w| w.project_id == project_id) {
            Some(entry) => PathBuf::from(&entry.path),
            None => return Err(unknown_project_response(&project_id)),
        }
    };

    let requested_name = headers
        .get("x-vex-filename")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let file_name = sanitize_ifc_filename(requested_name);

    let bytes = to_bytes(body, MAX_INBOX_UPLOAD_BYTES)
        .await
        .map_err(|error| {
            err_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                BridgeError::Config(format!(
                    "upload body exceeded {MAX_INBOX_UPLOAD_BYTES} bytes or could not be read: {error}"
                )),
            )
        })?;

    if bytes.is_empty() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            BridgeError::Config("upload body was empty".into()),
        ));
    }

    if !looks_like_ifc(&bytes) {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            BridgeError::Config(
                "uploaded file does not look like an IFC (STEP) file; expected an ISO-10303-21 header".into(),
            ),
        ));
    }

    if let Err(error) = std::fs::create_dir_all(&inbox_dir) {
        return Err(err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::Io(error),
        ));
    }

    // Write to a hidden temp file, then atomically rename into place. The temp
    // name does not match `*.ifc`, so the watcher ignores it until the rename.
    let temp_path = inbox_dir.join(format!(".vex-upload-{}.tmp", now_unix()));
    let final_path = inbox_dir.join(&file_name);

    if let Err(error) = std::fs::write(&temp_path, &bytes) {
        return Err(err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::Io(error),
        ));
    }
    if let Err(error) = std::fs::rename(&temp_path, &final_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::Io(error),
        ));
    }

    info!(
        project_id = %project_id,
        path = %final_path.display(),
        bytes = bytes.len(),
        "received IFC upload into inbox"
    );

    Ok(Json(serde_json::json!({
        "project_id": project_id,
        "file_name": file_name,
        "stored_path": final_path.to_string_lossy(),
        "bytes": bytes.len(),
    })))
}

/// Quick sniff for an IFC/STEP payload: the spec requires the file to begin with
/// the `ISO-10303-21;` header (possibly after a BOM or leading whitespace).
fn looks_like_ifc(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(64)];
    let text = String::from_utf8_lossy(head);
    text.trim_start_matches('\u{feff}')
        .trim_start()
        .to_ascii_uppercase()
        .starts_with("ISO-10303-21")
}

/// Turn a client-supplied filename into a safe `*.ifc` basename. Strips any path
/// components, keeps a conservative character set, and guarantees a non-empty
/// name ending in `.ifc`.
fn sanitize_ifc_filename(requested: &str) -> String {
    // Keep only the final path component so a client cannot escape the folder.
    let base = requested.rsplit(['/', '\\']).next().unwrap_or("").trim();

    let mut cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    cleaned = cleaned.trim().trim_matches('.').to_string();

    // Drop a trailing `.ifc` (any case) so we can re-append it canonically.
    let stem = cleaned
        .strip_suffix(".ifc")
        .or_else(|| cleaned.strip_suffix(".IFC"))
        .unwrap_or(&cleaned)
        .trim()
        .trim_matches('.')
        .to_string();

    let stem = if stem.is_empty() {
        format!("upload-{}", now_unix())
    } else {
        // Guard against pathologically long names on disk.
        stem.chars().take(120).collect()
    };

    format!("{stem}.ifc")
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
    Query(query): Query<LimitQuery>,
    State(state): State<AppState>,
) -> Result<Json<proto::RecentActivityResponse>, Response> {
    require_token(&headers, &state.access_token)?;
    let daemon_state = state.state.read().await;
    Ok(Json(proto::RecentActivityResponse {
        events: daemon_state.recent_activity(query.resolved()),
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
    if !is_local_vex_repo(&dir) {
        return Ok(Json(proto::ProjectHistoryResponse {
            project_id,
            project_name: entry.project_name.clone(),
            commits: Vec::new(),
        }));
    }
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
    let commits = if is_local_vex_repo(&dir) {
        let log = vex_cli::log_json(&cfg.vex_bin, &dir)
            .await
            .map_err(|error| err_response(StatusCode::BAD_GATEWAY, error))?;
        commits_from_log(log)
    } else {
        Vec::new()
    };
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
    let mut visual_diff = if let (Some(from), Some(to)) = (&previous_commit, &latest_commit) {
        vex_cli::compare_json(&cfg.vex_bin, &dir, from, to)
            .await
            .map_err(|error| err_response(StatusCode::BAD_GATEWAY, error))?
    } else if previous_commit.is_some() {
        vex_cli::changes_json(&cfg.vex_bin, &dir)
            .await
            .map_err(|error| err_response(StatusCode::BAD_GATEWAY, error))?
    } else {
        baseline_visual_json(&cfg.vex_bin, &dir, latest_commit.as_deref())
            .await
            .unwrap_or_else(|error| {
            serde_json::json!({
                "status": "baseline-unavailable",
                "summary": "Baseline model",
                "detail": error.to_string(),
                "counts": {"added": 0, "removed": 0, "modified": 0, "moved": 0, "renamed": 0, "unchanged": 0},
                "elements": []
            })
        })
    };
    attach_model_elements(
        &mut visual_diff,
        &cfg.vex_bin,
        &dir,
        latest_commit.as_deref(),
    )
    .await;
    Ok(Json(proto::ProjectChangesResponse {
        project_id,
        project_name: entry.project_name.clone(),
        caught_at_unix,
        latest_commit,
        previous_commit,
        visual_diff,
    }))
}

async fn handle_project_ifc_latest(
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
    Query(query): Query<IfcSnapshotQuery>,
    State(state): State<AppState>,
) -> Result<Response, Response> {
    require_token(&headers, &state.access_token)?;
    let path = resolve_ifc_snapshot_path(&state, &project_id, query.commit.as_deref()).await?;
    serve_ifc_file(path).await
}

async fn handle_project_ifc_commit(
    headers: HeaderMap,
    AxumPath((project_id, commit)): AxumPath<(String, String)>,
    State(state): State<AppState>,
) -> Result<Response, Response> {
    require_token(&headers, &state.access_token)?;
    let path = resolve_ifc_snapshot_path(&state, &project_id, Some(&commit)).await?;
    serve_ifc_file(path).await
}

async fn handle_delete_project(
    headers: HeaderMap,
    AxumPath(project_id): AxumPath<String>,
    State(state): State<AppState>,
    Json(req): Json<proto::DeleteProjectRequest>,
) -> Result<Json<proto::DeleteProjectResponse>, Response> {
    require_token(&headers, &state.access_token)?;
    let policy = req.deletion_policy.trim();
    if !matches!(policy, "keep_folder" | "archive_folder" | "delete_folder") {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            BridgeError::Config(format!(
                "invalid deletion_policy `{policy}`; expected keep_folder, archive_folder, or delete_folder"
            )),
        ));
    }

    let entry = {
        let mut cfg = state.config.write().await;
        let Some(entry) = cfg.remove_watch(&project_id) else {
            return Err(unknown_project_response(&project_id));
        };
        cfg.save(&state.paths)
            .map_err(|error| err_response(StatusCode::INTERNAL_SERVER_ERROR, error))?;
        entry
    };

    let watcher_stopped = {
        let mut watchers = state.watchers.write().await;
        let before = watchers.len();
        watchers.retain(|watcher| watcher.project_id != project_id);
        watchers.len() != before
    };

    let local_folder = entry.path.clone();
    let (folder_action, resulting_folder, policy_error) =
        apply_deletion_policy(Path::new(&entry.path), policy).await;

    info!(project_id = %project_id, path = %local_folder, policy, watcher_stopped, "deleted project watch");

    Ok(Json(proto::DeleteProjectResponse {
        project_id,
        local_folder,
        deletion_policy: policy.to_string(),
        removed_from_config: true,
        watcher_stopped,
        folder_action,
        resulting_folder,
        policy_error,
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
    let proto::RepoRegisterRequest {
        project_id,
        local_path,
        include,
        ifc_project_guid,
        project_name,
        allow_replace,
    } = req;

    let project_id = project_id.trim().to_string();
    if let Err(message) = validate_project_id(&project_id) {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            BridgeError::Config(message),
        ));
    }

    let local_path = match local_path {
        Some(path) if !path.trim().is_empty() => resolve_inbox_path(&path)
            .map_err(|error| err_response(StatusCode::BAD_REQUEST, BridgeError::Config(error)))?,
        _ => default_inbox_path(&project_id, project_name.as_deref(), None).map_err(|error| {
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

    let include = include
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| vec!["*.ifc".to_string()]);

    let entry = crate::config::WatchEntry {
        project_id: project_id.clone(),
        path: local_path.to_string_lossy().to_string(),
        include: include.clone(),
        ifc_project_guid: ifc_project_guid.clone(),
        project_name: project_name.clone(),
    };

    enum RegisterUpdate {
        Inserted,
        Updated,
        Conflict { existing_path: String },
    }

    let update = {
        let mut cfg = state.config.write().await;
        match cfg
            .watch
            .iter()
            .position(|watch| watch.project_id == project_id)
        {
            Some(index) => {
                let existing = &cfg.watch[index];
                let path_changed = !same_local_path(&existing.path, &entry.path);
                if path_changed && !allow_replace {
                    RegisterUpdate::Conflict {
                        existing_path: existing.path.clone(),
                    }
                } else {
                    cfg.watch[index] = entry.clone();
                    RegisterUpdate::Updated
                }
            }
            None => {
                cfg.watch.push(entry.clone());
                RegisterUpdate::Inserted
            }
        }
    };

    if let RegisterUpdate::Conflict { existing_path } = update {
        return Err(err_response(
            StatusCode::CONFLICT,
            BridgeError::Config(format!(
                "project_id `{project_id}` already maps to `{existing_path}`; pass allow_replace=true to remap"
            )),
        ));
    }

    let replaced = matches!(update, RegisterUpdate::Updated);

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
            watchers.retain(|watcher| watcher.project_id != project_id);
            watchers.push(pipeline);
            true
        }
        Err(error) => {
            warn!(project_id = %project_id, error = %error, "registered project but watch activation failed");
            false
        }
    };

    info!(project_id = %project_id, path = %local_path.display(), replaced, watching, "registered project");

    Ok(proto::RepoRegisterResponse {
        project_id,
        local_path: local_path.to_string_lossy().to_string(),
        include,
        ifc_project_guid,
        project_name,
        replaced,
        watching,
        project_id_auto_generated: None,
    })
}

fn parent_for_commit(commits: &[proto::CommitSummary], commit: Option<&str>) -> Option<String> {
    let commit = commit?;
    commits
        .iter()
        .find(|item| item.commit == commit || item.commit.starts_with(commit))
        .and_then(|item| item.parents.first().cloned())
}

async fn resolve_ifc_snapshot_path(
    state: &AppState,
    project_id: &str,
    commit: Option<&str>,
) -> Result<PathBuf, Response> {
    let cfg = state.config.read().await.clone();
    let entry =
        find_watch_entry(&cfg, project_id).ok_or_else(|| unknown_project_response(project_id))?;
    let dir = PathBuf::from(&entry.path);

    if let Some(commit) = commit.filter(|value| !value.trim().is_empty()) {
        {
            let daemon_state = state.state.read().await;
            if let Some(snapshot) = daemon_state.ifc_snapshot(project_id, commit) {
                let path = PathBuf::from(snapshot.path);
                validate_project_file_path(&dir, &path)
                    .map_err(|error| err_response(error.status, error.error))?;
                if path.is_file() {
                    return Ok(path);
                }
            }
        }
        // No recorded snapshot file is on disk for this commit. If this is a
        // local Vex repo, reconstruct the exact committed model from the object
        // store via `vex checkout` so historical commits remain viewable even
        // after their original IFC file was moved or overwritten.
        if is_local_vex_repo(&dir) {
            if let Some(path) = materialize_commit_ifc(&cfg.vex_bin, &dir, commit).await {
                return Ok(path);
            }
        }
        return Err(err_response(
            StatusCode::NOT_FOUND,
            BridgeError::Config(format!(
                "no IFC snapshot found for project `{project_id}` at commit `{commit}`"
            )),
        ));
    }

    // No explicit commit requested: prefer the newest recorded snapshot (the
    // latest commit's full model) so the caller always receives the complete
    // current model, even when no changes have been made since the last commit.
    // Fall back to a filesystem scan only when no snapshot has been recorded.
    {
        let daemon_state = state.state.read().await;
        if let Some(snapshot) = daemon_state.latest_ifc_snapshot_for_project(project_id) {
            let path = PathBuf::from(snapshot.path);
            if path.is_file() {
                validate_project_file_path(&dir, &path)
                    .map_err(|error| err_response(error.status, error.error))?;
                return Ok(path);
            }
        }
    }

    if let Some(path) = latest_ifc_snapshot(&dir)
        .map_err(|error| err_response(StatusCode::INTERNAL_SERVER_ERROR, error))?
    {
        validate_project_file_path(&dir, &path)
            .map_err(|error| err_response(error.status, error.error))?;
        return Ok(path);
    }

    // Nothing on disk: reconstruct the latest committed model from the store.
    if is_local_vex_repo(&dir) {
        if let Some(path) = materialize_commit_ifc(&cfg.vex_bin, &dir, "HEAD").await {
            return Ok(path);
        }
    }

    Err(err_response(
        StatusCode::NOT_FOUND,
        BridgeError::Config(format!("no IFC snapshot found for project `{project_id}`")),
    ))
}

/// Reconstruct the IFC model committed at `reference` into a cache file under
/// the project's `.vex/cache/ifc/` directory via `vex checkout`, returning the
/// cached path. Full 64-char commit hashes are immutable and reused across
/// requests; shorter refs (branches, tags, abbreviated hashes) are always
/// re-materialized. Returns `None` on any failure so callers fall through to a
/// 404 rather than surfacing engine errors.
async fn materialize_commit_ifc(bin: &str, dir: &Path, reference: &str) -> Option<PathBuf> {
    let safe: String = reference
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(64)
        .collect();
    if safe.is_empty() {
        return None;
    }
    let cache_dir = dir.join(".vex").join("cache").join("ifc");
    if let Err(error) = tokio::fs::create_dir_all(&cache_dir).await {
        tracing::debug!(%error, "failed to create ifc cache dir");
        return None;
    }
    let out = cache_dir.join(format!("{safe}.ifc"));
    let immutable = safe.len() == 64;
    if immutable && out.is_file() {
        return Some(out);
    }
    match vex_cli::checkout(bin, dir, reference, &out).await {
        Ok(bytes) if bytes > 0 => Some(out),
        Ok(_) => None,
        Err(error) => {
            tracing::debug!(%error, "vex checkout failed");
            None
        }
    }
}

fn validate_project_file_path(project_dir: &Path, file: &Path) -> Result<(), PathValidationError> {
    let project_dir = project_dir
        .canonicalize()
        .map_err(|error| PathValidationError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: BridgeError::Io(error),
        })?;
    let file = file.canonicalize().map_err(|error| {
        let status = if error.kind() == std::io::ErrorKind::NotFound {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        PathValidationError {
            status,
            error: BridgeError::Io(error),
        }
    })?;
    if !file.starts_with(&project_dir) {
        return Err(PathValidationError {
            status: StatusCode::FORBIDDEN,
            error: BridgeError::Config("IFC snapshot path is outside the project folder".into()),
        });
    }
    let is_ifc = file
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("ifc"))
        .unwrap_or(false);
    if !is_ifc {
        return Err(PathValidationError {
            status: StatusCode::FORBIDDEN,
            error: BridgeError::Config("snapshot path is not an IFC file".into()),
        });
    }
    Ok(())
}

async fn serve_ifc_file(path: PathBuf) -> Result<Response, Response> {
    let bytes = tokio::fs::read(&path).await.map_err(|error| {
        let status = if error.kind() == std::io::ErrorKind::NotFound {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        err_response(status, BridgeError::Io(error))
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("model.ifc");
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (
                header::CONTENT_DISPOSITION,
                &format!("inline; filename=\"{}\"", safe_header_value(file_name)),
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn apply_deletion_policy(
    path: &Path,
    policy: &str,
) -> (String, Option<String>, Option<String>) {
    match policy {
        "keep_folder" => (
            "kept".to_string(),
            Some(path.to_string_lossy().to_string()),
            None,
        ),
        "archive_folder" => match archive_project_folder(path) {
            Ok(archived) => (
                "archived".to_string(),
                Some(archived.to_string_lossy().to_string()),
                None,
            ),
            Err(error) => (
                "archive_failed".to_string(),
                Some(path.to_string_lossy().to_string()),
                Some(error.to_string()),
            ),
        },
        "delete_folder" => match validate_deletable_inbox_path(path)
            .and_then(|_| std::fs::remove_dir_all(path).map_err(BridgeError::Io))
        {
            Ok(()) => ("deleted".to_string(), None, None),
            Err(error) => (
                "delete_failed".to_string(),
                Some(path.to_string_lossy().to_string()),
                Some(error.to_string()),
            ),
        },
        _ => (
            "kept".to_string(),
            Some(path.to_string_lossy().to_string()),
            None,
        ),
    }
}

fn archive_project_folder(path: &Path) -> Result<PathBuf, BridgeError> {
    validate_deletable_inbox_path(path)?;
    if !path.exists() {
        return Ok(path.to_path_buf());
    }
    let parent = path
        .parent()
        .ok_or_else(|| BridgeError::Config("project folder has no parent".into()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let timestamp = rfc3339_from_unix(now_unix()).replace([':', 'T', 'Z'], "-");
    let mut target = parent.join(format!("{name}.archived-{timestamp}"));
    let mut suffix = 1u32;
    while target.exists() {
        target = parent.join(format!("{name}.archived-{timestamp}-{suffix}"));
        suffix += 1;
    }
    std::fs::rename(path, &target)?;
    Ok(target)
}

fn validate_deletable_inbox_path(path: &Path) -> Result<(), BridgeError> {
    let root = default_inbox_root().map_err(BridgeError::Config)?;
    let root = root.canonicalize()?;
    let path = path.canonicalize()?;
    if path == root || !path.starts_with(&root) {
        return Err(BridgeError::Config(format!(
            "refusing to modify folder outside {}",
            root.display()
        )));
    }
    Ok(())
}

fn safe_header_value(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
        .collect()
}

fn setup_project_id(req: &proto::SetupInboxRequest) -> (String, bool) {
    if let Some(project_id) = req
        .project_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return (project_id.to_string(), false);
    }
    (format!("vex-{}", Uuid::now_v7().simple()), true)
}

fn validate_project_id(project_id: &str) -> Result<(), String> {
    if project_id.is_empty() {
        return Err("project_id must not be empty".into());
    }
    if project_id.len() > 120 {
        return Err("project_id is too long (max 120 characters)".into());
    }
    if !project_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return Err("project_id may only contain ASCII letters, numbers, '-' or '_'".into());
    }
    Ok(())
}

fn same_local_path(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let a_path = Path::new(a);
    let b_path = Path::new(b);
    match (a_path.canonicalize(), b_path.canonicalize()) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

fn version_at_least(version: &str, major: u64, minor: u64, patch: u64) -> Option<bool> {
    let mut parts = version.trim_start_matches('v').split('.');
    let parsed_major = parse_version_part(parts.next()?)?;
    let parsed_minor = parse_version_part(parts.next()?)?;
    let parsed_patch = parse_version_part(parts.next()?)?;
    let current = (parsed_major, parsed_minor, parsed_patch);
    Some(current >= (major, minor, patch))
}

fn parse_version_part(value: &str) -> Option<u64> {
    let digits: String = value.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn setup_local_path(req: &proto::SetupInboxRequest) -> Option<String> {
    req.folder_name
        .as_deref()
        .or(req.local_path.as_deref())
        .or(req.project_name.as_deref())
        .map(str::to_string)
}

fn path_label(value: &str) -> &str {
    Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(value)
}

async fn attach_model_elements(
    visual_diff: &mut serde_json::Value,
    bin: &str,
    dir: &Path,
    reference: Option<&str>,
) {
    let Ok(elements) = model_preview_elements(bin, dir, reference).await else {
        return;
    };
    if elements.is_empty() {
        return;
    }
    if let Some(object) = visual_diff.as_object_mut() {
        object
            .entry("model_elements".to_string())
            .or_insert_with(|| serde_json::Value::Array(elements));
    }
}

async fn baseline_visual_json(
    bin: &str,
    dir: &Path,
    reference: Option<&str>,
) -> Result<serde_json::Value, BridgeError> {
    let elements = model_preview_elements(bin, dir, reference).await?;
    Ok(serde_json::json!({
        "schema": "vex.visual-diff/1",
        "status": "baseline",
        "summary": "Baseline model",
        "counts": {
            "added": 0,
            "removed": 0,
            "modified": 0,
            "moved": 0,
            "renamed": 0,
            "unchanged": elements.len()
        },
        "elements": elements,
    }))
}

/// Build the current element inventory for a project. Prefers the authoritative
/// `vex elements` engine command (reads the committed tree directly); falls back
/// to an approximate regex scan of the newest IFC snapshot when the engine lacks
/// the command or the directory is not a local Vex repo. Each element is tagged
/// with `data_source` so consumers can tell authoritative data from a guess.
async fn model_preview_elements(
    bin: &str,
    dir: &Path,
    reference: Option<&str>,
) -> Result<Vec<serde_json::Value>, BridgeError> {
    if is_local_vex_repo(dir) {
        let reference = reference.unwrap_or("HEAD");
        match vex_cli::elements_json(bin, dir, reference).await {
            Ok(payload) => {
                if let Some(list) = payload.get("elements").and_then(|value| value.as_array()) {
                    let elements = list
                        .iter()
                        .take(2000)
                        .enumerate()
                        .map(|(index, element)| authoritative_element(element, index))
                        .collect();
                    return Ok(elements);
                }
            }
            Err(error) => {
                tracing::debug!(
                    %error,
                    "vex elements unavailable; using approximate preview parse"
                );
            }
        }
    }
    approximate_preview_elements(dir)
}

/// Map one `vex.elements/1` record to the dashboard's element shape.
fn authoritative_element(element: &serde_json::Value, index: usize) -> serde_json::Value {
    let type_name = element
        .get("type_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("IFC element");
    let name = element.get("name").and_then(serde_json::Value::as_str);
    let global_id = element.get("global_id").and_then(serde_json::Value::as_str);
    let step_id = element.get("step_id").and_then(serde_json::Value::as_u64);
    let id = global_id
        .map(str::to_string)
        .or_else(|| step_id.map(|value| format!("#{value}")))
        .unwrap_or_else(|| format!("element-{index}"));
    serde_json::json!({
        "kind": "unchanged",
        "type": type_name,
        "type_name": type_name,
        "name": name,
        "hint": name,
        "id": id,
        "stable_id": global_id.unwrap_or_default(),
        "global_id": global_id,
        "step_id": step_id,
        "preview_index": index,
        "data_source": "authoritative",
    })
}

/// Approximate inventory: a regex scan of the newest IFC snapshot. Used only as
/// a fallback. Names are surfaced when the parser found them but are never
/// fabricated from STEP ids.
fn approximate_preview_elements(dir: &Path) -> Result<Vec<serde_json::Value>, BridgeError> {
    let Some(path) = latest_ifc_snapshot(dir)? else {
        return Ok(Vec::new());
    };
    let elements = parse_preview_elements(&path, 2000)?
        .into_iter()
        .enumerate()
        .map(|(index, element)| {
            serde_json::json!({
                "kind": "unchanged",
                "type": element.type_name,
                "type_name": element.type_name,
                "name": element.name,
                "hint": element.name,
                "id": format!("#{}", element.step_id),
                "stable_id": "",
                "step_id": element.step_id,
                "preview_index": index,
                "data_source": "approximate",
            })
        })
        .collect();
    Ok(elements)
}

fn latest_ifc_snapshot(dir: &Path) -> Result<Option<PathBuf>, BridgeError> {
    fn visit(
        dir: &Path,
        best: &mut Option<(PathBuf, std::time::SystemTime)>,
    ) -> Result<(), BridgeError> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let meta = entry.metadata()?;
            if meta.is_dir() {
                if path
                    .components()
                    .any(|component| matches!(component, Component::Normal(name) if name == ".vex"))
                    && path.file_name().and_then(|name| name.to_str()) != Some("archive")
                {
                    let archive = path.join("archive");
                    if archive.is_dir() {
                        visit(&archive, best)?;
                    }
                    continue;
                }
                visit(&path, best)?;
                continue;
            }
            let is_ifc = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.eq_ignore_ascii_case("ifc"))
                .unwrap_or(false);
            if !is_ifc {
                continue;
            }
            let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            match best {
                Some((_, current)) if *current >= modified => {}
                _ => *best = Some((path, modified)),
            }
        }
        Ok(())
    }

    let mut best = None;
    visit(dir, &mut best)?;
    Ok(best.map(|(path, _)| path))
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
        pending_push_count: state.pending_push_count(),
        projects,
    }
}

fn find_watch_entry(cfg: &Config, project_id: &str) -> Option<crate::config::WatchEntry> {
    cfg.watch
        .iter()
        .find(|watch| watch.project_id == project_id)
        .cloned()
}

fn is_local_vex_repo(dir: &Path) -> bool {
    dir.join(".vex").join("config.toml").is_file()
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

fn default_inbox_root() -> Result<PathBuf, String> {
    let home = directories::UserDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .ok_or_else(|| "could not resolve user home".to_string())?;
    Ok(home.join("VexInbox"))
}

fn default_inbox_path(
    project_id: &str,
    project_name: Option<&str>,
    folder_name: Option<&str>,
) -> Result<PathBuf, String> {
    let root = default_inbox_root()?;
    let label = project_name
        .or(folder_name)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(project_id);
    Ok(root.join(safe_path_segment(path_label(label))))
}

fn resolve_inbox_path(value: &str) -> Result<PathBuf, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("project folder name cannot be empty".to_string());
    }
    let root = default_inbox_root()?;
    let raw = Path::new(trimmed);
    if raw.is_absolute() {
        if raw.starts_with(&root) {
            return Ok(raw.to_path_buf());
        }
        return Err(format!(
            "tracked folders must live inside {}",
            root.display()
        ));
    }
    Ok(root.join(safe_path_segment(path_label(trimmed))))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_project_id_uses_explicit_value() {
        let req = proto::SetupInboxRequest {
            project_id: Some("prj_manual_001".into()),
            project_name: None,
            folder_name: None,
            local_path: None,
            include: None,
            ifc_project_guid: None,
        };
        let (project_id, auto_generated) = setup_project_id(&req);
        assert_eq!(project_id, "prj_manual_001");
        assert!(!auto_generated);
    }

    #[test]
    fn setup_project_id_generates_uuid_when_missing() {
        let req = proto::SetupInboxRequest {
            project_id: None,
            project_name: Some("Tower A".into()),
            folder_name: Some("tower-a".into()),
            local_path: None,
            include: None,
            ifc_project_guid: None,
        };
        let (project_id, auto_generated) = setup_project_id(&req);
        assert!(auto_generated);
        assert!(project_id.starts_with("vex-"));
        assert_eq!(project_id.len(), 36);
    }

    #[test]
    fn validate_project_id_rejects_invalid_chars() {
        assert!(validate_project_id("ok-Project_123").is_ok());
        assert!(validate_project_id("bad id").is_err());
        assert!(validate_project_id("bad/id").is_err());
    }

    #[test]
    fn version_at_least_parses_semver_prefixes() {
        assert_eq!(version_at_least("0.1.4", 0, 1, 3), Some(true));
        assert_eq!(version_at_least("v0.1.2", 0, 1, 3), Some(false));
        assert_eq!(version_at_least("0.1.3-beta.1", 0, 1, 3), Some(true));
        assert_eq!(version_at_least("unknown", 0, 1, 3), None);
    }

    #[test]
    fn version_greater_compares_semver() {
        assert_eq!(version_greater("0.2.34", "0.2.33"), Some(true));
        assert_eq!(version_greater("v0.3.0", "0.2.33"), Some(true));
        assert_eq!(version_greater("0.2.33", "0.2.33"), Some(false));
        assert_eq!(version_greater("0.2.32", "0.2.33"), Some(false));
        assert_eq!(version_greater("0.2.34-canary.1", "0.2.33"), Some(true));
        assert_eq!(version_greater("garbage", "0.2.33"), None);
    }

    #[test]
    fn semver_tuple_strips_prefix_and_metadata() {
        assert_eq!(semver_tuple("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(semver_tuple("1.2.3-rc.1+build5"), Some((1, 2, 3)));
        assert_eq!(semver_tuple("1.2"), Some((1, 2, 0)));
        assert_eq!(semver_tuple(""), None);
    }
}
