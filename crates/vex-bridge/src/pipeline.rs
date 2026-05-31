//! Glue: watch entries from config → IFC import+commit+push pipeline.
//!
//! For each `[[watch]]` block in `config.toml`, we spawn a debounced watcher
//! (see [`crate::watcher`]) on the directory. Whenever an IFC file settles, we
//! hash it, ask `vex` for project/header metadata, run `vex import`, commit,
//! push, and archive the processed export. If the directory isn't a vex repo yet
//! we initialize it and register the project's remote so the first IFC upload
//! works.
//!
//! Failures are logged but never crash the daemon — a misconfigured watch
//! must not take down the bridge for other projects.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::runtime::Handle;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};
use vex_bridge_protocol as proto;

use crate::config::{Config, Paths, WatchEntry};
use crate::errors::BridgeResult;
use crate::ifc::{hash_file, parse_intake, IfcIntake};
use crate::state::State;
use crate::vex_cli;
use crate::watcher::{self, WatchHandle};

const FILE_STABLE_INTERVAL: Duration = Duration::from_millis(500);
const FILE_STABLE_ATTEMPTS: usize = 10;

/// One pipeline per configured watch. Held by the daemon for the lifetime
/// of the process; dropping the inner handle stops the watcher.
pub struct WatchPipeline {
    pub project_id: String,
    pub path: PathBuf,
    _handle: WatchHandle,
}

struct PipelineRun {
    bin: String,
    dir: PathBuf,
    api_base: String,
    entry: WatchEntry,
    changed: PathBuf,
    author_name: Option<String>,
    author_email: Option<String>,
    state: Arc<RwLock<State>>,
    paths: Arc<Paths>,
}

struct ActivityDetails {
    kind: proto::ActivityKind,
    commit_hash: Option<String>,
    content_hash: Option<String>,
    message: String,
    detail: Option<String>,
}

/// Spin up one debounced watcher per `[[watch]]` entry. Returns the live
/// handles so the caller can keep them alive.
pub fn spawn_all(
    cfg: &Config,
    runtime: Handle,
    state: Arc<RwLock<State>>,
    paths: Arc<Paths>,
) -> Vec<WatchPipeline> {
    let mut out = Vec::new();
    for entry in &cfg.watch {
        match spawn_entry(
            cfg,
            runtime.clone(),
            entry.clone(),
            state.clone(),
            paths.clone(),
        ) {
            Ok(p) => out.push(p),
            Err(e) => {
                warn!(project = %entry.project_id, path = %entry.path, error = %e, "watch setup failed")
            }
        }
    }
    info!(count = out.len(), "watchers active");
    out
}

