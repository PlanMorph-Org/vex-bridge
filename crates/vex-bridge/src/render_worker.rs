//! Isolated, bounded, memory-aware render-artifact generation.
//!
//! The worker receives a canonical IFC checkout and produces only derived
//! files. It cannot alter Vex objects, refs, or commits.
//!
//! Jobs are driven by a process-wide [`RenderService`] that enforces three
//! reliability properties so many IFC exports arriving together cannot exhaust
//! a workstation:
//!
//! * **Bounded, memory-aware concurrency.** At most
//!   [`Config::render_max_concurrency`] jobs build at once, and each worker
//!   runs Node with a bounded `--max-old-space-size`, so peak render memory is
//!   roughly `concurrency * heap-cap` regardless of how much work is queued.
//! * **Fair per-project scheduling.** The next job to run is the one whose
//!   project currently has the fewest jobs in flight (ties broken by arrival
//!   order), so a single large project cannot starve the others.
//! * **Commit coalescing.** Queuing a newer commit for a project supersedes
//!   that project's still-queued (not yet running) work, so a worker never
//!   spends memory rendering a revision that a newer commit has already
//!   obsoleted. In-progress builds and published artifacts are never
//!   discarded, and semantic commits are never affected.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config::{Config, Paths};
use crate::errors::{BridgeError, BridgeResult};
use crate::render_artifact;
use crate::state::{RenderJobState, State};
use crate::vex_cli;

const RUNTIME_DIR: &str = "render-runtime-v1";
static RENDER_JOB_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn queued_submission_is_newer(existing: u64, candidate: u64) -> bool {
    candidate > existing
}

const WORKER_SOURCE: &str = include_str!("../../../tools/vex-render-worker.mjs");
const WEB_IFC_NODE_API: &[u8] =
    include_bytes!("../assets/render-worker/web-ifc/web-ifc-api-node.js");
const WEB_IFC_NODE_WASM: &[u8] =
    include_bytes!("../assets/render-worker/web-ifc/web-ifc-node.wasm");

/// Bounded resource envelope for a single render job, resolved from the daemon
/// config (with safe clamping applied by [`Config`]).
#[derive(Debug, Clone, Copy)]
struct RenderLimits {
    /// Process-wide ceiling on concurrent builds.
    max_concurrency: usize,
    /// Per-job Node old-space heap cap, in MiB (`--max-old-space-size`).
    heap_mb: u64,
    /// Per-job wall-clock timeout.
    timeout: Duration,
}

impl RenderLimits {
    fn from_config(cfg: &Config) -> Self {
        Self {
            max_concurrency: cfg.render_max_concurrency(),
            heap_mb: cfg.render_job_heap_mb(),
            timeout: cfg.render_job_timeout(),
        }
    }

    /// Conservative defaults, used when the config cannot be read.
    fn conservative_default() -> Self {
        Self::from_config(&Config::default())
    }
}

/// A queued render job together with everything a worker needs to build it.
struct RenderJob {
    node_bin: String,
    vex_bin: String,
    project_dir: PathBuf,
    project_id: String,
    commit_hash: String,
    state: Arc<RwLock<State>>,
    paths: Arc<Paths>,
    limits: RenderLimits,
    /// Captured before spawning the enqueue task, so an older delayed task
    /// cannot displace a newer queued commit for the same project.
    enqueue_sequence: u64,
}

/// One entry in the fair scheduler's pending queue.
struct QueueEntry<T> {
    project_id: String,
    /// Monotonic arrival order, used as the fair-scheduling tie-break.
    seq: u64,
    job: T,
}

/// A fair, bounded-concurrency scheduler with per-project commit coalescing.
///
/// It is deliberately synchronous and free of any I/O so its fairness and
/// coalescing behaviour can be unit-tested deterministically; the async
/// [`RenderService`] wraps it behind a mutex and performs the actual work.
struct FairScheduler<T> {
    max_concurrency: usize,
    next_seq: u64,
    /// Pending jobs, at most one per project after coalescing.
    queue: Vec<QueueEntry<T>>,
    /// Number of in-flight jobs per project (fair-scheduling input).
    running: std::collections::HashMap<String, usize>,
    in_flight: usize,
}

