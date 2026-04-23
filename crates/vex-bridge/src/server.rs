//! Local HTTP server that plugins call. Binds to 127.0.0.1 only.
//!
//! Auth model: every request must carry `X-Vex-Bridge-Token` matching the
//! contents of `<config_dir>/access-token`. The token is generated once on
//! daemon start with `mode 0600`, so:
//!   * a *cooperating* process running as the same user can read it;
//!   * a malicious webpage in the user's browser cannot (cross-origin reads
//!     of `localhost` are blocked by CORS, which we do NOT relax);
//!   * a different OS user cannot.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use tokio::sync::RwLock;
use tracing::{info, warn};

use vex_bridge_protocol as proto;

use crate::config::{Config, Paths};
use crate::errors::BridgeError;
use crate::pairing;
use crate::pipeline;
use crate::state::{now_unix, PairingState, State as DaemonState};
use crate::vex_cli;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub state: Arc<RwLock<DaemonState>>,
    pub paths: Arc<Paths>,
    pub access_token: Arc<String>,
    pub started_at: Instant,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(handle_health))
        .route("/v1/pair/status", get(handle_pair_status))
        .route("/v1/pair/start", post(handle_pair_start))
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

// ─── Helpers ─────────────────────────────────────────────────────────────

#[allow(clippy::result_large_err)]
fn require_token(headers: &HeaderMap, expected: &str) -> Result<(), Response> {
    match headers
        .get("x-vex-bridge-token")
        .and_then(|v| v.to_str().ok())
    {
        Some(t) if constant_time_eq(t.as_bytes(), expected.as_bytes()) => Ok(()),
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

fn err_response(status: StatusCode, e: BridgeError) -> Response {
    warn!(error = %e, "request failed");
    (
        status,
        Json(proto::ApiError {
            error: format!("{:?}", e)
                .split_whitespace()
                .next()
                .unwrap_or("error")
                .to_lowercase(),
            message: e.to_string(),
        }),
    )
        .into_response()
}

// ─── Handlers ────────────────────────────────────────────────────────────

async fn handle_health(State(s): State<AppState>) -> Json<proto::Health> {
    let cfg = s.config.read().await.clone();
    let state = s.state.read().await.clone();
    let vex_ver = vex_cli::version(&cfg.vex_bin).await.ok().flatten();
    Json(proto::Health {
        version: env!("CARGO_PKG_VERSION").to_string(),
        paired: matches!(state.pairing, PairingState::Paired { .. }),
        vex_bin: cfg.vex_bin,
        vex_version: vex_ver,
        uptime_seconds: s.started_at.elapsed().as_secs(),
    })
}

async fn handle_pair_status(
    headers: HeaderMap,
    State(s): State<AppState>,
) -> Result<Json<proto::PairStatus>, Response> {
    require_token(&headers, &s.access_token)?;
    let state = s.state.read().await.clone();
    let body = match state.pairing {
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
    State(s): State<AppState>,
    Json(req): Json<proto::PairStartRequest>,
) -> Result<Json<proto::PairStartResponse>, Response> {
    require_token(&headers, &s.access_token)?;
    let cfg = s.config.read().await.clone();
    let outcome = pairing::start(&cfg, &req.device_label)
        .await
        .map_err(|e| err_response(StatusCode::BAD_GATEWAY, e))?;

    // Persist the pending pairing so subsequent /pair/status calls see it.
    {
        let mut state = s.state.write().await;
        state.pairing = PairingState::Pending {
            code: outcome.code.clone(),
            pair_url: outcome.pair_url.clone(),
            expires_at_unix: now_unix() + 600,
            device_label: req.device_label.clone(),
        };
        state
            .save(&s.paths)
            .map_err(|e| err_response(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }

    Ok(Json(proto::PairStartResponse {
        code: outcome.code,
        pair_url: outcome.pair_url,
        expires_at: outcome.expires_at,
    }))
}

fn rfc3339_from_unix(ts: i64) -> String {
    // Tiny formatter to avoid pulling in chrono just for this. Format: UTC Z.
    use std::time::{Duration, UNIX_EPOCH};
    let dt = UNIX_EPOCH + Duration::from_secs(ts.max(0) as u64);
    let secs = dt
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, min, sec) = unix_to_civil(secs as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Howard Hinnant's civil-from-days. Pure integer math, no dependency.
fn unix_to_civil(secs: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let time = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (
        year,
        m,
        d,
        (time / 3600) as u32,
        ((time % 3600) / 60) as u32,
        (time % 60) as u32,
    )
}

/// `POST /v1/repo/push`
///
/// Manually triggers the same add → commit → push pipeline that the file
/// watcher would normally run. The Revit/Rhino plug-ins call this when
/// the architect clicks "Push to architur"; the project_id maps to a
/// configured `[[watch]]` entry whose `path` is the local repo dir.
async fn handle_repo_push(
    headers: HeaderMap,
    State(s): State<AppState>,
    Json(req): Json<proto::PushRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    require_token(&headers, &s.access_token)?;
    let cfg = s.config.read().await.clone();
    let branch = req.branch.unwrap_or_else(|| "main".to_string());

    match pipeline::run_manual_push(&cfg, &req.project_id, &branch).await {
        Ok(commit_hash) => Ok(Json(serde_json::json!({
            "commit_hash": commit_hash,
            "project_id":  req.project_id,
            "branch":      branch,
        }))),
        Err(e) => {
            // 404 if the project isn't configured, 502 for vex CLI failures.
            let status = match &e {
                BridgeError::Config(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::BAD_GATEWAY,
            };
            Err(err_response(status, e))
        }
    }
}

/// `POST /v1/repo/register`
///
/// Idempotently register or update the watch entry that maps an architur
/// `project_id` to a local directory. Persists to `config.toml` so the
/// next daemon restart will pick up the watcher automatically; the
/// `manual push` path (POST /v1/repo/push) starts working immediately
/// without a restart because it reads `cfg.watch` on every call.
async fn handle_repo_register(
    headers: HeaderMap,
    State(s): State<AppState>,
    Json(req): Json<proto::RepoRegisterRequest>,
) -> Result<Json<proto::RepoRegisterResponse>, Response> {
    require_token(&headers, &s.access_token)?;

    if req.project_id.trim().is_empty() {
        return Err(err_response(
            StatusCode::BAD_REQUEST,
            BridgeError::Config("project_id must not be empty".into()),
        ));
    }

    // Resolve the on-disk path. If the caller didn't pin one, default to
    // `<home>/Architur/<project_id>` — under the user's home so it
    // survives uninstall and shows up where designers expect their files.
    let local_path = match req.local_path {
        Some(p) if !p.trim().is_empty() => {
            let candidate = std::path::PathBuf::from(p.trim());
            if !candidate.is_absolute()
                || candidate
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return Err(err_response(
                    StatusCode::BAD_REQUEST,
                    BridgeError::Config(
                        "local_path must be an absolute path without '..'".into(),
                    ),
                ));
            }

            let base = candidate
                .parent()
                .unwrap_or(candidate.as_path())
                .canonicalize()
                .map_err(|_| {
                    err_response(
                        StatusCode::BAD_REQUEST,
                        BridgeError::Config(
                            "local_path must have an existing parent directory".into(),
                        ),
                    )
                })?;
            let resolved = base.join(
                candidate
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("")),
            );
            if !resolved.starts_with(&base) {
                return Err(err_response(
                    StatusCode::BAD_REQUEST,
                    BridgeError::Config("invalid local_path".into()),
                ));
            }
            resolved
        }
        _ => {
            let home = directories::UserDirs::new()
                .map(|d| d.home_dir().to_path_buf())
                .ok_or_else(|| {
                    err_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        BridgeError::Config("could not resolve user home".into()),
                    )
                })?;
            home.join("Architur").join(&req.project_id)
        }
    };

    if let Err(e) = std::fs::create_dir_all(&local_path) {
        return Err(err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            BridgeError::Io(e),
        ));
    }

    let include = req
        .include
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| vec!["*.ifc".to_string()]);

    let entry = crate::config::WatchEntry {
        project_id: req.project_id.clone(),
        path: local_path.to_string_lossy().to_string(),
        include: include.clone(),
    };

    // Mutate, persist, and report whether we replaced an existing entry.
    let replaced = {
        let mut cfg = s.config.write().await;
        let prev = cfg
            .watch
            .iter()
            .position(|w| w.project_id == req.project_id);
        match prev {
            Some(i) => {
                cfg.watch[i] = entry.clone();
                true
            }
            None => {
                cfg.watch.push(entry.clone());
                false
            }
        }
    };

    {
        let cfg = s.config.read().await;
        if let Err(e) = cfg.save(&s.paths) {
            return Err(err_response(StatusCode::INTERNAL_SERVER_ERROR, e));
        }
    }

    info!(
        project_id = %req.project_id,
        path = %local_path.display(),
        replaced,
        "registered project"
    );

    Ok(Json(proto::RepoRegisterResponse {
        project_id: req.project_id,
        local_path: local_path.to_string_lossy().to_string(),
        include,
        replaced,
    }))
}