pub fn spawn_entry(
    cfg: &Config,
    runtime: Handle,
    entry: WatchEntry,
    state: Arc<RwLock<State>>,
    paths: Arc<Paths>,
) -> BridgeResult<WatchPipeline> {
    let dir = PathBuf::from(&entry.path);
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    let bin = cfg.vex_bin.clone();
    let api_base = cfg.api_base.clone();
    let author_name = cfg.default_author_name.clone();
    let author_email = cfg.default_author_email.clone();

    // Coalesce concurrent change events on the same repo into one in-flight
    // add/commit/push attempt. notify can fire many events per save.
    let lock: Arc<Mutex<()>> = Arc::new(Mutex::new(()));

    let dir_for_cb = dir.clone();
    let entry_for_cb = entry.clone();
    let runtime_for_cb = runtime.clone();
    let lock_for_cb = lock.clone();
    let bin_for_cb = bin.clone();
    let api_base_for_cb = api_base.clone();
    let author_name_for_cb = author_name.clone();
    let author_email_for_cb = author_email.clone();
    let state_for_cb = state.clone();
    let paths_for_cb = paths.clone();
    let handle = watcher::spawn(entry.clone(), move |changed| {
        if !is_ifc_candidate(&changed) || !matches_include(&changed, &entry_for_cb.include) {
            return;
        }
        let runtime = runtime_for_cb.clone();
        let lock = lock_for_cb.clone();
        let bin = bin_for_cb.clone();
        let api_base = api_base_for_cb.clone();
        let entry = entry_for_cb.clone();
        let project_id = entry.project_id.clone();
        let dir = dir_for_cb.clone();
        let author_name = author_name_for_cb.clone();
        let author_email = author_email_for_cb.clone();
        let state = state_for_cb.clone();
        let paths = paths_for_cb.clone();
        runtime.spawn(async move {
            let _g = lock.lock().await;
            if let Err(e) = run_pipeline(PipelineRun {
                bin: bin.clone(),
                dir: dir.clone(),
                api_base: api_base.clone(),
                entry: entry.clone(),
                changed: changed.clone(),
                author_name: author_name.clone(),
                author_email: author_email.clone(),
                state: state.clone(),
                paths: paths.clone(),
            })
            .await
            {
                record_activity(
                    &state,
                    &paths,
                    activity_event(
                        &entry,
                        &dir,
                        &changed,
                        ActivityDetails {
                            kind: proto::ActivityKind::Error,
                            commit_hash: None,
                            content_hash: None,
                            message: format!("Could not process {}", file_label(&changed)),
                            detail: Some(e.to_string()),
                        },
                    ),
                )
                .await;
                error!(error = %e, project = %project_id, "push pipeline failed");
            }
        });
    })?;

    let scan_lock = lock.clone();
    let scan_bin = cfg.vex_bin.clone();
    let scan_api_base = cfg.api_base.clone();
    let scan_entry = entry.clone();
    let scan_dir = dir.clone();
    let scan_author_name = cfg.default_author_name.clone();
    let scan_author_email = cfg.default_author_email.clone();
    let scan_state = state.clone();
    let scan_paths = paths.clone();
    runtime.spawn(async move {
        let _g = scan_lock.lock().await;
        let files = match existing_ifc_files(&scan_dir) {
            Ok(files) => files,
            Err(error) => {
                warn!(path = %scan_dir.display(), error = %error, "could not scan existing IFC files");
                return;
            }
        };
        for changed in files {
            if !matches_include(&changed, &scan_entry.include) {
                continue;
            }
            if let Err(error) = run_pipeline(PipelineRun {
                bin: scan_bin.clone(),
                dir: scan_dir.clone(),
                api_base: scan_api_base.clone(),
                entry: scan_entry.clone(),
                changed: changed.clone(),
                author_name: scan_author_name.clone(),
                author_email: scan_author_email.clone(),
                state: scan_state.clone(),
                paths: scan_paths.clone(),
            })
            .await
            {
                record_activity(
                    &scan_state,
                    &scan_paths,
                    activity_event(
                        &scan_entry,
                        &scan_dir,
                        &changed,
                        ActivityDetails {
                            kind: proto::ActivityKind::Error,
                            commit_hash: None,
                            content_hash: None,
                            message: format!("Could not process {}", file_label(&changed)),
                            detail: Some(error.to_string()),
                        },
                    ),
                )
                .await;
                error!(error = %error, project = %scan_entry.project_id, "inbox scan failed");
            }
        }
    });

    Ok(WatchPipeline {
        project_id: entry.project_id,
        path: dir,
        _handle: handle,
    })
}