impl<T> FairScheduler<T> {
    fn new(max_concurrency: usize) -> Self {
        Self {
            max_concurrency: max_concurrency.max(1),
            next_seq: 0,
            queue: Vec::new(),
            running: std::collections::HashMap::new(),
            in_flight: 0,
        }
    }

    fn set_max_concurrency(&mut self, max_concurrency: usize) {
        self.max_concurrency = max_concurrency.max(1);
    }

    /// Enqueue a job, coalescing away any still-queued (not yet running) work
    /// for the same project. The superseded payloads are returned so the caller
    /// can observe them; a newly enqueued commit is always the freshest work
    /// for its project.
    fn enqueue(&mut self, project_id: String, job: T) -> Vec<T> {
        let mut superseded = Vec::new();
        let mut i = 0;
        while i < self.queue.len() {
            if self.queue[i].project_id == project_id {
                superseded.push(self.queue.remove(i).job);
            } else {
                i += 1;
            }
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.queue.push(QueueEntry {
            project_id,
            seq,
            job,
        });
        superseded
    }

    /// Pick the next job to run, or `None` if at capacity or idle. Fairness:
    /// choose the queued job whose project has the fewest jobs currently in
    /// flight; break ties by arrival order so no project can be starved.
    fn next_runnable(&mut self) -> Option<(String, T)> {
        if self.in_flight >= self.max_concurrency || self.queue.is_empty() {
            return None;
        }
        let mut best_index = 0usize;
        let mut best_key: Option<(usize, u64)> = None;
        for (index, entry) in self.queue.iter().enumerate() {
            let running = self.running.get(&entry.project_id).copied().unwrap_or(0);
            let key = (running, entry.seq);
            if best_key.is_none_or(|current| key < current) {
                best_index = index;
                best_key = Some(key);
            }
        }
        let entry = self.queue.remove(best_index);
        *self.running.entry(entry.project_id.clone()).or_insert(0) += 1;
        self.in_flight += 1;
        Some((entry.project_id, entry.job))
    }

    /// Release a slot held by a finished (or skipped) job for `project_id`.
    fn finish(&mut self, project_id: &str) {
        if let Some(count) = self.running.get_mut(project_id) {
            *count -= 1;
            if *count == 0 {
                self.running.remove(project_id);
            }
        }
        self.in_flight = self.in_flight.saturating_sub(1);
    }

    #[cfg(test)]
    fn in_flight(&self) -> usize {
        self.in_flight
    }

    #[cfg(test)]
    fn queued_len(&self) -> usize {
        self.queue.len()
    }
}

/// Process-wide render scheduler. A single instance owns the fair queue and
/// concurrency accounting shared by every `queue_after_commit` call.
struct RenderService {
    core: Mutex<FairScheduler<RenderJob>>,
    latest_submission: Mutex<HashMap<String, u64>>,
}

fn render_service() -> &'static RenderService {
    static SERVICE: OnceLock<RenderService> = OnceLock::new();
    SERVICE.get_or_init(|| RenderService {
        // Seeded conservatively; the effective concurrency is set from config
        // on every submit so a running daemon always honours the latest value.
        core: Mutex::new(FairScheduler::new(1)),
        latest_submission: Mutex::new(HashMap::new()),
    })
}

impl RenderService {
    fn register_submission(&self, project_id: &str, sequence: u64) {
        self.latest_submission
            .lock()
            .expect("render submission mutex poisoned")
            .insert(project_id.to_string(), sequence);
    }

    fn is_latest_submission(&self, project_id: &str, sequence: u64) -> bool {
        self.latest_submission
            .lock()
            .expect("render submission mutex poisoned")
            .get(project_id)
            .is_some_and(|latest| *latest == sequence)
    }

    fn submit(&'static self, job: RenderJob) {
        {
            let mut core = self.core.lock().expect("render scheduler mutex poisoned");
            core.set_max_concurrency(job.limits.max_concurrency);
            let project_id = job.project_id.clone();
            if core.queue.iter().any(|entry| {
                entry.project_id == project_id
                    && !queued_submission_is_newer(entry.job.enqueue_sequence, job.enqueue_sequence)
            }) {
                // The durable ledger has already superseded this older job.
                // Do not let delayed config loading reintroduce it ahead of
                // the newest queued commit.
                return;
            }
            core.enqueue(project_id, job);
        }
        self.pump();
    }

