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
    extract::{Path as AxumPath, Query, Request, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use tokio::sync::{Mutex, RwLock};
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
    /// Serializes project deletion with federation membership writes. These
    /// operations span both config and durable state, so neither lock alone can
    /// prevent a saved federation from acquiring a dangling source project.
    pub federation_project_lock: Arc<Mutex<()>>,
    /// Notified when the daemon should shut down gracefully (e.g. a desktop
    /// client detected a version mismatch and requested a clean restart).
    pub shutdown: Arc<tokio::sync::Notify>,
    /// Per-channel cache of the last GitHub update lookup, so the in-app
    /// update check doesn't hammer the GitHub API (or its rate limit) on every
    /// dashboard poll.
    pub update_cache: Arc<tokio::sync::Mutex<HashMap<String, (Instant, proto::UpdateInfo)>>>,
}

fn canonical_render_object_uri(project_id: &str, commit: &str, sha256: &str) -> String {
    format!("/v1/projects/{project_id}/render/{commit}/objects/{sha256}")
}

fn validate_render_resource_uris(
    manifest: &proto::RenderArtifactManifest,
    project_id: &str,
    commit: &str,
) -> Result<(), String> {
    for resource in manifest.resources() {
        let expected = canonical_render_object_uri(project_id, commit, &resource.sha256);
        if resource.uri != expected {
            return Err(format!(
                "render artifact resource URI does not match its canonical object endpoint: expected `{expected}`"
            ));
        }
    }
    Ok(())
}

/// Result of interpreting a client `Range` header against a known object
/// length. Render objects are immutable and content-addressed, so only a
/// single `bytes=` range is meaningful; anything else is made safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ByteRange {
    /// No range, an unknown unit, or a malformed/multi range: serve the whole
    /// object with `200 OK` (RFC 7233 permits ignoring the Range header).
    Full,
    /// A single satisfiable inclusive range `start..=end`.
    Partial { start: u64, end: u64 },
    /// A syntactically valid but unsatisfiable range: answer `416`.
    Unsatisfiable,
}

/// Parse a standards-compliant single `bytes=` range. Only one range is
/// supported; a multi-range set, an unknown unit, or malformed syntax degrades
/// safely to [`ByteRange::Full`] rather than erroring.
fn parse_render_byte_range(header: Option<&str>, len: u64) -> ByteRange {
    let Some(raw) = header else {
        return ByteRange::Full;
    };
    let Some(spec) = raw.trim().strip_prefix("bytes=") else {
        // Unknown range unit: ignore per RFC 7233 §3.1 and serve the full body.
        return ByteRange::Full;
    };
    let spec = spec.trim();
    if spec.is_empty() || spec.contains(',') {
        // A multi-range set is deliberately unsupported: serve the full body.
        return ByteRange::Full;
    }
    let Some((start_raw, end_raw)) = spec.split_once('-') else {
        return ByteRange::Full;
    };
    let start_raw = start_raw.trim();
    let end_raw = end_raw.trim();
    // No content can satisfy any range.
    if len == 0 {
        return ByteRange::Unsatisfiable;
    }
    match (start_raw.is_empty(), end_raw.is_empty()) {
        // Suffix range: `bytes=-N` selects the last N bytes.
        (true, false) => match end_raw.parse::<u64>() {
            Ok(0) => ByteRange::Unsatisfiable,
            Ok(suffix) => ByteRange::Partial {
                start: len.saturating_sub(suffix),
                end: len - 1,
            },
            Err(_) => ByteRange::Full,
        },
        // Open-ended range: `bytes=N-` selects from N to the end.
        (false, true) => match start_raw.parse::<u64>() {
            Ok(start) if start < len => ByteRange::Partial {
                start,
                end: len - 1,
            },
            Ok(_) => ByteRange::Unsatisfiable,
            Err(_) => ByteRange::Full,
        },
        // Closed range: `bytes=A-B`.
        (false, false) => match (start_raw.parse::<u64>(), end_raw.parse::<u64>()) {
            (Ok(start), Ok(end)) if start > end => ByteRange::Full,
            (Ok(start), Ok(_)) if start >= len => ByteRange::Unsatisfiable,
            (Ok(start), Ok(end)) => ByteRange::Partial {
                start,
                end: end.min(len - 1),
            },
            _ => ByteRange::Full,
        },
        // `bytes=-` is malformed: serve the full body.
        (true, true) => ByteRange::Full,
    }
}

/// Build the HTTP response for a verified, permitted render object serve.
///
/// Every branch advertises `Accept-Ranges: bytes`, the immutable SHA-256 ETag,
/// and immutable cache headers. A satisfiable range yields `206 Partial
/// Content` with a `Content-Range`; an unsatisfiable range yields `416` with a
/// `Content-Range: bytes */<total>`.
fn render_object_response(
    content_type: &str,
    sha256: &str,
    bytes: Vec<u8>,
    range: ByteRange,
) -> Response {
    let total = bytes.len() as u64;
    let etag = format!("\"{sha256}\"");
    let mut response = match range {
        ByteRange::Unsatisfiable => StatusCode::RANGE_NOT_SATISFIABLE.into_response(),
        ByteRange::Partial { start, end } => {
            let slice = bytes[start as usize..=end as usize].to_vec();
            (
                StatusCode::PARTIAL_CONTENT,
                [(header::CONTENT_TYPE, content_type)],
                slice,
            )
                .into_response()
        }
        ByteRange::Full => ([(header::CONTENT_TYPE, content_type)], bytes).into_response(),
    };
    let headers = response.headers_mut();
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&etag).expect("validated SHA-256 is a valid ETag"),
    );
    match range {
        ByteRange::Partial { start, end } => {
            headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {start}-{end}/{total}"))
                    .expect("byte range is a valid Content-Range"),
            );
        }
        ByteRange::Unsatisfiable => {
            headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{total}"))
                    .expect("unsatisfied range is a valid Content-Range"),
            );
        }
        ByteRange::Full => {}
    }
    response
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

async fn checkout_cache_is_complete(out: &Path, complete: &Path) -> bool {
    let Ok(expected_bytes) = tokio::fs::read_to_string(complete).await else {
        return false;
    };
    let Ok(expected_bytes) = expected_bytes.trim().parse::<u64>() else {
        return false;
    };
    tokio::fs::metadata(out)
        .await
        .map(|metadata| metadata.len() == expected_bytes && expected_bytes > 0)
        .unwrap_or(false)
}

async fn checkout_temp_has_expected_size(path: &Path, expected_bytes: u64) -> bool {
    tokio::fs::metadata(path)
        .await
        .map(|metadata| metadata.len() == expected_bytes && expected_bytes > 0)
        .unwrap_or(false)
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
        .route("/v1/cloud/projects", get(handle_cloud_projects))
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
            "/v1/projects/:project_id/render/:commit/status",
            get(handle_project_render_status),
        )
        .route(
            "/v1/projects/:project_id/render/:commit/manifest",
            get(handle_project_render_manifest),
        )
        .route(
            "/v1/projects/:project_id/render/:commit/objects/:sha256",
            get(handle_project_render_object),
        )
        .route(
            "/v1/projects/:project_id/inbox",
            post(handle_project_inbox_upload),
        )
        .route("/v1/projects/:project_id", delete(handle_delete_project))
        .route(
            "/v1/federations",
            get(handle_list_federations).post(handle_create_federation),
        )
        .route(
            "/v1/federations/:federation_id",
            get(handle_get_federation)
                .patch(handle_update_federation)
                .delete(handle_delete_federation),
        )
        .route(
            "/v1/federations/:federation_id/snapshot",
            get(handle_federation_snapshot),
        )
        .route("/v1/repo/push", post(handle_repo_push))
        .route("/v1/repo/register", post(handle_repo_register))
        .route("/v1/daemon/shutdown", post(handle_daemon_shutdown))
        .route("/v1/update/check", get(handle_update_check))
        .route("/v1/update/apply", post(handle_update_apply))
        .route("/v1/diagnostics", get(handle_diagnostics))
        .layer(middleware::from_fn(add_cross_origin_isolation_headers))
        .with_state(state)
}