async fn run_pipeline(run: PipelineRun) -> BridgeResult<()> {
    let PipelineRun {
        bin,
        dir,
        api_base,
        entry,
        changed,
        author_name,
        author_email,
        state,
        paths,
    } = run;
    let bin = bin.as_str();
    let dir = dir.as_path();
    let api_base = api_base.as_str();
    let changed = changed.as_path();
    let author_name = author_name.as_deref();
    let author_email = author_email.as_deref();
    let entry = &entry;

    wait_for_stable_file(changed).await?;

    record_activity(
        &state,
        &paths,
        activity_event(
            entry,
            dir,
            changed,
            ActivityDetails {
                kind: proto::ActivityKind::ProcessingStarted,
                commit_hash: None,
                content_hash: None,
                message: format!("Processing {}", file_label(changed)),
                detail: None,
            },
        ),
    )
    .await;

    let content_hash = hash_file_async(changed).await?;
    if state.read().await.has_seen_ifc_hash(&content_hash) {
        info!(file = %changed.display(), hash = %content_hash, "duplicate IFC skipped");
        record_activity(
            &state,
            &paths,
            activity_event(
                entry,
                dir,
                changed,
                ActivityDetails {
                    kind: proto::ActivityKind::DuplicateSkipped,
                    commit_hash: None,
                    content_hash: Some(content_hash.clone()),
                    message: format!("Skipped duplicate {}", file_label(changed)),
                    detail: None,
                },
            ),
        )
        .await;
        ensure_repo_initialized(bin, dir, api_base, &entry.project_id).await?;
        if let Err(e) = archive_processed_file_async(dir, changed, &content_hash).await {
            warn!(file = %changed.display(), error = %e, "could not archive duplicate IFC");
        }
        return Ok(());
    }

    let intake = parse_intake_with_engine(bin, changed).await?;
    if !entry_matches_ifc(entry, &intake) {
        warn!(
            file = %changed.display(),
            project = %entry.project_id,
            expected = ?entry.ifc_project_guid,
            actual = ?intake.routing_key(),
            "IFC does not match configured project route"
        );
        record_activity(
            &state,
            &paths,
            activity_event(
                entry,
                dir,
                changed,
                ActivityDetails {
                    kind: proto::ActivityKind::RouteSkipped,
                    commit_hash: None,
                    content_hash: Some(content_hash.clone()),
                    message: format!("Skipped unmatched IFC {}", file_label(changed)),
                    detail: Some(format!(
                        "expected {:?}, actual {:?}",
                        entry.ifc_project_guid,
                        intake.routing_key()
                    )),
                },
            ),
        )
        .await;
        return Ok(());
    }

    ensure_repo_initialized(bin, dir, api_base, &entry.project_id).await?;

    info!(file = %changed.display(), hash = %content_hash, "vex import+commit+push");
    let _tree = vex_cli::import_file(bin, dir, changed).await?;

    let msg = commit_message(entry, &intake, changed);
    let author = author_identity(&intake, author_name, author_email);
    let author = author
        .as_ref()
        .map(|(name, email)| (name.as_str(), email.as_str()));
    let hash = match vex_cli::commit(bin, dir, &msg, author).await {
        Ok(h) => h,
        Err(e) => {
            // "nothing to commit" is a normal no-op; downgrade to debug.
            let s = e.to_string();
            if s.contains("nothing to commit") || s.contains("no changes") {
                tracing::debug!("watcher: no changes to commit");
                record_activity(
                    &state,
                    &paths,
                    activity_event(
                        entry,
                        dir,
                        changed,
                        ActivityDetails {
                            kind: proto::ActivityKind::NoChanges,
                            commit_hash: None,
                            content_hash: Some(content_hash.clone()),
                            message: format!("No changes in {}", file_label(changed)),
                            detail: None,
                        },
                    ),
                )
                .await;
                return Ok(());
            }
            return Err(e);
        }
    };
    info!(commit = %hash, "committed");

    let push_detail = match vex_cli::push(bin, dir, "origin", "refs/heads/main").await {
        Ok(()) => {
            info!(project = %entry.project_id, commit = %hash, "pushed");
            format!("pushed {hash}")
        }
        Err(error) => {
            warn!(project = %entry.project_id, commit = %hash, error = %error, "committed locally but push failed");
            format!("committed locally; sync failed: {error}")
        }
    };

    let snapshot_path = match archive_processed_file_async(dir, changed, &content_hash).await {
        Ok(path) => path,
        Err(error) => {
            warn!(file = %changed.display(), error = %error, "could not archive processed IFC");
            changed.to_path_buf()
        }
    };

    {
        let mut state = state.write().await;
        state.mark_ifc_hash_seen(
            content_hash.clone(),
            entry.project_id.clone(),
            intake.project_guid.clone(),
        );
        state.record_ifc_snapshot(
            entry.project_id.clone(),
            hash.clone(),
            content_hash.clone(),
            snapshot_path.to_string_lossy().to_string(),
            intake.project_guid.clone(),
        );
        state.push_activity(activity_event(
            entry,
            dir,
            changed,
            ActivityDetails {
                kind: proto::ActivityKind::CommitCreated,
                commit_hash: Some(hash.clone()),
                content_hash: Some(content_hash.clone()),
                message: msg.clone(),
                detail: Some(push_detail),
            },
        ));
        state.save(&paths)?;
    }
    Ok(())
}