    /// Start as many runnable jobs as the concurrency budget allows. Each
    /// spawned job releases its slot and pumps again on completion, so the
    /// queue keeps draining without any long-lived driver task.
    fn pump(&'static self) {
        loop {
            let next = {
                let mut core = self.core.lock().expect("render scheduler mutex poisoned");
                core.next_runnable()
            };
            let Some((project_id, job)) = next else {
                break;
            };
            let service = self;
            tokio::spawn(async move {
                run_job(job).await;
                {
                    let mut core = service
                        .core
                        .lock()
                        .expect("render scheduler mutex poisoned");
                    core.finish(&project_id);
                }
                service.pump();
            });
        }
    }
}

/// Queue artifact work without delaying the commit pipeline. The raw IFC
/// viewer remains usable until this job publishes a validated artifact.
///
/// This records the durable job (coalescing away any obsolete queued work for
/// the same project) and hands it to the shared [`RenderService`], which
/// enforces bounded concurrency and fair per-project scheduling. The public
/// signature is intentionally unchanged so existing callers need no edits.
pub fn queue_after_commit(
    node_bin: String,
    vex_bin: String,
    project_dir: PathBuf,
    project_id: String,
    commit_hash: String,
    state: Arc<RwLock<State>>,
    paths: Arc<Paths>,
) {
    let enqueue_sequence = RENDER_JOB_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let service = render_service();
    service.register_submission(&project_id, enqueue_sequence);
    tokio::spawn(async move {
        let should_run = {
            let mut state = state.write().await;
            let should_run = service.is_latest_submission(&project_id, enqueue_sequence)
                && state.enqueue_render_artifact(project_id.clone(), commit_hash.clone());
            if should_run {
                state.save(&paths)
            } else {
                Ok(())
            }
            .map(|_| should_run)
        };
        let should_run = match should_run {
            Ok(should_run) => should_run,
            Err(error) => {
                warn!(project_id = %project_id, commit = %commit_hash, %error, "could not persist queued render artifact");
                return;
            }
        };
        if !should_run {
            return;
        }

        // Resolve the (clamped) resource envelope for this build. A config read
        // failure must never drop the job — fall back to conservative defaults.
        let limits = match Config::load_or_default(&paths) {
            Ok(cfg) => RenderLimits::from_config(&cfg),
            Err(error) => {
                warn!(project_id = %project_id, commit = %commit_hash, %error, "could not load render scheduler limits; using conservative defaults");
                RenderLimits::conservative_default()
            }
        };

        render_service().submit(RenderJob {
            node_bin,
            vex_bin,
            project_dir,
            project_id,
            commit_hash,
            state,
            paths,
            limits,
            enqueue_sequence,
        });
    });
}

/// Build one render job to completion. Called only by the scheduler, once a
/// concurrency slot is free.
async fn run_job(job: RenderJob) {
    let RenderJob {
        node_bin,
        vex_bin,
        project_dir,
        project_id,
        commit_hash,
        state,
        paths,
        limits,
        ..
    } = job;

    // A newer commit may have superseded this one while it waited in the
    // queue. Re-read the durable state and skip obsolete work rather than
    // spend memory rendering a revision no one will use.
    let still_queued = {
        let state = state.read().await;
        matches!(
            state.render_job_state(&project_id, &commit_hash),
            Some(RenderJobState::Queued)
        )
    };
    if !still_queued {
        info!(project_id = %project_id, commit = %commit_hash, "render job skipped; superseded or no longer queued");
        return;
    }

    if let Err(error) = update_status(
        &state,
        &paths,
        &project_id,
        &commit_hash,
        RenderJobState::Building,
    )
    .await
    {
        warn!(project_id = %project_id, commit = %commit_hash, %error, "could not persist running render artifact");
        return;
    }

    let result = generate_artifact(
        &node_bin,
        &vex_bin,
        &project_dir,
        &project_id,
        &commit_hash,
        &limits,
    )
    .await;
    let status = match result {
        Ok(path) => {
            info!(project_id = %project_id, commit = %commit_hash, path = %path.display(), "render artifact published");
            // Publishing is already atomic (rename into place); enforce the
            // cache capacity only afterwards so eviction can never affect
            // the just-published artifact or the publish itself.
            prune_render_cache_after_publish(
                &state,
                &paths,
                &project_dir,
                &project_id,
                &commit_hash,
            )
            .await;
            RenderJobState::Ready
        }
        Err(error) => {
            warn!(project_id = %project_id, commit = %commit_hash, %error, "render artifact generation failed; IFC fallback remains available");
            RenderJobState::Failed {
                message: error.to_string(),
                retryable: true,
            }
        }
    };
    if let Err(error) = update_status(&state, &paths, &project_id, &commit_hash, status).await {
        warn!(project_id = %project_id, commit = %commit_hash, %error, "could not persist render artifact result");
    }
}