/// Mark every response as cross-origin isolated.
///
/// The bundled `web-ifc` viewer ships a multi-threaded WASM build
/// (`web-ifc-mt.wasm`) that parses/triangulates IFC geometry across a worker
/// pool sized to `navigator.hardwareConcurrency` instead of on a single main
/// thread. `web-ifc` only selects that build when `self.crossOriginIsolated`
/// is `true` in the browser, which in turn requires every response from this
/// origin to carry `Cross-Origin-Opener-Policy: same-origin` and
/// `Cross-Origin-Embedder-Policy: require-corp`. Without these headers the
/// browser silently falls back to the single-threaded `web-ifc.wasm` build,
/// so large IFC models tessellate on one core and the 3D preview feels stuck
/// even though the final draw call is already GPU-accelerated via WebGL.
/// Everything served by this daemon is same-origin (127.0.0.1), so enabling
/// this has no cross-origin side effects here.
async fn add_cross_origin_isolation_headers(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        HeaderName::from_static("cross-origin-embedder-policy"),
        HeaderValue::from_static("require-corp"),
    );
    response
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
        if let BridgeError::Config(message) = error {
            // A project that still backs a federation cannot be deleted; this is
            // distinct from a project-id remap conflict and needs its own code
            // and hint so the UI guides the operator to the federations.
            if message.contains("referenced by") && message.contains("federation") {
                return (
                    "project_referenced_by_federation",
                    Some("Remove this project from its federations (or delete them) before deleting the project."),
                    false,
                );
            }
            return (
                "project_id_conflict",
                Some("Choose a different project id or retry with allow_replace=true."),
                false,
            );
        }
    }
    match error {
        BridgeError::NotPaired => (
            "not_paired",
            Some("Pair this device before syncing projects."),
            false,
        ),
        BridgeError::PairingKeyUnavailable => (
            "pairing_key_unavailable",
            Some(
                "This device's pairing record is incomplete or its private key is unavailable. Pair this device again.",
            ),
            false,
        ),
        BridgeError::PairingCredentialsRejected => (
            "pairing_credentials_rejected",
            Some(
                "The cloud service rejected this device key. Pair this device again; this is not a browser sign-in problem.",
            ),
            false,
        ),
        BridgeError::CloudProjectsUnavailable(_) => (
            "cloud_projects_unavailable",
            Some("The cloud projects service did not respond after retrying. Check your connection and retry."),
            true,
        ),
        BridgeError::CloudProjectsInvalidResponse(_) => (
            "cloud_projects_invalid_response",
            Some("The cloud projects response was incompatible. Update Vex Atlas or contact support."),
            false,
        ),
        BridgeError::UpstreamApi(_) => (
            "upstream_api_error",
            Some("Check your network connection and retry."),
            true,
        ),
        BridgeError::VexCli(msg) if is_repo_locked(msg) => (
            "repo_locked",
            Some("Another vex process is using this project. Click Repair, then retry."),
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

/// Detect the engine's redb exclusive-lock failure in a CLI error message. The
/// object store takes an OS-level lock, so two `vex` processes touching the same
/// repo fail with this signature. We classify it distinctly so the UI can offer
/// "Repair" (retire the stale daemon) instead of a generic engine error.
fn is_repo_locked(message: &str) -> bool {
    let m = message.to_ascii_lowercase();
    m.contains("database already open") || m.contains("cannot acquire lock")
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
        "three/examples/jsm/loaders/GLTFLoader.js" => Some((
            include_bytes!("../assets/viewer/three/examples/jsm/loaders/GLTFLoader.js"),
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
        "web-ifc-three/IFCWorker.js" => Some((
            include_bytes!("../assets/viewer/web-ifc-three/IFCWorker.js"),
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
        paired: daemon_state.has_usable_pairing_record(),
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

/// `GET /v1/diagnostics` — a one-shot, copy-pasteable health report for support.
///
/// Bundles everything needed to diagnose the "stale daemon / version skew / repo
/// locked" class of problems in a single token-gated payload: the running
/// build, PID/uptime, the advisory lockfile, the bundled engine version and its
/// schema compatibility, how many watchers are active, pair state, and a short
/// tail of the daemon log. Sensitive values (the access token, and the user's
/// home-directory prefix) are redacted before they leave the process so the
/// report is safe to paste into a bug tracker.
async fn handle_diagnostics(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, Response> {
    require_token(&headers, &state.access_token)?;
    let cfg = state.config.read().await.clone();
    let daemon_state = state.state.read().await.clone();
    let vex_version = vex_cli::version(&cfg.vex_bin).await.ok().flatten();
    let vex_schema_compatible = vex_version
        .as_deref()
        .and_then(|version| version_at_least(version, 0, 1, 3));
    let active_watchers = state.watchers.read().await.len();
    let lock = crate::daemon_lock::read(&state.paths);
    let log_tail = read_log_tail(&state.paths.log_file, 40);

    let report = serde_json::json!({
        "bridge_version": env!("CARGO_PKG_VERSION"),
        "pid": std::process::id(),
        "uptime_seconds": state.started_at.elapsed().as_secs(),
        "port": cfg.port,
        "paired": daemon_state.has_usable_pairing_record(),
        "active_watchers": active_watchers,
        "vex_version": vex_version,
        "vex_bin": cfg.vex_bin,
        "expected_visual_diff_schema": proto::schema::VISUAL_DIFF,
        "vex_schema_compatible": vex_schema_compatible,
        "lockfile": lock,
        "log_tail": redact(&log_tail, &state.access_token),
    });
    Ok(Json(report))
}

/// Read the last `lines` lines of the daemon log, best-effort. Returns an empty
/// string if the log can't be read.
fn read_log_tail(path: &std::path::Path, lines: usize) -> String {
    let Ok(content) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

/// Redact secrets before diagnostics leave the process: the user's home prefix
/// (so absolute paths don't leak the username) and the access token.
fn redact(text: &str, token: &str) -> String {
    let mut out = text.to_string();
    if let Some(home) = dirs_home() {
        if !home.is_empty() {
            out = out.replace(&home, "~");
        }
    }
    if token.len() >= 8 {
        out = out.replace(token, "[redacted-token]");
    }
    out
}

fn dirs_home() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
        .filter(|h| !h.is_empty())
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

/// `POST /v1/update/apply?channel=stable|canary` — download the verified
/// platform installer and launch it. The installer (already trusted; it ships
/// the app) closes the running app, swaps binaries, and relaunches. We only
/// download, integrity-check, and launch it detached so a process-tree kill by
/// the installer can't take the spawning daemon's child with it.
async fn handle_update_apply(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<UpdateQuery>,
) -> Result<Json<proto::UpdateApplyResponse>, Response> {
    require_token(&headers, &state.access_token)?;
    let channel = match query.channel.as_deref() {
        Some("canary") => "canary",
        _ => "stable",
    }
    .to_string();

    // Always recompute (never trust a possibly-stale cache for an action that
    // launches an executable), then refresh the cache as a side effect.
    let info = compute_update_info(&channel).await;
    state
        .update_cache
        .lock()
        .await
        .insert(channel, (Instant::now(), info.clone()));

    let mut response = proto::UpdateApplyResponse {
        launched: false,
        target_version: info.latest_version.clone(),
        release_url: info.release_url.clone(),
        error: None,
    };

    if !info.update_available {
        response.error = Some("already up to date".into());
        return Ok(Json(response));
    }
    let (Some(url), Some(expected_sha)) = (info.installer_url, info.installer_sha256) else {
        response.error = Some("no verifiable installer for this platform".into());
        return Ok(Json(response));
    };

    match apply_installer(&url, &expected_sha, info.latest_version.as_deref()).await {
        Ok(()) => response.launched = true,
        Err(message) => response.error = Some(message),
    }
    Ok(Json(response))
}

/// Download the installer, verify its SHA-256 against the release manifest, and
/// launch it detached. The platform-specific launch lives in `launch_installer`.
async fn apply_installer(
    url: &str,
    expected_sha: &str,
    version: Option<&str>,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|error| error.to_string())?;
    let bytes = client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            concat!("vex-bridge/", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await
        .map_err(|error| format!("download failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("download failed: {error}"))?
        .bytes()
        .await
        .map_err(|error| format!("download failed: {error}"))?;

    let digest = Sha256::digest(&bytes);
    let actual_sha = hex_lower(&digest);
    if actual_sha != expected_sha.to_ascii_lowercase() {
        return Err("installer checksum mismatch; refusing to launch".into());
    }

    let label = version.unwrap_or("latest");
    let file_name = format!("VexAtlasSetup-{}.exe", safe_path_segment(label));
    let path = std::env::temp_dir().join(file_name);
    std::fs::write(&path, &bytes).map_err(|error| format!("write failed: {error}"))?;

    launch_installer(&path)
}

/// Launch the downloaded installer detached from this daemon. On Windows the
/// installer taskkills `vex-bridge.exe` and its tree, so we route through
/// `cmd /C start` to reparent it away from us first. Unsupported elsewhere.
#[cfg(target_os = "windows")]
fn launch_installer(path: &std::path::Path) -> Result<(), String> {
    let target = path.to_string_lossy().to_string();
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &target])
        .spawn()
        .map_err(|error| format!("launch failed: {error}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn launch_installer(_path: &std::path::Path) -> Result<(), String> {
    Err("in-app install is only supported on Windows".into())
}

/// Lowercase hex encoding of a byte slice (digest formatting).
fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
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
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// Filename fragments that identify the Windows installer asset the apply flow
/// can launch. Other platforms have no equivalent one-click installer (only
/// tarballs), so their asset match fails and apply falls back to the release
/// page. Always compiled so `GithubAsset` is exercised on every target.
const INSTALLER_ASSET_PREFIX: &str = "VexAtlasSetup";
const INSTALLER_ASSET_SUFFIX: &str = "-windows-x86_64.exe";

/// Locate the platform installer asset and its published SHA-256 for `release`.
/// Returns `(installer_url, installer_sha256)`. Either may be `None`: a missing
/// digest disables apply (the daemon never launches an unverified installer).
async fn resolve_installer_asset(release: &GithubRelease) -> (Option<String>, Option<String>) {
    let Some(installer) = release.assets.iter().find(|asset| {
        asset.name.starts_with(INSTALLER_ASSET_PREFIX)
            && asset.name.ends_with(INSTALLER_ASSET_SUFFIX)
    }) else {
        return (None, None);
    };
    let url = Some(installer.browser_download_url.clone());
    let Some(checksums) = release
        .assets
        .iter()
        .find(|asset| asset.name == "SHA256SUMS.txt")
    else {
        return (url, None);
    };
    let sha = fetch_installer_sha256(&checksums.browser_download_url, &installer.name).await;
    (url, sha)
}

/// Download the small `SHA256SUMS.txt` manifest and return the lowercase hex
/// digest recorded for `asset_name`, if present. Format per line:
/// `<hex>  <filename>` (sha256sum style; the name may carry a `*`/path prefix).
async fn fetch_installer_sha256(checksums_url: &str, asset_name: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;
    let text = client
        .get(checksums_url)
        .header(
            reqwest::header::USER_AGENT,
            concat!("vex-bridge/", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    parse_sha256sums(&text, asset_name)
}

/// Pure parse of a `sha256sum`-style manifest: return the lowercase hex digest
/// recorded for `asset_name`. Lines look like `<hex>  <filename>`; the filename
/// may carry a leading `*` (binary mode marker) or a path prefix.
fn parse_sha256sums(text: &str, asset_name: &str) -> Option<String> {
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        let name = name.trim_start_matches('*');
        if name.rsplit('/').next().unwrap_or(name) == asset_name {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
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
        installer_url: None,
        installer_sha256: None,
        can_apply: false,
        checked_at: rfc3339_from_unix(now_unix()),
        error: None,
    };
    match fetch_latest_release(channel).await {
        Ok(Some(release)) => {
            let latest = release.tag_name.trim_start_matches('v').to_string();
            info.update_available = version_greater(&latest, &current).unwrap_or(false);
            info.latest_version = Some(latest);
            info.release_url = Some(release.html_url.clone());
            info.release_notes = release.body.clone();
            info.published_at = release.published_at.clone();
            if info.update_available {
                let (installer_url, installer_sha256) = resolve_installer_asset(&release).await;
                info.can_apply = cfg!(target_os = "windows")
                    && installer_url.is_some()
                    && installer_sha256.is_some();
                info.installer_url = installer_url;
                info.installer_sha256 = installer_sha256;
            }
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

async fn handle_cloud_projects(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Vec<proto::CloudProject>>, Response> {
    require_token(&headers, &state.access_token)?;
    let cfg = state.config.read().await.clone();
    let key_id = {
        let daemon_state = state.state.read().await;
        match &daemon_state.pairing {
            PairingState::Paired { key_id, .. } if !key_id.trim().is_empty() => key_id.clone(),
            PairingState::Paired { .. } => {
                return Err(err_response(
                    StatusCode::CONFLICT,
                    BridgeError::PairingKeyUnavailable,
                ))
            }
            _ => {
                return Err(err_response(
                    StatusCode::UNAUTHORIZED,
                    BridgeError::NotPaired,
                ))
            }
        }
    };
    pairing::projects(&cfg, &key_id)
        .await
        .map(Json)
        .map_err(|error| {
            let status = match &error {
                BridgeError::PairingCredentialsRejected => StatusCode::UNAUTHORIZED,
                BridgeError::PairingKeyUnavailable => StatusCode::CONFLICT,
                _ => StatusCode::BAD_GATEWAY,
            };
            err_response(status, error)
        })
}

fn pair_status_from_state(state: &DaemonState) -> proto::PairStatus {
    pair_status_from_pairing(state, local_pairing_key_is_available(&state.pairing))
}

fn local_pairing_key_is_available(pairing: &PairingState) -> bool {
    let PairingState::Paired {
        key_fingerprint, ..
    } = pairing
    else {
        return true;
    };

    match crate::keychain::load() {
        Ok(Some(signing_key)) => {
            let actual_fingerprint = pairing::fingerprint_for(&signing_key);
            if actual_fingerprint == *key_fingerprint {
                true
            } else {
                warn!("local signing key does not match the persisted pairing record");
                false
            }
        }
        Ok(None) => {
            warn!("persisted pairing record has no local signing key");
            false
        }
        Err(error) => {
            warn!(%error, "could not read the local signing key for persisted pairing record");
            false
        }
    }
}

fn pair_status_from_pairing(
    state: &DaemonState,
    local_key_is_available: bool,
) -> proto::PairStatus {
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
            key_id,
        } if !key_id.trim().is_empty() && local_key_is_available => proto::PairStatus::Paired {
            device_label: device_label.clone(),
            key_fingerprint: key_fingerprint.clone(),
            paired_at: rfc3339_from_unix(*paired_at_unix),
            account_id: account_id.clone(),
            account_email: account_email.clone(),
            account_name: account_name.clone(),
        },
        PairingState::Paired { .. } => proto::PairStatus::Unpaired,
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
    let pair_status = pair_status_from_state(&daemon_state);
    let inbox_root = default_inbox_root().map_err(|error| {
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::Config(error),
        )
    })?;
    Ok(Json(proto::SetupStatus {
        paired: matches!(pair_status, proto::PairStatus::Paired { .. }),
        pair_status,
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
    let immutable = query.commit.as_deref().is_some_and(is_full_commit_hash);
    let path = resolve_ifc_snapshot_path(&state, &project_id, query.commit.as_deref()).await?;
    serve_ifc_file(path, immutable).await
}

async fn handle_project_ifc_commit(
    headers: HeaderMap,
    AxumPath((project_id, commit)): AxumPath<(String, String)>,
    State(state): State<AppState>,
) -> Result<Response, Response> {
    require_token(&headers, &state.access_token)?;
    let path = resolve_ifc_snapshot_path(&state, &project_id, Some(&commit)).await?;
    serve_ifc_file(path, is_full_commit_hash(&commit)).await
}

/// Report whether a validated, commit-exact derived render artifact is
/// available. Artifact generation intentionally remains independent of import
/// success: callers can always fall back to the canonical IFC endpoint.
async fn handle_project_render_status(
    headers: HeaderMap,
    AxumPath((project_id, commit)): AxumPath<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<proto::RenderArtifactStatusResponse>, Response> {
    require_token(&headers, &state.access_token)?;
    require_full_commit_hash(&commit)?;
    let status = match read_render_manifest(&state, &project_id, &commit).await? {
        Some(manifest) => proto::RenderArtifactStatus::Ready {
            manifest: Box::new(manifest),
        },
        None => match state
            .state
            .read()
            .await
            .render_artifact_status(&project_id, &commit)
        {
            Some(crate::state::RenderArtifactJobStatus::Queued) => {
                proto::RenderArtifactStatus::Queued
            }
            Some(crate::state::RenderArtifactJobStatus::Building) => {
                proto::RenderArtifactStatus::Building {
                    completed_tiles: 0,
                    total_tiles: None,
                }
            }
            Some(crate::state::RenderArtifactJobStatus::Failed { message, retryable }) => {
                proto::RenderArtifactStatus::Failed { message, retryable }
            }
            Some(crate::state::RenderArtifactJobStatus::Ready) => {
                // A ready state without a validated manifest is never served as
                // ready. It indicates a cache eviction or interrupted publish.
                proto::RenderArtifactStatus::NotRequested
            }
            None => proto::RenderArtifactStatus::NotRequested,
        },
    };
    Ok(Json(proto::RenderArtifactStatusResponse {
        schema: proto::schema::RENDER_STATUS.to_string(),
        project_id,
        commit_hash: commit,
        status,
    }))
}

/// Return a validated manifest for a derived render artifact. The manifest is
/// immutable data keyed by the complete Vex commit hash; no render artifact is
/// ever inferred from a branch, abbreviated hash, or the mutable `HEAD`.
async fn handle_project_render_manifest(
    headers: HeaderMap,
    AxumPath((project_id, commit)): AxumPath<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<proto::RenderArtifactManifest>, Response> {
    require_token(&headers, &state.access_token)?;
    require_full_commit_hash(&commit)?;
    let manifest = read_render_manifest(&state, &project_id, &commit)
        .await?
        .ok_or_else(|| {
            err_response(
                StatusCode::NOT_FOUND,
                BridgeError::Config(format!(
                    "no validated render artifact is available for commit `{commit}`"
                )),
            )
        })?;
    Ok(Json(manifest))
}

/// Serve an immutable, integrity-checked object named by a validated render
/// manifest. A client cannot read arbitrary files by inventing a hash: the
/// requested object must be present in the manifest and match the file digest.
async fn handle_project_render_object(
    headers: HeaderMap,
    AxumPath((project_id, commit, sha256)): AxumPath<(String, String, String)>,
    State(state): State<AppState>,
) -> Result<Response, Response> {
    require_token(&headers, &state.access_token)?;
    require_full_commit_hash(&commit)?;
    let manifest = read_render_manifest(&state, &project_id, &commit)
        .await?
        .ok_or_else(|| {
            err_response(
                StatusCode::NOT_FOUND,
                BridgeError::Config(format!(
                    "no validated render artifact is available for commit `{commit}`"
                )),
            )
        })?;
    let resource = manifest
        .resources()
        .find(|resource| resource.sha256 == sha256)
        .ok_or_else(|| {
            err_response(
                StatusCode::NOT_FOUND,
                BridgeError::Config("render artifact object is not in the manifest".into()),
            )
        })?;

    let root = render_artifact_dir(&state, &project_id, &commit).await?;
    // Hold a serve guard for the whole read so cache eviction can never delete
    // this artifact directory out from under an in-flight download.
    let _serve_guard = crate::render_artifact::ArtifactServeGuard::acquire(&root);
    let path = root.join("objects").join(&sha256);
    let bytes = tokio::fs::read(&path).await.map_err(|error| {
        let status = if error.kind() == std::io::ErrorKind::NotFound {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        err_response(status, BridgeError::Io(error))
    })?;
    let actual = sha256_hex(&bytes);
    if actual != resource.sha256 {
        return Err(err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::Config(format!(
                "render artifact object integrity check failed for `{sha256}`"
            )),
        ));
    }

    // The object is authorized, declared by the validated manifest, and its
    // bytes match the manifest digest: interpret any client range and build a
    // standards-compliant response over the immutable object.
    let range = parse_render_byte_range(
        headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok()),
        bytes.len() as u64,
    );
    let response = render_object_response(&resource.content_type, &sha256, bytes, range);

    // Access tracking updates only after a valid, permitted serve that
    // actually returns object bytes (200 or 206), never for an unsatisfiable
    // range. This drives least-recently-used cache eviction.
    if matches!(range, ByteRange::Full | ByteRange::Partial { .. }) {
        crate::render_artifact::record_artifact_access(&root);
    }
    Ok(response)
}

// Axum handlers return `Response` errors directly so callers can use `?`
// without repeatedly translating a validation failure into an HTTP response.
#[allow(clippy::result_large_err)]
fn require_full_commit_hash(commit: &str) -> Result<(), Response> {
    if is_full_commit_hash(commit) {
        Ok(())
    } else {
        Err(err_response(
            StatusCode::BAD_REQUEST,
            BridgeError::Config(
                "render artifacts require a complete 64-character commit hash".into(),
            ),
        ))
    }
}

async fn read_render_manifest(
    state: &AppState,
    project_id: &str,
    commit: &str,
) -> Result<Option<proto::RenderArtifactManifest>, Response> {
    let root = render_artifact_dir(state, project_id, commit).await?;
    // Only the bridge-created validated manifest may be served. The
    // worker-supplied input manifest stays in the artifact directory for
    // diagnostics but is never trusted after publication.
    let manifest_path = root.join("manifest.validated.json");
    let contents = match tokio::fs::read_to_string(&manifest_path).await {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                BridgeError::Io(error),
            ))
        }
    };
    let manifest: proto::RenderArtifactManifest =
        serde_json::from_str(&contents).map_err(|error| {
            err_response(
                StatusCode::UNPROCESSABLE_ENTITY,
                BridgeError::Config(format!("render artifact manifest is invalid JSON: {error}")),
            )
        })?;
    manifest.validate().map_err(|error| {
        err_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            BridgeError::Config(format!(
                "render artifact manifest failed validation: {error}"
            )),
        )
    })?;
    if manifest.project_id != project_id || manifest.commit_hash != commit {
        return Err(err_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            BridgeError::Config(
                "render artifact manifest identity does not match its request".into(),
            ),
        ));
    }
    validate_render_resource_uris(&manifest, project_id, commit).map_err(|error| {
        err_response(StatusCode::UNPROCESSABLE_ENTITY, BridgeError::Config(error))
    })?;
    Ok(Some(manifest))
}

async fn render_artifact_dir(
    state: &AppState,
    project_id: &str,
    commit: &str,
) -> Result<PathBuf, Response> {
    let cfg = state.config.read().await.clone();
    let entry =
        find_watch_entry(&cfg, project_id).ok_or_else(|| unknown_project_response(project_id))?;
    Ok(PathBuf::from(entry.path)
        .join(".vex")
        .join("cache")
        .join("render")
        .join(commit))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;

    format!("{:x}", sha2::Sha256::digest(bytes))
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

    // Serialize this state/config mutation with federation create and update.
    // A source-membership check outside this gate could race a federation write
    // that successfully resolved the project just before deletion.
    let _federation_project_guard = state.federation_project_lock.lock().await;
    // A project that participates in one or more federations must not be
    // deleted out from under those coordination sets: doing so would leave
    // silent dangling references to a project that no longer exists. Require
    // the operator to remove the project from (or delete) the referencing
    // federations first.
    let referencing = {
        let daemon_state = state.state.read().await;
        daemon_state.federations_referencing_project(&project_id)
    };
    if !referencing.is_empty() {
        return Err(err_response(
            StatusCode::CONFLICT,
            BridgeError::Config(format!(
                "project `{project_id}` is referenced by {} federation(s): {}. Remove it from those federations (or delete them) first.",
                referencing.len(),
                referencing.join(", ")
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

// ---------------------------------------------------------------------------
// Federations: token-gated CRUD + snapshot for local-first coordination sets.
//
// A federation is a derived, saved set of exact discipline-model commit
// references plus placement transforms over *independent* local projects. It
// never merges models and makes no semantic-merge guarantee. All routes are
// token-gated exactly like the other `/v1` endpoints, and every identifier is
// validated against a safe character set so it can never encode a path.
// ---------------------------------------------------------------------------

/// Why a requested member commit could not be pinned. Kept separate from the
/// HTTP layer so the resolution logic is unit-testable without a live engine.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CommitResolveError {
    /// An explicit hash was supplied but is not a complete 64 hex commit.
    InvalidHash(String),
    /// The supplied (complete) hash is not present in the project's history.
    NotInHistory(String),
    /// HEAD was requested but the source project has no commits to pin.
    NoCommits,
}

/// Resolve the immutable commit a member should pin, given the source project's
/// commit history (newest first). An explicit hash must be complete and present
/// in history; otherwise the project's HEAD (the newest commit) is pinned. The
/// returned hash is always the exact string recorded in history, so the stored
/// member identity matches the engine's canonical hash.
fn resolve_member_commit(
    commits: &[proto::CommitSummary],
    explicit: Option<&str>,
) -> Result<String, CommitResolveError> {
    match explicit.map(str::trim).filter(|value| !value.is_empty()) {
        Some(hash) => {
            if !proto::is_full_commit_hash(hash) {
                return Err(CommitResolveError::InvalidHash(hash.to_string()));
            }
            commits
                .iter()
                .find(|commit| commit.commit.eq_ignore_ascii_case(hash))
                .map(|commit| commit.commit.clone())
                .ok_or_else(|| CommitResolveError::NotInHistory(hash.to_string()))
        }
        None => commits
            .first()
            .map(|commit| commit.commit.clone())
            .ok_or(CommitResolveError::NoCommits),
    }
}

fn unknown_federation_response(federation_id: &str) -> Response {
    err_response(
        StatusCode::NOT_FOUND,
        BridgeError::Config(format!("unknown federation `{federation_id}`")),
    )
}

fn unknown_source_project_response(project_id: &str) -> Response {
    err_response(
        StatusCode::NOT_FOUND,
        BridgeError::Config(format!(
            "unknown source project `{project_id}`; register it locally before referencing it in a federation"
        )),
    )
}

fn federation_bad_request(message: impl Into<String>) -> Response {
    err_response(StatusCode::BAD_REQUEST, BridgeError::Config(message.into()))
}

fn federation_validation_response(error: proto::FederationValidationError) -> Response {
    federation_bad_request(format!("invalid federation: {error}"))
}

fn commit_resolve_response(project_id: &str, error: CommitResolveError) -> Response {
    let message = match error {
        CommitResolveError::InvalidHash(hash) => format!(
            "member commit `{hash}` for project `{project_id}` must be a complete 64 hex character commit hash"
        ),
        CommitResolveError::NotInHistory(hash) => format!(
            "commit `{hash}` is not in the history of project `{project_id}`"
        ),
        CommitResolveError::NoCommits => format!(
            "source project `{project_id}` has no commits to pin; import and commit a model first"
        ),
    };
    federation_bad_request(message)
}

/// Fetch a source project's commit history for federation resolution. Returns
/// an error response when the project is not configured locally; an empty list
/// when it is configured but has no local vex repo yet.
async fn source_project_commits(
    cfg: &Config,
    project_id: &str,
) -> Result<Vec<proto::CommitSummary>, Response> {
    let entry = find_watch_entry(cfg, project_id)
        .ok_or_else(|| unknown_source_project_response(project_id))?;
    let dir = PathBuf::from(&entry.path);
    if !is_local_vex_repo(&dir) {
        return Ok(Vec::new());
    }
    let log = vex_cli::log_json(&cfg.vex_bin, &dir)
        .await
        .map_err(|error| err_response(StatusCode::BAD_GATEWAY, error))?;
    Ok(commits_from_log(log))
}

/// Load and cache a project's commit history within a single request so a
/// federation that references the same project several times issues one engine
/// call, not one per member.
async fn cached_source_commits(
    cfg: &Config,
    cache: &mut HashMap<String, Vec<proto::CommitSummary>>,
    project_id: &str,
) -> Result<Vec<proto::CommitSummary>, Response> {
    if let Some(commits) = cache.get(project_id) {
        return Ok(commits.clone());
    }
    let commits = source_project_commits(cfg, project_id).await?;
    cache.insert(project_id.to_string(), commits.clone());
    Ok(commits)
}

fn normalized_discipline(discipline: Option<&String>) -> Option<String> {
    discipline
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn handle_list_federations(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<Vec<proto::FederationSummary>>, Response> {
    require_token(&headers, &state.access_token)?;
    let daemon_state = state.state.read().await;
    let summaries = daemon_state
        .federations()
        .iter()
        .map(proto::Federation::summary)
        .collect();
    Ok(Json(summaries))
}

async fn handle_get_federation(
    headers: HeaderMap,
    AxumPath(federation_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<proto::Federation>, Response> {
    require_token(&headers, &state.access_token)?;
    let federation = state
        .state
        .read()
        .await
        .federation(&federation_id)
        .ok_or_else(|| unknown_federation_response(&federation_id))?;
    Ok(Json(federation))
}

async fn handle_create_federation(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<proto::CreateFederationRequest>,
) -> Result<Json<proto::Federation>, Response> {
    require_token(&headers, &state.access_token)?;
    if req.name.trim().is_empty() {
        return Err(federation_bad_request("federation name must not be empty"));
    }
    if req.members.is_empty() {
        return Err(federation_bad_request(
            "a federation must have at least one member",
        ));
    }
    if req.members.len() > proto::Federation::MAX_MEMBERS {
        return Err(federation_bad_request(format!(
            "a federation may have at most {} members",
            proto::Federation::MAX_MEMBERS
        )));
    }

    let cfg = state.config.read().await.clone();
    let mut commit_cache: HashMap<String, Vec<proto::CommitSummary>> = HashMap::new();
    let mut members = Vec::with_capacity(req.members.len());
    for input in &req.members {
        let source = input.source_project_id.trim();
        validate_project_id(source).map_err(federation_bad_request)?;
        let commits = cached_source_commits(&cfg, &mut commit_cache, source).await?;
        let commit = resolve_member_commit(&commits, input.commit_hash.as_deref())
            .map_err(|error| commit_resolve_response(source, error))?;
        members.push(proto::FederationMember {
            member_id: format!("mem-{}", Uuid::now_v7().simple()),
            source_project_id: source.to_string(),
            commit_hash: commit,
            display_name: input.display_name.trim().to_string(),
            discipline: normalized_discipline(input.discipline.as_ref()),
            transform: input.transform.unwrap_or_default(),
            visible: input.visible.unwrap_or(true),
        });
    }

    let now = now_unix();
    let federation = proto::Federation {
        schema: proto::schema::FEDERATION.to_string(),
        federation_id: format!("fed-{}", Uuid::now_v7().simple()),
        name: req.name.trim().to_string(),
        created_at_unix: now,
        updated_at_unix: now,
        members,
    };
    federation
        .validate()
        .map_err(federation_validation_response)?;

    // The source projects may have been deleted while commit histories were
    // being resolved. Recheck under the mutation gate before persisting.
    let _federation_project_guard = state.federation_project_lock.lock().await;
    {
        let cfg = state.config.read().await;
        for member in &federation.members {
            if find_watch_entry(&cfg, &member.source_project_id).is_none() {
                return Err(unknown_project_response(&member.source_project_id));
            }
        }
    }
    {
        let mut daemon_state = state.state.write().await;
        if !daemon_state.insert_federation(federation.clone()) {
            return Err(err_response(
                StatusCode::CONFLICT,
                BridgeError::Config(format!(
                    "the maximum number of federations ({}) has been reached",
                    crate::state::State::MAX_FEDERATIONS
                )),
            ));
        }
        daemon_state
            .save(&state.paths)
            .map_err(|error| err_response(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    }
    info!(federation_id = %federation.federation_id, members = federation.members.len(), "created federation");
    Ok(Json(federation))
}

async fn handle_update_federation(
    headers: HeaderMap,
    AxumPath(federation_id): AxumPath<String>,
    State(state): State<AppState>,
    Json(req): Json<proto::UpdateFederationRequest>,
) -> Result<Json<proto::Federation>, Response> {
    require_token(&headers, &state.access_token)?;
    let members_replaced = req.members.is_some();
    let mut federation = state
        .state
        .read()
        .await
        .federation(&federation_id)
        .ok_or_else(|| unknown_federation_response(&federation_id))?;

    if let Some(name) = req.name.as_ref() {
        if name.trim().is_empty() {
            return Err(federation_bad_request("federation name must not be empty"));
        }
        federation.name = name.trim().to_string();
    }

    if let Some(inputs) = req.members.as_ref() {
        if inputs.is_empty() {
            return Err(federation_bad_request(
                "a federation must have at least one member",
            ));
        }
        if inputs.len() > proto::Federation::MAX_MEMBERS {
            return Err(federation_bad_request(format!(
                "a federation may have at most {} members",
                proto::Federation::MAX_MEMBERS
            )));
        }
        let cfg = state.config.read().await.clone();
        let existing_ids: HashSet<String> = federation
            .members
            .iter()
            .map(|member| member.member_id.clone())
            .collect();
        let mut commit_cache: HashMap<String, Vec<proto::CommitSummary>> = HashMap::new();
        let mut used_ids: HashSet<String> = HashSet::new();
        let mut new_members = Vec::with_capacity(inputs.len());
        for input in inputs {
            let source = input.source_project_id.trim();
            validate_project_id(source).map_err(federation_bad_request)?;
            let commits = cached_source_commits(&cfg, &mut commit_cache, source).await?;
            let commit = resolve_member_commit(&commits, input.commit_hash.as_deref())
                .map_err(|error| commit_resolve_response(source, error))?;
            // A supplied member_id must reference an existing member so the
            // stable identity is preserved; an unknown or repeated id is a
            // client error, never a silently minted new member.
            let member_id = match input
                .member_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                Some(id) => {
                    if !existing_ids.contains(id) {
                        return Err(federation_bad_request(format!(
                            "member_id `{id}` does not exist in federation `{federation_id}`"
                        )));
                    }
                    if !used_ids.insert(id.to_string()) {
                        return Err(federation_bad_request(format!(
                            "member_id `{id}` is repeated in the update"
                        )));
                    }
                    id.to_string()
                }
                None => format!("mem-{}", Uuid::now_v7().simple()),
            };
            new_members.push(proto::FederationMember {
                member_id,
                source_project_id: source.to_string(),
                commit_hash: commit,
                display_name: input.display_name.trim().to_string(),
                discipline: normalized_discipline(input.discipline.as_ref()),
                transform: input.transform.unwrap_or_default(),
                visible: input.visible.unwrap_or(true),
            });
        }
        federation.members = new_members;
    }

    federation.updated_at_unix = now_unix();
    federation
        .validate()
        .map_err(federation_validation_response)?;

    // The source projects may have been deleted while commit histories were
    // being resolved. Recheck under the mutation gate before persisting.
    let _federation_project_guard = state.federation_project_lock.lock().await;
    if members_replaced {
        let cfg = state.config.read().await;
        for member in &federation.members {
            if find_watch_entry(&cfg, &member.source_project_id).is_none() {
                return Err(unknown_project_response(&member.source_project_id));
            }
        }
    }
    {
        let mut daemon_state = state.state.write().await;
        if !daemon_state.replace_federation(federation.clone()) {
            return Err(unknown_federation_response(&federation_id));
        }
        daemon_state
            .save(&state.paths)
            .map_err(|error| err_response(StatusCode::INTERNAL_SERVER_ERROR, error))?;
    }
    info!(federation_id = %federation.federation_id, members = federation.members.len(), "updated federation");
    Ok(Json(federation))
}

async fn handle_delete_federation(
    headers: HeaderMap,
    AxumPath(federation_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<proto::DeleteFederationResponse>, Response> {
    require_token(&headers, &state.access_token)?;
    let removed = {
        let mut daemon_state = state.state.write().await;
        let removed = daemon_state.remove_federation(&federation_id);
        if removed {
            daemon_state
                .save(&state.paths)
                .map_err(|error| err_response(StatusCode::INTERNAL_SERVER_ERROR, error))?;
        }
        removed
    };
    if !removed {
        return Err(unknown_federation_response(&federation_id));
    }
    info!(federation_id = %federation_id, "deleted federation");
    Ok(Json(proto::DeleteFederationResponse {
        federation_id,
        removed,
    }))
}

/// Resolve a federation to an immutable snapshot: each member's exact commit
/// identity plus its current artifact status. The dashboard uses this to load
/// every discipline independently (via the existing per-project render/IFC
/// endpoints) with no semantic merge.
async fn handle_federation_snapshot(
    headers: HeaderMap,
    AxumPath(federation_id): AxumPath<String>,
    State(state): State<AppState>,
) -> Result<Json<proto::FederationSnapshot>, Response> {
    require_token(&headers, &state.access_token)?;
    let federation = state
        .state
        .read()
        .await
        .federation(&federation_id)
        .ok_or_else(|| unknown_federation_response(&federation_id))?;

    let cfg = state.config.read().await.clone();
    // Cache per project: `None` means the project is not configured locally,
    // `Some(commits)` its (possibly empty) history.
    let mut cache: HashMap<String, Option<Vec<proto::CommitSummary>>> = HashMap::new();
    let mut members = Vec::with_capacity(federation.members.len());
    for member in &federation.members {
        let commits = match cache.get(&member.source_project_id) {
            Some(cached) => cached.clone(),
            None => {
                let probe = snapshot_source_commits(&cfg, &member.source_project_id).await;
                cache.insert(member.source_project_id.clone(), probe.clone());
                probe
            }
        };
        let project_available = commits.is_some();
        let commit_available = commits
            .as_ref()
            .map(|commits| {
                commits
                    .iter()
                    .any(|commit| commit.commit.eq_ignore_ascii_case(&member.commit_hash))
            })
            .unwrap_or(false);
        let render_status = if project_available && commit_available {
            federation_member_render_status(
                &state,
                &cfg,
                &member.source_project_id,
                &member.commit_hash,
            )
            .await
        } else {
            proto::FederationRenderStatus::Unavailable
        };
        members.push(proto::FederationSnapshotMember {
            member_id: member.member_id.clone(),
            source_project_id: member.source_project_id.clone(),
            commit_hash: member.commit_hash.clone(),
            display_name: member.display_name.clone(),
            discipline: member.discipline.clone(),
            transform: member.transform,
            visible: member.visible,
            project_available,
            commit_available,
            render_status,
        });
    }

    Ok(Json(proto::FederationSnapshot {
        schema: proto::schema::FEDERATION_SNAPSHOT.to_string(),
        federation_id: federation.federation_id,
        name: federation.name,
        created_at_unix: federation.created_at_unix,
        updated_at_unix: federation.updated_at_unix,
        members,
    }))
}

/// Best-effort commit probe for a snapshot. Returns `None` when the project is
/// not configured locally, otherwise its history (empty when it has no local
/// repo yet, or when the engine call fails — a snapshot must degrade to
/// "unavailable" for that member rather than failing the whole request).
async fn snapshot_source_commits(
    cfg: &Config,
    project_id: &str,
) -> Option<Vec<proto::CommitSummary>> {
    let entry = find_watch_entry(cfg, project_id)?;
    let dir = PathBuf::from(&entry.path);
    if !is_local_vex_repo(&dir) {
        return Some(Vec::new());
    }
    match vex_cli::log_json(&cfg.vex_bin, &dir).await {
        Ok(log) => Some(commits_from_log(log)),
        Err(_) => Some(Vec::new()),
    }
}

/// Compact render-status hint for a federation member. A validated manifest on
/// disk is the only signal that reports `Ready`; otherwise the durable job
/// state is projected, and a stale `Ready` job without a manifest degrades to
/// `NotRequested` so a viewer falls back to the canonical IFC.
async fn federation_member_render_status(
    state: &AppState,
    cfg: &Config,
    project_id: &str,
    commit: &str,
) -> proto::FederationRenderStatus {
    if federation_render_manifest_present(cfg, project_id, commit).await {
        return proto::FederationRenderStatus::Ready;
    }
    match state
        .state
        .read()
        .await
        .render_artifact_status(project_id, commit)
    {
        Some(crate::state::RenderArtifactJobStatus::Queued) => {
            proto::FederationRenderStatus::Queued
        }
        Some(crate::state::RenderArtifactJobStatus::Building) => {
            proto::FederationRenderStatus::Building
        }
        Some(crate::state::RenderArtifactJobStatus::Failed { .. }) => {
            proto::FederationRenderStatus::Failed
        }
        // A ready job without a validated manifest is not truly ready.
        Some(crate::state::RenderArtifactJobStatus::Ready) | None => {
            proto::FederationRenderStatus::NotRequested
        }
    }
}

async fn federation_render_manifest_present(cfg: &Config, project_id: &str, commit: &str) -> bool {
    let Some(entry) = find_watch_entry(cfg, project_id) else {
        return false;
    };
    let manifest = PathBuf::from(entry.path)
        .join(".vex")
        .join("cache")
        .join("render")
        .join(commit)
        .join("manifest.validated.json");
    tokio::fs::metadata(&manifest)
        .await
        .map(|meta| meta.is_file())
        .unwrap_or(false)
}

async fn handle_repo_push(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<proto::PushRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    require_token(&headers, &state.access_token)?;
    let cloud_project_id = req
        .cloud_project_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            err_response(
                StatusCode::BAD_REQUEST,
                BridgeError::Config("select a cloud project before pushing".to_string()),
            )
        })?;
    uuid::Uuid::parse_str(cloud_project_id).map_err(|_| {
        err_response(
            StatusCode::BAD_REQUEST,
            BridgeError::Config("cloud_project_id must be an Architur repository GUID".to_string()),
        )
    })?;

    let cfg = {
        let cfg = state.config.read().await;
        cfg.watch
            .iter()
            .find(|entry| entry.project_id == req.project_id)
            .ok_or_else(|| unknown_project_response(&req.project_id))?;
        cfg.clone()
    };
    let branch = req.branch.unwrap_or_else(|| "main".to_string());

    match pipeline::run_manual_push(
        &cfg,
        &state.state,
        &state.paths,
        &req.project_id,
        cloud_project_id,
        &branch,
    )
    .await
    {
        Ok(commit_hash) => {
            let mut cfg = state.config.write().await;
            let previous = {
                let entry = cfg
                    .watch
                    .iter_mut()
                    .find(|entry| entry.project_id == req.project_id)
                    .ok_or_else(|| unknown_project_response(&req.project_id))?;
                entry.cloud_project_id.replace(cloud_project_id.to_string())
            };
            if let Err(error) = cfg.save(&state.paths) {
                if let Some(entry) = cfg
                    .watch
                    .iter_mut()
                    .find(|entry| entry.project_id == req.project_id)
                {
                    entry.cloud_project_id = previous;
                }
                return Err(err_response(StatusCode::INTERNAL_SERVER_ERROR, error));
            }
            Ok(Json(serde_json::json!({
                "commit_hash": commit_hash,
                "project_id": req.project_id,
                "cloud_project_id": cloud_project_id,
                "branch": branch,
            })))
        }
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
        cloud_project_id: None,
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
                    let mut updated = entry.clone();
                    updated.cloud_project_id = existing.cloud_project_id.clone();
                    cfg.watch[index] = updated;
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
    let started = Instant::now();
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
    let complete = cache_dir.join(format!("{safe}.ifc.complete"));
    let immutable = safe.len() == 64;
    if immutable && out.is_file() && checkout_cache_is_complete(&out, &complete).await {
        info!(
            reference = %reference,
            elapsed_ms = started.elapsed().as_millis(),
            path = %out.display(),
            "IFC checkout cache hit"
        );
        return Some(out);
    }
    // Caches created before completion markers, or left behind by a crashed
    // checkout, must not be served as immutable historical IFC snapshots.
    // `vex checkout` writes directly to its output path, so materialize into a
    // sibling temporary file and publish only after it completes successfully.
    if let Err(error) = tokio::fs::remove_file(&out).await {
        if error.kind() != std::io::ErrorKind::NotFound {
            warn!(path = %out.display(), %error, "could not remove incomplete IFC checkout cache");
            return None;
        }
    }
    if let Err(error) = tokio::fs::remove_file(&complete).await {
        if error.kind() != std::io::ErrorKind::NotFound {
            warn!(path = %complete.display(), %error, "could not remove IFC checkout completion marker");
            return None;
        }
    }
    let temp = cache_dir.join(format!(".{safe}-{}.ifc.partial", Uuid::now_v7()));
    match vex_cli::checkout(bin, dir, reference, &temp).await {
        Ok(bytes) if bytes > 0 && checkout_temp_has_expected_size(&temp, bytes).await => {
            if let Err(error) = tokio::fs::rename(&temp, &out).await {
                warn!(path = %temp.display(), target = %out.display(), %error, "could not publish IFC checkout cache");
                let _ = tokio::fs::remove_file(&temp).await;
                return None;
            }
            if let Err(error) = tokio::fs::write(&complete, bytes.to_string()).await {
                warn!(path = %complete.display(), %error, "could not write IFC checkout completion marker");
                let _ = tokio::fs::remove_file(&out).await;
                return None;
            }
            info!(
                reference = %reference,
                bytes,
                elapsed_ms = started.elapsed().as_millis(),
                path = %out.display(),
                "IFC checkout materialized"
            );
            Some(out)
        }
        Ok(_) => {
            let _ = tokio::fs::remove_file(&temp).await;
            None
        }
        Err(error) => {
            let _ = tokio::fs::remove_file(&temp).await;
            warn!(
                reference = %reference,
                elapsed_ms = started.elapsed().as_millis(),
                %error,
                "vex checkout failed"
            );
            None
        }
    }
}

fn is_full_commit_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
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

async fn serve_ifc_file(path: PathBuf, immutable: bool) -> Result<Response, Response> {
    let started = Instant::now();
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
    let len = bytes.len();
    let etag = format!("\"{}\"", blake3::hash(&bytes).to_hex());
    let mut response = (
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (
                header::CONTENT_DISPOSITION,
                &format!("inline; filename=\"{}\"", safe_header_value(file_name)),
            ),
        ],
        bytes,
    )
        .into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if immutable {
            "private, max-age=31536000, immutable"
        } else {
            "private, no-cache"
        }),
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&etag).expect("BLAKE3 ETag is a valid header"),
    );
    info!(
        path = %path.display(),
        bytes = len,
        immutable,
        elapsed_ms = started.elapsed().as_millis(),
        "served IFC snapshot"
    );
    Ok(response)
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
                cloud_project_id: watch.cloud_project_id.clone(),
                local_path: watch.path.clone(),
                path_exists: Path::new(&watch.path).is_dir(),
                active: active.contains(&watch.project_id),
                include: watch.include.clone(),
                ifc_project_guid: watch.ifc_project_guid.clone(),
                seen_import_count,
                last_imported_at_unix,
                pending_push_count: state.pending_push_count_for_project(&watch.project_id),
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
    fn viewer_worker_assets_are_embedded() {
        for path in [
            "web-ifc-three/IFCWorker.js",
            "web-ifc/web-ifc.wasm",
            "web-ifc/web-ifc-mt.wasm",
        ] {
            let (bytes, _) = viewer_asset(path).unwrap_or_else(|| panic!("missing {path}"));
            assert!(!bytes.is_empty(), "empty {path}");
        }
    }

    #[test]
    fn hex_lower_formats_bytes() {
        assert_eq!(hex_lower(&[0x00, 0x0f, 0xa3, 0xff]), "000fa3ff");
        assert_eq!(hex_lower(&[]), "");
    }

    #[test]
    fn full_commit_hash_requires_exact_lower_or_upper_hex_length() {
        assert!(is_full_commit_hash(&"a".repeat(64)));
        assert!(is_full_commit_hash(&"A".repeat(64)));
        assert!(!is_full_commit_hash(&"a".repeat(63)));
        assert!(!is_full_commit_hash(&format!("{}g", "a".repeat(63))));
    }

    #[test]
    fn sha256_hex_matches_known_digest() {
        assert_eq!(
            sha256_hex(b"vex-render-artifact"),
            "1165f3b59ba78e92ad6c69757db2bad5cf777df4eaae63aee796832ff34a58f1"
        );
    }

    fn render_resource(
        project_id: &str,
        commit: &str,
        sha256: &str,
    ) -> proto::RenderArtifactResource {
        proto::RenderArtifactResource {
            uri: canonical_render_object_uri(project_id, commit, sha256),
            content_type: "application/octet-stream".into(),
            sha256: sha256.into(),
            byte_length: 1,
        }
    }

    fn v1_render_manifest(project_id: &str, commit: &str) -> proto::RenderArtifactManifest {
        let tile_sha = "a".repeat(64);
        let index_sha = "b".repeat(64);
        proto::RenderArtifactManifest {
            schema: proto::schema::RENDER_MANIFEST.into(),
            project_id: project_id.into(),
            commit_hash: commit.into(),
            artifact_id: "artifact-1".into(),
            generated_at: "2026-07-17T00:00:00Z".into(),
            render_policy: None,
            tiles: vec![proto::RenderTileDescriptor {
                tile_id: "full-model".into(),
                lod: 0,
                group: None,
                bounds: proto::RenderBounds {
                    min: [0.0, 0.0, 0.0],
                    max: [1.0, 1.0, 1.0],
                },
                geometric_error: 0.0,
                artifact: render_resource(project_id, commit, &tile_sha),
                semantic_index: None,
            }],
            semantic_index: Some(proto::RenderSemanticIndexDescriptor {
                schema: proto::schema::RENDER_SEMANTIC_INDEX.into(),
                entry_count: 1,
                artifact: render_resource(project_id, commit, &index_sha),
            }),
        }
    }

    fn v2_render_manifest(project_id: &str, commit: &str) -> proto::RenderArtifactManifest {
        let tile_sha = "c".repeat(64);
        let index_sha = "d".repeat(64);
        proto::RenderArtifactManifest {
            schema: proto::schema::RENDER_MANIFEST_V2.into(),
            project_id: project_id.into(),
            commit_hash: commit.into(),
            artifact_id: "artifact-2".into(),
            generated_at: "2026-07-17T00:00:00Z".into(),
            render_policy: Some(proto::RenderPolicy {
                id: "vex.render-policy.full-model/1".into(),
                hash: "e".repeat(64),
            }),
            tiles: vec![proto::RenderTileDescriptor {
                tile_id: "full-model".into(),
                lod: 0,
                group: None,
                bounds: proto::RenderBounds {
                    min: [0.0, 0.0, 0.0],
                    max: [1.0, 1.0, 1.0],
                },
                geometric_error: 0.0,
                artifact: render_resource(project_id, commit, &tile_sha),
                semantic_index: Some(proto::RenderSemanticIndexDescriptor {
                    schema: proto::schema::RENDER_SEMANTIC_INDEX.into(),
                    entry_count: 1,
                    artifact: render_resource(project_id, commit, &index_sha),
                }),
            }],
            semantic_index: None,
        }
    }

    #[test]
    fn validate_render_resource_uris_accepts_v1_and_v2_manifests() {
        let project = "project-x";
        let commit = "f".repeat(64);

        let v1 = v1_render_manifest(project, &commit);
        assert!(v1.validate().is_ok());
        assert!(validate_render_resource_uris(&v1, project, &commit).is_ok());
        assert_eq!(v1.resources().count(), 2);

        let v2 = v2_render_manifest(project, &commit);
        assert!(v2.validate().is_ok());
        assert!(validate_render_resource_uris(&v2, project, &commit).is_ok());
        // The tile artifact and its tile-local semantic index are both served.
        assert_eq!(v2.resources().count(), 2);
    }

    #[test]
    fn validate_render_resource_uris_rejects_non_canonical_tile_index() {
        // A v2 tile-local semantic index whose URI is not the canonical object
        // endpoint must be rejected so the server never serves a
        // worker-controlled URI.
        let project = "project-x";
        let commit = "f".repeat(64);
        let mut manifest = v2_render_manifest(project, &commit);
        manifest.tiles[0]
            .semantic_index
            .as_mut()
            .unwrap()
            .artifact
            .uri = "/v1/projects/project-x/render/evil/objects/deadbeef".into();

        let error = validate_render_resource_uris(&manifest, project, &commit).unwrap_err();
        assert!(error.contains("canonical object endpoint"));
    }

    #[test]
    fn render_manifest_only_declares_its_own_object_hashes() {
        // The object handler resolves a requested hash strictly against the
        // manifest's declared resources, so an invented hash is never found.
        let project = "project-x";
        let commit = "f".repeat(64);
        let manifest = v2_render_manifest(project, &commit);
        let declared: Vec<_> = manifest.resources().map(|r| r.sha256.clone()).collect();

        assert!(declared.iter().any(|sha| sha == &"c".repeat(64)));
        assert!(declared.iter().any(|sha| sha == &"d".repeat(64)));
        assert!(!manifest
            .resources()
            .any(|resource| resource.sha256 == "9".repeat(64)));
    }

    #[test]
    fn parse_render_byte_range_serves_full_when_absent_or_unusable() {
        // No header, unknown unit, multi-range, and malformed forms all degrade
        // safely to a full-body 200.
        assert_eq!(parse_render_byte_range(None, 100), ByteRange::Full);
        assert_eq!(
            parse_render_byte_range(Some("items=0-10"), 100),
            ByteRange::Full
        );
        assert_eq!(
            parse_render_byte_range(Some("bytes=0-1,3-4"), 100),
            ByteRange::Full
        );
        assert_eq!(
            parse_render_byte_range(Some("bytes="), 100),
            ByteRange::Full
        );
        assert_eq!(
            parse_render_byte_range(Some("bytes=-"), 100),
            ByteRange::Full
        );
        assert_eq!(
            parse_render_byte_range(Some("bytes=abc-def"), 100),
            ByteRange::Full
        );
        // A start greater than the end is invalid syntax: ignore the header.
        assert_eq!(
            parse_render_byte_range(Some("bytes=50-10"), 100),
            ByteRange::Full
        );
    }

    #[test]
    fn parse_render_byte_range_handles_valid_single_ranges() {
        assert_eq!(
            parse_render_byte_range(Some("bytes=0-99"), 100),
            ByteRange::Partial { start: 0, end: 99 }
        );
        assert_eq!(
            parse_render_byte_range(Some("bytes=10-19"), 100),
            ByteRange::Partial { start: 10, end: 19 }
        );
        // Open-ended runs to the last byte.
        assert_eq!(
            parse_render_byte_range(Some("bytes=90-"), 100),
            ByteRange::Partial { start: 90, end: 99 }
        );
        // Suffix selects the final N bytes; an oversized suffix clamps to whole.
        assert_eq!(
            parse_render_byte_range(Some("bytes=-10"), 100),
            ByteRange::Partial { start: 90, end: 99 }
        );
        assert_eq!(
            parse_render_byte_range(Some("bytes=-500"), 100),
            ByteRange::Partial { start: 0, end: 99 }
        );
    }

    #[test]
    fn parse_render_byte_range_clamps_end_to_last_byte() {
        // An end past the object length is valid and clamps to the final byte.
        assert_eq!(
            parse_render_byte_range(Some("bytes=50-9999"), 100),
            ByteRange::Partial { start: 50, end: 99 }
        );
    }

    #[test]
    fn parse_render_byte_range_reports_unsatisfiable() {
        // Start at or beyond the length, a zero-length suffix, and any range
        // against empty content are unsatisfiable (416).
        assert_eq!(
            parse_render_byte_range(Some("bytes=100-200"), 100),
            ByteRange::Unsatisfiable
        );
        assert_eq!(
            parse_render_byte_range(Some("bytes=100-"), 100),
            ByteRange::Unsatisfiable
        );
        assert_eq!(
            parse_render_byte_range(Some("bytes=-0"), 100),
            ByteRange::Unsatisfiable
        );
        assert_eq!(
            parse_render_byte_range(Some("bytes=0-0"), 0),
            ByteRange::Unsatisfiable
        );
    }

    #[test]
    fn render_object_full_response_advertises_ranges_and_etag() {
        let sha = "a".repeat(64);
        let response = render_object_response(
            "model/gltf-binary",
            &sha,
            b"full-object-bytes".to_vec(),
            ByteRange::Full,
        );
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers();
        assert_eq!(headers.get(header::ACCEPT_RANGES).unwrap(), "bytes");
        assert_eq!(headers.get(header::ETAG).unwrap(), &format!("\"{sha}\""));
        assert!(headers
            .get(header::CACHE_CONTROL)
            .unwrap()
            .to_str()
            .unwrap()
            .contains("immutable"));
        assert!(headers.get(header::CONTENT_RANGE).is_none());
    }

    #[test]
    fn render_object_unsatisfiable_response_is_416_with_content_range() {
        let sha = "b".repeat(64);
        let response = render_object_response(
            "application/octet-stream",
            &sha,
            vec![0u8; 42],
            ByteRange::Unsatisfiable,
        );
        assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes */42"
        );
        assert_eq!(
            response.headers().get(header::ACCEPT_RANGES).unwrap(),
            "bytes"
        );
    }

    #[tokio::test]
    async fn render_object_partial_response_returns_slice_and_content_range() {
        let sha = "c".repeat(64);
        let body = (0u8..100).collect::<Vec<u8>>();
        let response = render_object_response(
            "application/octet-stream",
            &sha,
            body,
            ByteRange::Partial { start: 10, end: 19 },
        );
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes 10-19/100"
        );
        assert_eq!(
            response.headers().get(header::ETAG).unwrap(),
            &format!("\"{sha}\"")
        );
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(bytes.len(), 10);
        assert_eq!(&bytes[..], &(10u8..20).collect::<Vec<u8>>()[..]);
    }

    #[test]
    fn parse_sha256sums_matches_asset() {
        let manifest = concat!(
            "aaaa1111  vex-bridge-v0.2.38-windows-x86_64.tar.gz\n",
            "BBBB2222  VexAtlasSetup-v0.2.38-windows-x86_64.exe\n",
            "cccc3333  early-access-distribution.md\n",
        );
        assert_eq!(
            parse_sha256sums(manifest, "VexAtlasSetup-v0.2.38-windows-x86_64.exe"),
            Some("bbbb2222".to_string())
        );
        assert_eq!(parse_sha256sums(manifest, "does-not-exist.exe"), None);
    }

    #[test]
    fn parse_sha256sums_handles_star_and_path_prefixes() {
        let manifest = "dddd4444 *dist/VexAtlasSetup-v0.2.38-windows-x86_64.exe\n";
        assert_eq!(
            parse_sha256sums(manifest, "VexAtlasSetup-v0.2.38-windows-x86_64.exe"),
            Some("dddd4444".to_string())
        );
    }

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

    #[test]
    fn repo_locked_signature_is_detected() {
        assert!(is_repo_locked(
            "Database already open. Cannot acquire lock."
        ));
        assert!(is_repo_locked("error: CANNOT ACQUIRE LOCK on objects.redb"));
        assert!(!is_repo_locked("some unrelated engine failure"));
    }

    #[test]
    fn repo_locked_error_classifies_as_retryable_repair() {
        let err = BridgeError::VexCli("Database already open. Cannot acquire lock.".into());
        let (code, hint, retryable) = classify_error(StatusCode::INTERNAL_SERVER_ERROR, &err);
        assert_eq!(code, "repo_locked");
        assert!(retryable);
        assert!(hint.unwrap().contains("Repair"));
    }

    #[test]
    fn generic_vex_cli_error_is_not_repo_locked() {
        let err = BridgeError::VexCli("vex import failed: bad ifc".into());
        let (code, _hint, _retryable) = classify_error(StatusCode::INTERNAL_SERVER_ERROR, &err);
        assert_eq!(code, "vex_cli_error");
    }

    #[test]
    fn rejected_pairing_credentials_are_not_reported_as_a_network_failure() {
        let (code, hint, retryable) = classify_error(
            StatusCode::UNAUTHORIZED,
            &BridgeError::PairingCredentialsRejected,
        );
        assert_eq!(code, "pairing_credentials_rejected");
        assert!(!retryable);
        assert!(hint.unwrap().contains("not a browser sign-in"));
    }

    #[test]
    fn incomplete_pairing_record_is_not_reported_as_a_project_conflict() {
        let (code, _hint, retryable) =
            classify_error(StatusCode::CONFLICT, &BridgeError::PairingKeyUnavailable);
        assert_eq!(code, "pairing_key_unavailable");
        assert!(!retryable);
    }

    #[test]
    fn pairing_with_an_unavailable_local_key_is_reported_as_unpaired() {
        let state = DaemonState {
            pairing: PairingState::Paired {
                device_label: "Vex Atlas".into(),
                key_fingerprint: "SHA256:expected".into(),
                key_id: "2cdaa28b-7185-4380-ba7f-f34bd0b819cf".into(),
                paired_at_unix: 0,
                account_id: None,
                account_email: None,
                account_name: None,
            },
            ..DaemonState::default()
        };

        assert!(matches!(
            pair_status_from_pairing(&state, false),
            proto::PairStatus::Unpaired
        ));
    }

    #[test]
    fn unavailable_cloud_projects_are_reported_as_retryable() {
        let (code, _hint, retryable) = classify_error(
            StatusCode::BAD_GATEWAY,
            &BridgeError::CloudProjectsUnavailable("HTTP 503".into()),
        );
        assert_eq!(code, "cloud_projects_unavailable");
        assert!(retryable);
    }

    #[test]
    fn redact_replaces_token_and_home() {
        let token = "supersecrettoken1234";
        let text = format!("token={token} path=/home/me/file");
        let out = redact(&text, token);
        assert!(!out.contains(token));
        assert!(out.contains("[redacted-token]"));
    }

    // -----------------------------------------------------------------------
    // Federation resolution: pure, engine-free unit tests.
    // -----------------------------------------------------------------------

    fn commit(hash: &str) -> proto::CommitSummary {
        proto::CommitSummary {
            commit: hash.to_string(),
            author: "Author".into(),
            email: "author@example.com".into(),
            timestamp: 0,
            message: "commit".into(),
            parents: Vec::new(),
        }
    }

    #[test]
    fn resolve_member_commit_pins_head_when_no_hash_is_given() {
        // Newest-first history: HEAD is the first entry, and that exact string
        // is what gets pinned.
        let head = "a".repeat(64);
        let older = "b".repeat(64);
        let commits = vec![commit(&head), commit(&older)];
        assert_eq!(resolve_member_commit(&commits, None).unwrap(), head);
    }

    #[test]
    fn resolve_member_commit_accepts_an_explicit_hash_in_history() {
        let head = "a".repeat(64);
        let older = "b".repeat(64);
        let commits = vec![commit(&head), commit(&older)];
        // A case-insensitive match resolves to the canonical stored hash.
        assert_eq!(
            resolve_member_commit(&commits, Some(&older.to_uppercase())).unwrap(),
            older
        );
    }

    #[test]
    fn resolve_member_commit_rejects_a_non_full_hash() {
        let commits = vec![commit(&"a".repeat(64))];
        let short = "abc123";
        assert_eq!(
            resolve_member_commit(&commits, Some(short)),
            Err(CommitResolveError::InvalidHash(short.to_string()))
        );
    }

    #[test]
    fn resolve_member_commit_rejects_a_hash_absent_from_history() {
        let commits = vec![commit(&"a".repeat(64))];
        let ghost = "c".repeat(64);
        assert_eq!(
            resolve_member_commit(&commits, Some(&ghost)),
            Err(CommitResolveError::NotInHistory(ghost))
        );
    }

    #[test]
    fn resolve_member_commit_reports_no_commits_to_pin() {
        assert_eq!(
            resolve_member_commit(&[], None),
            Err(CommitResolveError::NoCommits)
        );
    }

    // -----------------------------------------------------------------------
    // Federation HTTP routes: end-to-end over a real localhost listener.
    // These never invoke the vex engine — every path here is reachable with
    // list/get/delete, deletion-safety, input validation, and best-effort
    // snapshot, none of which shell out.
    // -----------------------------------------------------------------------

    const TEST_TOKEN: &str = "federation-test-token";

    fn test_paths() -> (Paths, PathBuf) {
        let root = std::env::temp_dir().join(format!("vex-bridge-fed-test-{}", Uuid::now_v7()));
        let config_dir = root.join("config");
        let data_dir = root.join("data");
        let paths = Paths {
            config_file: config_dir.join("config.toml"),
            access_token_file: config_dir.join("access-token"),
            state_file: data_dir.join("state.json"),
            log_file: data_dir.join("bridge.log"),
            daemon_lock_file: data_dir.join("daemon.lock"),
            config_dir,
            data_dir,
        };
        (paths, root)
    }

    fn watch_entry(project_id: &str, path: &Path) -> crate::config::WatchEntry {
        crate::config::WatchEntry {
            project_id: project_id.to_string(),
            cloud_project_id: None,
            path: path.to_string_lossy().into_owned(),
            include: vec!["*.ifc".to_string()],
            ifc_project_guid: None,
            project_name: None,
        }
    }

    fn seeded_federation(
        id: &str,
        name: &str,
        source: &str,
        commit_hash: &str,
    ) -> proto::Federation {
        proto::Federation {
            schema: proto::schema::FEDERATION.to_string(),
            federation_id: id.to_string(),
            name: name.to_string(),
            created_at_unix: 100,
            updated_at_unix: 100,
            members: vec![proto::FederationMember {
                member_id: format!("mem-{source}"),
                source_project_id: source.to_string(),
                commit_hash: commit_hash.to_string(),
                display_name: format!("{source} model"),
                discipline: Some("architecture".to_string()),
                transform: proto::FederationTransform::identity(),
                visible: true,
            }],
        }
    }

    struct TestServer {
        base: String,
        client: reqwest::Client,
        root: PathBuf,
    }

    impl TestServer {
        async fn start(config: Config, state: DaemonState) -> Self {
            let (paths, root) = test_paths();
            paths.ensure_dirs().expect("create test dirs");
            let app = AppState {
                config: Arc::new(RwLock::new(config)),
                state: Arc::new(RwLock::new(state)),
                paths: Arc::new(paths),
                access_token: Arc::new(TEST_TOKEN.to_string()),
                started_at: Instant::now(),
                watchers: Arc::new(RwLock::new(Vec::new())),
                federation_project_lock: Arc::new(Mutex::new(())),
                shutdown: Arc::new(tokio::sync::Notify::new()),
                update_cache: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            };
            let listener = bind(0).await.expect("bind ephemeral port");
            let addr = listener.local_addr().expect("local addr");
            tokio::spawn(async move {
                let _ = serve(app, listener).await;
            });
            Self {
                base: format!("http://{addr}"),
                client: reqwest::Client::new(),
                root,
            }
        }

        fn get(&self, path: &str) -> reqwest::RequestBuilder {
            self.client
                .get(format!("{}{path}", self.base))
                .header("X-Vex-Bridge-Token", TEST_TOKEN)
        }

        fn post(&self, path: &str) -> reqwest::RequestBuilder {
            self.client
                .post(format!("{}{path}", self.base))
                .header("X-Vex-Bridge-Token", TEST_TOKEN)
        }

        fn patch(&self, path: &str) -> reqwest::RequestBuilder {
            self.client
                .patch(format!("{}{path}", self.base))
                .header("X-Vex-Bridge-Token", TEST_TOKEN)
        }

        fn delete(&self, path: &str) -> reqwest::RequestBuilder {
            self.client
                .delete(format!("{}{path}", self.base))
                .header("X-Vex-Bridge-Token", TEST_TOKEN)
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[tokio::test]
    async fn federation_routes_reject_a_missing_or_wrong_token() {
        let server = TestServer::start(Config::default(), DaemonState::default()).await;
        // No token at all.
        let unauth = server
            .client
            .get(format!("{}/v1/federations", server.base))
            .send()
            .await
            .unwrap();
        assert_eq!(unauth.status(), reqwest::StatusCode::UNAUTHORIZED);
        // Wrong token.
        let wrong = server
            .client
            .get(format!("{}/v1/federations", server.base))
            .header("X-Vex-Bridge-Token", "nope")
            .send()
            .await
            .unwrap();
        assert_eq!(wrong.status(), reqwest::StatusCode::UNAUTHORIZED);
        // Correct token is accepted.
        let ok = server.get("/v1/federations").send().await.unwrap();
        assert_eq!(ok.status(), reqwest::StatusCode::OK);
    }

    #[tokio::test]
    async fn federation_list_get_and_delete_round_trip() {
        let mut state = DaemonState::default();
        state.insert_federation(seeded_federation(
            "fed-1",
            "Tower A",
            "arch",
            &"a".repeat(64),
        ));
        state.insert_federation(seeded_federation(
            "fed-2",
            "Tower B",
            "struct",
            &"b".repeat(64),
        ));
        let server = TestServer::start(Config::default(), state).await;

        // List is newest-first with accurate summaries.
        let summaries: Vec<proto::FederationSummary> = server
            .get("/v1/federations")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let ids: Vec<&str> = summaries
            .iter()
            .map(|summary| summary.federation_id.as_str())
            .collect();
        assert_eq!(ids, vec!["fed-2", "fed-1"]);
        assert_eq!(summaries[0].member_count, 1);
        assert_eq!(summaries[0].visible_member_count, 1);

        // Fetch one full federation, including its exact commit identity.
        let federation: proto::Federation = server
            .get("/v1/federations/fed-1")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(federation.members[0].commit_hash, "a".repeat(64));
        assert_eq!(federation.members[0].source_project_id, "arch");

        // Delete it, then confirm it is gone.
        let deleted: proto::DeleteFederationResponse = server
            .delete("/v1/federations/fed-1")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(deleted.removed);
        let after = server.get("/v1/federations/fed-1").send().await.unwrap();
        assert_eq!(after.status(), reqwest::StatusCode::NOT_FOUND);
        let remaining: Vec<proto::FederationSummary> = server
            .get("/v1/federations")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
    }

    #[tokio::test]
    async fn get_unknown_or_unsafe_federation_id_is_not_found() {
        let server = TestServer::start(Config::default(), DaemonState::default()).await;
        // A plausible-but-absent id.
        let missing = server
            .get("/v1/federations/fed-ghost")
            .send()
            .await
            .unwrap();
        assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);
        // A traversal-shaped id is treated as an opaque lookup key, never a
        // path: it simply does not match any stored federation.
        let traversal = server
            .get("/v1/federations/..%2f..%2fsecret")
            .send()
            .await
            .unwrap();
        assert!(
            traversal.status() == reqwest::StatusCode::NOT_FOUND
                || traversal.status() == reqwest::StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn create_federation_rejects_empty_name_or_members() {
        let server = TestServer::start(Config::default(), DaemonState::default()).await;

        let empty_name = server
            .post("/v1/federations")
            .json(&serde_json::json!({ "name": "   ", "members": [] }))
            .send()
            .await
            .unwrap();
        assert_eq!(empty_name.status(), reqwest::StatusCode::BAD_REQUEST);

        let no_members = server
            .post("/v1/federations")
            .json(&serde_json::json!({ "name": "Coordination", "members": [] }))
            .send()
            .await
            .unwrap();
        assert_eq!(no_members.status(), reqwest::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_federation_referencing_an_unknown_project_is_not_found() {
        let server = TestServer::start(Config::default(), DaemonState::default()).await;
        let response = server
            .post("/v1/federations")
            .json(&serde_json::json!({
                "name": "Coordination",
                "members": [{
                    "source_project_id": "not-registered",
                    "display_name": "Architecture"
                }]
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_federation_with_a_hash_absent_from_history_is_bad_request() {
        // A project is configured locally but is not a vex repo yet, so its
        // history is empty. An explicit hash therefore cannot be in history and
        // create must fail with 400 — no engine call is needed to prove this.
        let (_paths, root) = test_paths();
        let project_dir = root.join("arch-project");
        std::fs::create_dir_all(&project_dir).unwrap();
        let mut config = Config::default();
        config.watch.push(watch_entry("arch", &project_dir));
        let server = TestServer::start(config, DaemonState::default()).await;

        let response = server
            .post("/v1/federations")
            .json(&serde_json::json!({
                "name": "Coordination",
                "members": [{
                    "source_project_id": "arch",
                    "commit_hash": "a".repeat(64),
                    "display_name": "Architecture"
                }]
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn deleting_a_project_referenced_by_a_federation_conflicts() {
        let (_paths, root) = test_paths();
        let project_dir = root.join("arch-project");
        std::fs::create_dir_all(&project_dir).unwrap();
        let mut config = Config::default();
        config.watch.push(watch_entry("arch", &project_dir));
        let mut state = DaemonState::default();
        state.insert_federation(seeded_federation("fed-1", "Tower", "arch", &"a".repeat(64)));
        let server = TestServer::start(config, state).await;

        let response = server
            .delete("/v1/projects/arch")
            .json(&serde_json::json!({ "deletion_policy": "keep_folder" }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
        let body: proto::ApiError = response.json().await.unwrap();
        assert_eq!(
            body.code.as_deref(),
            Some("project_referenced_by_federation")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn snapshot_degrades_when_the_source_project_is_missing() {
        let mut state = DaemonState::default();
        state.insert_federation(seeded_federation("fed-1", "Tower", "arch", &"a".repeat(64)));
        let server = TestServer::start(Config::default(), state).await;

        let snapshot: proto::FederationSnapshot = server
            .get("/v1/federations/fed-1/snapshot")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(snapshot.federation_id, "fed-1");
        assert_eq!(snapshot.members.len(), 1);
        let member = &snapshot.members[0];
        // The pinned identity is preserved exactly even when unresolved.
        assert_eq!(member.commit_hash, "a".repeat(64));
        assert!(!member.project_available);
        assert!(!member.commit_available);
        assert_eq!(
            member.render_status,
            proto::FederationRenderStatus::Unavailable
        );
    }

    #[tokio::test]
    async fn update_federation_renames_without_touching_members() {
        let mut state = DaemonState::default();
        state.insert_federation(seeded_federation(
            "fed-1",
            "Old Name",
            "arch",
            &"a".repeat(64),
        ));
        let server = TestServer::start(Config::default(), state).await;

        let updated: proto::Federation = server
            .patch("/v1/federations/fed-1")
            .json(&serde_json::json!({ "name": "New Name" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(updated.name, "New Name");
        // The member set — and its exact commit identity — is untouched.
        assert_eq!(updated.members.len(), 1);
        assert_eq!(updated.members[0].commit_hash, "a".repeat(64));
        assert!(updated.updated_at_unix >= updated.created_at_unix);
    }
}