async fn wait_for_stable_file(path: &Path) -> BridgeResult<()> {
    let mut last: Option<(u64, Option<SystemTime>)> = None;
    for _ in 0..FILE_STABLE_ATTEMPTS {
        let meta = tokio::fs::metadata(path).await?;
        let current = (meta.len(), meta.modified().ok());
        if last.as_ref() == Some(&current) {
            return Ok(());
        }
        last = Some(current);
        tokio::time::sleep(FILE_STABLE_INTERVAL).await;
    }
    Ok(())
}

async fn hash_file_async(path: &Path) -> BridgeResult<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || hash_file(&path))
        .await
        .map_err(join_error)?
}

async fn parse_intake_with_engine(bin: &str, path: &Path) -> BridgeResult<IfcIntake> {
    match vex_cli::ifc_intake(bin, path).await {
        Ok(intake) => Ok(intake),
        Err(engine_error) => {
            warn!(
                file = %path.display(),
                error = %engine_error,
                "vex ifc-intake unavailable; using local IFC intake fallback"
            );
            let path = path.to_path_buf();
            tokio::task::spawn_blocking(move || parse_intake(&path))
                .await
                .map_err(join_error)?
        }
    }
}

async fn archive_processed_file_async(
    dir: &Path,
    file: &Path,
    content_hash: &str,
) -> BridgeResult<PathBuf> {
    let dir = dir.to_path_buf();
    let file = file.to_path_buf();
    let content_hash = content_hash.to_string();
    tokio::task::spawn_blocking(move || archive_processed_file(&dir, &file, &content_hash))
        .await
        .map_err(join_error)?
}

fn join_error(error: tokio::task::JoinError) -> crate::errors::BridgeError {
    crate::errors::BridgeError::Config(format!("blocking worker failed: {error}"))
}

async fn record_activity(
    state: &Arc<RwLock<State>>,
    paths: &Arc<Paths>,
    event: proto::ActivityEvent,
) {
    let mut state = state.write().await;
    state.push_activity(event);
    if let Err(error) = state.save(paths) {
        warn!(error = %error, "could not persist activity event");
    }
}

fn activity_event(
    entry: &WatchEntry,
    dir: &Path,
    changed: &Path,
    details: ActivityDetails,
) -> proto::ActivityEvent {
    let ActivityDetails {
        kind,
        commit_hash,
        content_hash,
        message,
        detail,
    } = details;
    let caught_at_unix = crate::state::now_unix();
    let event_nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(caught_at_unix as u128);
    let id_material = format!(
        "{event_nanos}:{}:{kind:?}:{}:{}",
        entry.project_id,
        changed.display(),
        commit_hash.as_deref().unwrap_or_default()
    );
    let digest = blake3::hash(id_material.as_bytes()).to_hex().to_string();
    proto::ActivityEvent {
        id: format!("{caught_at_unix}-{}", &digest[..12]),
        kind,
        project_id: entry.project_id.clone(),
        project_name: entry.project_name.clone(),
        local_path: Some(dir.to_string_lossy().to_string()),
        source_path: Some(changed.to_string_lossy().to_string()),
        commit_hash,
        content_hash,
        message,
        detail,
        caught_at_unix,
    }
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("model.ifc")
        .to_string()
}

fn is_ifc_candidate(path: &Path) -> bool {
    let has_ifc_extension = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("ifc"))
        .unwrap_or(false);
    has_ifc_extension
        && !path
            .components()
            .any(|component| matches!(component, Component::Normal(name) if name == ".vex"))
}

fn matches_include(path: &Path, include: &[String]) -> bool {
    if include.is_empty() {
        return true;
    }
    let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    include.iter().any(|pattern| match pattern.as_str() {
        "*" => true,
        "*.ifc" | "*.IFC" => is_ifc_candidate(path),
        _ => pattern == file_name,
    })
}