async fn update_status(
    state: &Arc<RwLock<State>>,
    paths: &Paths,
    project_id: &str,
    commit_hash: &str,
    status: RenderJobState,
) -> BridgeResult<()> {
    let mut state = state.write().await;
    state.set_render_job_state(project_id.into(), commit_hash.into(), status);
    state.save(paths)
}

/// Bound the on-disk render artifact cache after a successful publish.
///
/// The just-published commit, the project's current HEAD (its latest recorded
/// IFC snapshot), and any commit a worker is still building are protected from
/// eviction; the artifact currently being served is protected inside
/// [`render_artifact::prune_render_cache`] via its serve guard. Eviction is
/// best-effort: a failure here never affects the published artifact, the
/// commit, or the job outcome.
async fn prune_render_cache_after_publish(
    state: &Arc<RwLock<State>>,
    paths: &Paths,
    project_dir: &Path,
    project_id: &str,
    commit_hash: &str,
) {
    let max_bytes = match Config::load_or_default(paths) {
        Ok(config) => config.render_cache_capacity_bytes(),
        Err(error) => {
            warn!(project_id, commit = %commit_hash, %error, "could not load config for render cache pruning; skipping");
            return;
        }
    };

    let mut protected: HashSet<String> = HashSet::new();
    protected.insert(commit_hash.to_string());
    {
        let state = state.read().await;
        for commit in state.active_render_commits(project_id) {
            protected.insert(commit);
        }
        if let Some(snapshot) = state.latest_ifc_snapshot_for_project(project_id) {
            protected.insert(snapshot.commit_hash);
        }
    }

    let project_dir = project_dir.to_path_buf();
    let project_id = project_id.to_string();
    let commit = commit_hash.to_string();
    let result = tokio::task::spawn_blocking(move || {
        render_artifact::prune_render_cache(&project_dir, max_bytes, &protected)
    })
    .await;
    match result {
        Ok(Ok(report)) if !report.evicted.is_empty() => {
            info!(
                project_id,
                commit = %commit,
                evicted = report.evicted.len(),
                retained_bytes = report.retained_bytes,
                "evicted least-recently-served render artifacts to bound cache",
            );
        }
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            warn!(project_id, commit = %commit, %error, "render cache pruning failed; cache may exceed its cap");
        }
        Err(error) => {
            warn!(project_id, commit = %commit, %error, "render cache pruning task panicked");
        }
    }
}

async fn generate_artifact(
    node_bin: &str,
    vex_bin: &str,
    project_dir: &Path,
    project_id: &str,
    commit_hash: &str,
    limits: &RenderLimits,
) -> BridgeResult<PathBuf> {
    let staging = render_artifact::create_artifact_staging_dir(project_dir, commit_hash)?;
    let ifc_path = staging.join("model.ifc");
    let result = async {
        let bytes =
            vex_cli::checkout_for_render(vex_bin, project_dir, commit_hash, &ifc_path).await?;
        if bytes == 0 {
            return Err(BridgeError::Config(
                "vex checkout returned an empty IFC for render generation".into(),
            ));
        }
        let runtime = ensure_runtime(project_dir)?;
        let output = run_node_worker(
            WorkerInvocation {
                node_bin,
                worker: &runtime.worker,
                ifc_path: &ifc_path,
                out: &staging,
                project_id,
                commit_hash,
                web_ifc_api: &runtime.web_ifc_api,
                wasm_dir: &runtime.wasm_dir,
            },
            limits,
        )
        .await?;
        info!(
            project_id,
            commit = commit_hash,
            worker_output = %output,
            "render worker completed"
        );
        std::fs::remove_file(&ifc_path)?;
        render_artifact::publish_staged_artifact(project_dir, project_id, commit_hash, &staging)
    }
    .await;

    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

struct RuntimePaths {
    worker: PathBuf,
    web_ifc_api: PathBuf,
    wasm_dir: PathBuf,
}

fn ensure_runtime(project_dir: &Path) -> BridgeResult<RuntimePaths> {
    // Concurrent render jobs can share a project's runtime directory. Serialize
    // installation so Windows never observes a remove/rename race while another
    // Node worker is starting from the same pinned runtime files.
    static RUNTIME_INSTALL_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _install_guard = RUNTIME_INSTALL_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("render runtime installation mutex poisoned");
    let wasm_dir = render_artifact::render_cache_root(project_dir)
        .join(RUNTIME_DIR)
        .join("web-ifc");
    std::fs::create_dir_all(&wasm_dir)?;
    let worker = wasm_dir
        .parent()
        .expect("web-ifc directory always has a parent")
        .join("vex-render-worker.mjs");
    let web_ifc_api = wasm_dir.join("web-ifc-api-node.js");
    let wasm = wasm_dir.join("web-ifc-node.wasm");
    write_runtime_file(&worker, WORKER_SOURCE.as_bytes())?;
    write_runtime_file(&web_ifc_api, WEB_IFC_NODE_API)?;
    write_runtime_file(&wasm, WEB_IFC_NODE_WASM)?;
    Ok(RuntimePaths {
        worker,
        web_ifc_api,
        wasm_dir,
    })
}

fn write_runtime_file(path: &Path, contents: &[u8]) -> BridgeResult<()> {
    if std::fs::read(path).ok().as_deref() == Some(contents) {
        return Ok(());
    }
    let temp = path.with_extension(format!("{}.partial", uuid::Uuid::now_v7()));
    std::fs::write(&temp, contents)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(temp, path)?;
    Ok(())
}

struct WorkerInvocation<'a> {
    node_bin: &'a str,
    worker: &'a Path,
    ifc_path: &'a Path,
    out: &'a Path,
    project_id: &'a str,
    commit_hash: &'a str,
    web_ifc_api: &'a Path,
    wasm_dir: &'a Path,
}

/// Construct the full Node.js argument vector for one worker invocation.
///
/// The `--max-old-space-size` cap is a Node *runtime* flag, so it must precede
/// the worker script path; every subsequent argument is a script argument.
/// Kept as a pure function so argument construction — especially the bounded
/// heap flag — can be unit-tested without spawning a process.
fn build_worker_args(invocation: &WorkerInvocation<'_>, heap_mb: u64) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::with_capacity(14);
    args.push(OsString::from(format!("--max-old-space-size={heap_mb}")));
    args.push(invocation.worker.as_os_str().to_os_string());
    args.push(OsString::from("--ifc"));
    args.push(invocation.ifc_path.as_os_str().to_os_string());
    args.push(OsString::from("--out"));
    args.push(invocation.out.as_os_str().to_os_string());
    args.push(OsString::from("--project-id"));
    args.push(OsString::from(invocation.project_id));
    args.push(OsString::from("--commit"));
    args.push(OsString::from(invocation.commit_hash));
    args.push(OsString::from("--web-ifc-api"));
    args.push(invocation.web_ifc_api.as_os_str().to_os_string());
    args.push(OsString::from("--wasm-dir"));
    args.push(invocation.wasm_dir.as_os_str().to_os_string());
    args
}