fn existing_ifc_files(dir: &Path) -> BridgeResult<Vec<PathBuf>> {
    fn visit(dir: &Path, out: &mut Vec<(PathBuf, SystemTime)>) -> BridgeResult<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path
                .components()
                .any(|component| matches!(component, Component::Normal(name) if name == ".vex"))
            {
                continue;
            }
            let meta = entry.metadata()?;
            if meta.is_dir() {
                visit(&path, out)?;
                continue;
            }
            if !is_ifc_candidate(&path) {
                continue;
            }
            out.push((path, meta.modified().unwrap_or(SystemTime::UNIX_EPOCH)));
        }
        Ok(())
    }

    let mut files = Vec::new();
    visit(dir, &mut files)?;
    files.sort_by_key(|(_, modified)| *modified);
    Ok(files.into_iter().map(|(path, _)| path).collect())
}

fn entry_matches_ifc(entry: &WatchEntry, intake: &IfcIntake) -> bool {
    let Some(expected) = entry.ifc_project_guid.as_deref() else {
        return true;
    };
    if expected.starts_with("fingerprint:") {
        return intake
            .structural_fingerprint()
            .as_deref()
            .map(|actual| actual == expected)
            .unwrap_or(false);
    }
    intake
        .routing_key()
        .as_deref()
        .map(|actual| actual == expected)
        .unwrap_or(false)
}

async fn ensure_repo_initialized(
    bin: &str,
    dir: &Path,
    api_base: &str,
    project_id: &str,
) -> BridgeResult<()> {
    if is_vex_repo(dir) {
        return Ok(());
    }
    info!(path = %dir.display(), "initialising vex repo");
    vex_cli::init_repo(bin, dir).await?;
    if let Some(remote_url) = derive_remote_url(api_base, project_id) {
        let r = vex_cli::run(bin, Some(dir), ["remote", "add", "origin", &remote_url]).await?;
        if !r.ok() {
            warn!(stderr = %r.stderr.trim(), "could not register origin remote");
        }
    }
    Ok(())
}

fn is_vex_repo(dir: &Path) -> bool {
    dir.join(".vex").join("config.toml").is_file()
}

fn commit_message(entry: &WatchEntry, intake: &IfcIntake, changed: &Path) -> String {
    let project = intake
        .project_name
        .as_deref()
        .or(entry.project_name.as_deref())
        .unwrap_or(&entry.project_id);
    let source = intake
        .originating_system
        .as_deref()
        .or(intake.description.as_deref())
        .unwrap_or("IFC export");
    let file = changed
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("model.ifc");
    format!("auto: {project} from {source} ({file})")
}

fn author_identity(
    intake: &IfcIntake,
    default_name: Option<&str>,
    default_email: Option<&str>,
) -> Option<(String, String)> {
    let name = intake.author.as_deref().or(default_name)?;
    let email = default_email.unwrap_or("user@vex");
    Some((name.to_string(), email.to_string()))
}

fn archive_processed_file(dir: &Path, file: &Path, content_hash: &str) -> BridgeResult<PathBuf> {
    let archive_dir = dir.join(".vex").join("archive");
    fs::create_dir_all(&archive_dir)?;
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("model");
    let ext = file.extension().and_then(|s| s.to_str()).unwrap_or("ifc");
    let short_hash = &content_hash[..12.min(content_hash.len())];
    let mut target = archive_dir.join(format!("{stem}-{short_hash}.{ext}"));
    let mut suffix = 1u32;
    while target.exists() {
        target = archive_dir.join(format!("{stem}-{short_hash}-{suffix}.{ext}"));
        suffix += 1;
    }
    match fs::rename(file, &target) {
        Ok(()) => Ok(target),
        Err(rename_err) => {
            fs::copy(file, &target).map_err(|_| rename_err)?;
            fs::remove_file(file)?;
            Ok(target)
        }
    }
}

/// Convert the configured api_base (e.g. `https://studio.planmorph.software`)
/// into the SSH push URL on the architur droplet
/// (`ssh://vex@vex.planmorph.software:22/proj/<uuid>`).
///
/// We strip the leftmost subdomain label (`studio.`, `api.`, `app.`, …) so
/// the SSH host always lands on `vex.<root-domain>`, which is the
/// `vex-sshd` listener on the DO droplet. Returns `None` if the api_base
/// can't be parsed; the caller treats this as a non-fatal "user must
/// register the remote manually".
fn derive_remote_url(api_base: &str, project_id: &str) -> Option<String> {
    let host = api_base
        .strip_prefix("https://")
        .or_else(|| api_base.strip_prefix("http://"))?
        .split('/')
        .next()?
        .split(':')
        .next()?;
    if host.is_empty() {
        return None;
    }
    // Strip the leftmost label if there's at least one dot remaining after
    // it (i.e. host has 3+ labels like `studio.planmorph.software`). For a
    // bare apex like `planmorph.software` we leave it untouched.
    let root = match host.split_once('.') {
        Some((_, rest)) if rest.contains('.') => rest,
        _ => host,
    };
    Some(format!("ssh://vex@vex.{root}:22/proj/{project_id}"))
}

/// Manually run the same import → commit → push pipeline that the watcher
/// runs automatically, but driven by an explicit plug-in request rather
/// than a filesystem event. The project_id is resolved against the
/// configured `[[watch]]` entries to find the local repo dir; if no
/// matching entry exists we return `BridgeError::Config(...)` so the
/// HTTP handler can surface a 404.
///
/// Returns the resulting commit hash on success. If there were no
/// changes to commit we still attempt the push (the remote may be
/// behind on prior commits) and return the head hash.
pub async fn run_manual_push(cfg: &Config, project_id: &str, branch: &str) -> BridgeResult<String> {
    let entry = cfg
        .watch
        .iter()
        .find(|w| w.project_id == project_id)
        .ok_or_else(|| {
            crate::errors::BridgeError::Config(format!(
                "no [[watch]] entry registered for project_id `{project_id}`. \
                 Add one to config.toml or configure the project from the \
                 architur web UI before pushing."
            ))
        })?;

    let dir = std::path::PathBuf::from(&entry.path);
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }

    ensure_repo_initialized(&cfg.vex_bin, &dir, &cfg.api_base, project_id).await?;

    let scan_dir = dir.clone();
    let ifc_file = tokio::task::spawn_blocking(move || latest_ifc_file(&scan_dir))
        .await
        .map_err(join_error)??
        .ok_or_else(|| {
            crate::errors::BridgeError::Config(format!(
                "no IFC file found under configured project directory `{}`",
                dir.display()
            ))
        })?;
    let _tree = vex_cli::import_file(&cfg.vex_bin, &dir, &ifc_file).await?;

    let msg = format!(
        "manual push via vex-bridge ({})",
        ifc_file
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(project_id)
    );
    let author = match (
        cfg.default_author_name.as_deref(),
        cfg.default_author_email.as_deref(),
    ) {
        (Some(n), Some(e)) => Some((n, e)),
        _ => None,
    };
    let commit_hash = match vex_cli::commit(&cfg.vex_bin, &dir, &msg, author).await {
        Ok(h) => h,
        Err(e) => {
            let s = e.to_string();
            // "nothing to commit" is OK — push whatever HEAD is.
            if !(s.contains("nothing to commit") || s.contains("no changes")) {
                return Err(e);
            }
            // Best-effort head hash; if we can't get it just use a marker.
            "HEAD".to_string()
        }
    };

    let refspec = format!("refs/heads/{branch}");
    vex_cli::push(&cfg.vex_bin, &dir, "origin", &refspec).await?;
    info!(project = %project_id, commit = %commit_hash, branch, "manual push complete");
    Ok(commit_hash)
}

fn latest_ifc_file(dir: &Path) -> BridgeResult<Option<PathBuf>> {
    fn visit(dir: &Path, best: &mut Option<(PathBuf, std::time::SystemTime)>) -> BridgeResult<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path
                .components()
                .any(|component| matches!(component, Component::Normal(name) if name == ".vex"))
            {
                continue;
            }
            let meta = entry.metadata()?;
            if meta.is_dir() {
                visit(&path, best)?;
                continue;
            }
            if !is_ifc_candidate(&path) {
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