async fn run_node_worker(
    invocation: WorkerInvocation<'_>,
    limits: &RenderLimits,
) -> BridgeResult<String> {
    let mut command = Command::new(invocation.node_bin);
    command
        .args(build_worker_args(&invocation, limits.heap_mb))
        .kill_on_drop(true);
    let child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            BridgeError::Config(format!(
                "render worker requires Node.js `{}`, but it was not found",
                invocation.node_bin
            ))
        } else {
            BridgeError::Io(error)
        }
    })?;
    let output = tokio::time::timeout(limits.timeout, child.wait_with_output())
        .await
        .map_err(|_| {
            BridgeError::Config(format!(
                "render worker exceeded the {} second limit",
                limits.timeout.as_secs()
            ))
        })?
        .map_err(BridgeError::Io)?;
    if !output.status.success() {
        return Err(BridgeError::Config(format!(
            "render worker failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn runtime_assets_are_embedded() {
        assert!(WORKER_SOURCE.contains("vex.render-manifest/2"));
        assert!(!WEB_IFC_NODE_API.is_empty());
        assert!(!WEB_IFC_NODE_WASM.is_empty());
    }

    #[test]
    fn worker_args_place_bounded_heap_flag_before_the_script() {
        let invocation = WorkerInvocation {
            node_bin: "node",
            worker: Path::new("/rt/vex-render-worker.mjs"),
            ifc_path: Path::new("/stg/model.ifc"),
            out: Path::new("/stg"),
            project_id: "proj",
            commit_hash: "deadbeef",
            web_ifc_api: Path::new("/rt/web-ifc/web-ifc-api-node.js"),
            wasm_dir: Path::new("/rt/web-ifc"),
        };
        let args = build_worker_args(&invocation, 2048);

        // The heap cap is a Node runtime flag and must be first, before the
        // worker script, or Node treats it as a script argument and ignores it.
        assert_eq!(args[0], OsString::from("--max-old-space-size=2048"));
        assert_eq!(args[1], OsString::from("/rt/vex-render-worker.mjs"));

        // Every documented worker flag/value pair is forwarded, in order.
        let rest: Vec<String> = args[2..]
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            rest,
            vec![
                "--ifc",
                "/stg/model.ifc",
                "--out",
                "/stg",
                "--project-id",
                "proj",
                "--commit",
                "deadbeef",
                "--web-ifc-api",
                "/rt/web-ifc/web-ifc-api-node.js",
                "--wasm-dir",
                "/rt/web-ifc",
            ]
        );
    }

    #[test]
    fn worker_args_reflect_the_configured_heap_cap() {
        let invocation = WorkerInvocation {
            node_bin: "node",
            worker: Path::new("worker.mjs"),
            ifc_path: Path::new("model.ifc"),
            out: Path::new("out"),
            project_id: "p",
            commit_hash: "c",
            web_ifc_api: Path::new("api.js"),
            wasm_dir: Path::new("wasm"),
        };
        for heap in [512u64, 2048, 8192] {
            let args = build_worker_args(&invocation, heap);
            assert_eq!(
                args[0],
                OsString::from(format!("--max-old-space-size={heap}"))
            );
        }
    }

    #[test]
    fn render_limits_track_clamped_config_values() {
        let cfg = Config {
            render_max_concurrency: 0,
            render_job_heap_mb: 1,
            render_job_timeout_secs: 0,
            ..Config::default()
        };
        let limits = RenderLimits::from_config(&cfg);
        assert_eq!(limits.max_concurrency, Config::RENDER_MIN_CONCURRENCY);
        assert_eq!(limits.heap_mb, Config::RENDER_MIN_JOB_HEAP_MB);
        assert_eq!(
            limits.timeout.as_secs(),
            Config::RENDER_MIN_JOB_TIMEOUT_SECS
        );
    }

    // --- Fair scheduler ---------------------------------------------------

    fn drain_order(scheduler: &mut FairScheduler<&'static str>) -> Vec<String> {
        // Run every job to completion one at a time, recording the order the
        // scheduler chose. Deterministic because finish() releases each slot
        // before the next pick.
        let mut order = Vec::new();
        while let Some((project, _job)) = scheduler.next_runnable() {
            order.push(project.clone());
            scheduler.finish(&project);
        }
        order
    }

    #[test]
    fn fair_scheduler_coalesces_same_project_queue() {
        let mut scheduler = FairScheduler::<&'static str>::new(1);
        assert!(scheduler.enqueue("a".into(), "c1").is_empty());
        let superseded = scheduler.enqueue("a".into(), "c2");
        assert_eq!(superseded, vec!["c1"]);
        assert_eq!(scheduler.queued_len(), 1);
        // Only the newest commit for the project remains runnable.
        let (project, job) = scheduler.next_runnable().unwrap();
        assert_eq!((project.as_str(), job), ("a", "c2"));
    }

    #[test]
    fn fair_scheduler_respects_concurrency_cap() {
        let mut scheduler = FairScheduler::<&'static str>::new(2);
        scheduler.enqueue("a".into(), "a1");
        scheduler.enqueue("b".into(), "b1");
        scheduler.enqueue("c".into(), "c1");
        assert!(scheduler.next_runnable().is_some());
        assert!(scheduler.next_runnable().is_some());
        assert_eq!(scheduler.in_flight(), 2);
        // Third pick is denied until a slot is released.
        assert!(scheduler.next_runnable().is_none());
        scheduler.finish("a");
        assert!(scheduler.next_runnable().is_some());
    }

    #[test]
    fn fair_scheduler_round_robins_across_projects() {
        let mut scheduler = FairScheduler::<&'static str>::new(2);
        scheduler.enqueue("a".into(), "a1");
        scheduler.enqueue("b".into(), "b1");
        scheduler.enqueue("c".into(), "c1");
        // First pick: all idle, arrival order breaks the tie -> a.
        let (p1, _) = scheduler.next_runnable().unwrap();
        assert_eq!(p1, "a");
        // Second pick: a now has one in flight, so b and c (zero each) are
        // preferred; arrival order picks b.
        let (p2, _) = scheduler.next_runnable().unwrap();
        assert_eq!(p2, "b");
        // At capacity now.
        assert!(scheduler.next_runnable().is_none());
        scheduler.finish(&p1);
        // Only c is left queued.
        let (p3, _) = scheduler.next_runnable().unwrap();
        assert_eq!(p3, "c");
    }

    #[test]
    fn fair_scheduler_prevents_single_project_starvation() {
        // A "big" project floods commits; a "small" project enqueues once.
        // Even with single-flight concurrency, the small project must run.
        let mut scheduler = FairScheduler::<&'static str>::new(1);
        scheduler.enqueue("big".into(), "b1");
        scheduler.enqueue("small".into(), "s1");

        // big is picked first (arrival order tie-break) and occupies the slot.
        let (first, _) = scheduler.next_runnable().unwrap();
        assert_eq!(first, "big");
        assert!(scheduler.next_runnable().is_none());

        // While big runs it commits again; coalescing keeps only the newest,
        // and the fresh commit arrives *after* small in the queue.
        scheduler.enqueue("big".into(), "b2");
        scheduler.finish(&first);

        // small has zero in-flight and the lower arrival seq, so it runs next
        // rather than being starved by the flooding project.
        let (second, _) = scheduler.next_runnable().unwrap();
        assert_eq!(second, "small");
        scheduler.finish(&second);

        // Then the coalesced big commit runs.
        let (third, _) = scheduler.next_runnable().unwrap();
        assert_eq!(third, "big");
    }

    #[test]
    fn fair_scheduler_full_drain_is_fair() {
        let mut scheduler = FairScheduler::<&'static str>::new(1);
        scheduler.enqueue("a".into(), "a1");
        scheduler.enqueue("b".into(), "b1");
        scheduler.enqueue("c".into(), "c1");
        assert_eq!(drain_order(&mut scheduler), vec!["a", "b", "c"]);
    }

    #[test]
    fn delayed_submission_cannot_replace_newer_queued_job() {
        assert!(queued_submission_is_newer(4, 5));
        assert!(!queued_submission_is_newer(5, 4));
        assert!(!queued_submission_is_newer(5, 5));
    }

    #[test]
    fn stale_enqueue_task_is_rejected_before_durable_coalescing() {
        let service = RenderService {
            core: Mutex::new(FairScheduler::new(1)),
            latest_submission: Mutex::new(HashMap::new()),
        };
        service.register_submission("project", 12);
        service.register_submission("project", 13);
        assert!(!service.is_latest_submission("project", 12));
        assert!(service.is_latest_submission("project", 13));
    }
}
