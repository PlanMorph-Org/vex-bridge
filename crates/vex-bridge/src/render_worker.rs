//! Isolated, bounded render-artifact generation.
//!
//! The worker receives a canonical IFC checkout and produces only derived
//! files. It cannot alter Vex objects, refs, or commits. A single semaphore
//! keeps large geometry jobs from exhausting a workstation when several IFC
//! exports arrive together.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::process::Command;
use tokio::sync::{RwLock, Semaphore};
use tracing::{info, warn};

use crate::config::Paths;
use crate::errors::{BridgeError, BridgeResult};
use crate::render_artifact;
use crate::state::{RenderArtifactJobStatus, State};
use crate::vex_cli;

const RENDER_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const RUNTIME_DIR: &str = "render-runtime-v1";

const WORKER_SOURCE: &str = include_str!("../../../tools/vex-render-worker.mjs");
const WEB_IFC_NODE_API: &[u8] =
    include_bytes!("../assets/render-worker/web-ifc/web-ifc-api-node.js");
const WEB_IFC_NODE_WASM: &[u8] =
    include_bytes!("../assets/render-worker/web-ifc/web-ifc-node.wasm");

fn worker_semaphore() -> &'static Semaphore {
    static SEMAPHORE: OnceLock<Semaphore> = OnceLock::new();
    SEMAPHORE.get_or_init(|| Semaphore::new(1))
}

/// Queue artifact work without delaying the commit pipeline. The raw IFC
/// viewer remains usable until this job publishes a validated artifact.
pub fn queue_after_commit(
    node_bin: String,
    vex_bin: String,
    project_dir: PathBuf,
    project_id: String,
    commit_hash: String,
    state: Arc<RwLock<State>>,
    paths: Arc<Paths>,
) {
    tokio::spawn(async move {
        let should_run = {
            let mut state = state.write().await;
            let should_run = state.enqueue_render_artifact(project_id.clone(), commit_hash.clone());
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

        let _permit = worker_semaphore()
            .acquire()
            .await
            .expect("render worker semaphore must remain open");
        if let Err(error) = update_status(
            &state,
            &paths,
            &project_id,
            &commit_hash,
            RenderArtifactJobStatus::Building,
        )
        .await
        {
            warn!(project_id = %project_id, commit = %commit_hash, %error, "could not persist running render artifact");
            return;
        }

        let result =
            generate_artifact(&node_bin, &vex_bin, &project_dir, &project_id, &commit_hash).await;
        let status = match result {
            Ok(path) => {
                info!(project_id = %project_id, commit = %commit_hash, path = %path.display(), "render artifact published");
                RenderArtifactJobStatus::Ready
            }
            Err(error) => {
                warn!(project_id = %project_id, commit = %commit_hash, %error, "render artifact generation failed; IFC fallback remains available");
                RenderArtifactJobStatus::Failed {
                    message: error.to_string(),
                    retryable: true,
                }
            }
        };
        if let Err(error) = update_status(&state, &paths, &project_id, &commit_hash, status).await {
            warn!(project_id = %project_id, commit = %commit_hash, %error, "could not persist render artifact result");
        }
    });
}

async fn update_status(
    state: &Arc<RwLock<State>>,
    paths: &Paths,
    project_id: &str,
    commit_hash: &str,
    status: RenderArtifactJobStatus,
) -> BridgeResult<()> {
    let mut state = state.write().await;
    state.set_render_artifact_status(project_id.into(), commit_hash.into(), status);
    state.save(paths)
}

async fn generate_artifact(
    node_bin: &str,
    vex_bin: &str,
    project_dir: &Path,
    project_id: &str,
    commit_hash: &str,
) -> BridgeResult<PathBuf> {
    let staging = render_artifact::create_artifact_staging_dir(project_dir, commit_hash)?;
    let ifc_path = staging.join("model.ifc");
    let result = async {
        let bytes = vex_cli::checkout(vex_bin, project_dir, commit_hash, &ifc_path).await?;
        if bytes == 0 {
            return Err(BridgeError::Config(
                "vex checkout returned an empty IFC for render generation".into(),
            ));
        }
        let runtime = ensure_runtime(project_dir)?;
        let output = run_node_worker(WorkerInvocation {
            node_bin,
            worker: &runtime.worker,
            ifc_path: &ifc_path,
            out: &staging,
            project_id,
            commit_hash,
            web_ifc_api: &runtime.web_ifc_api,
            wasm_dir: &runtime.wasm_dir,
        })
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

async fn run_node_worker(invocation: WorkerInvocation<'_>) -> BridgeResult<String> {
    let mut command = Command::new(invocation.node_bin);
    command
        .arg(invocation.worker)
        .arg("--ifc")
        .arg(invocation.ifc_path)
        .arg("--out")
        .arg(invocation.out)
        .arg("--project-id")
        .arg(invocation.project_id)
        .arg("--commit")
        .arg(invocation.commit_hash)
        .arg("--web-ifc-api")
        .arg(invocation.web_ifc_api)
        .arg("--wasm-dir")
        .arg(invocation.wasm_dir)
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
    let output = tokio::time::timeout(RENDER_TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| {
            BridgeError::Config(format!(
                "render worker exceeded the {} minute limit",
                RENDER_TIMEOUT.as_secs() / 60
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

    #[test]
    fn runtime_assets_are_embedded() {
        assert!(WORKER_SOURCE.contains("vex.render-manifest/1"));
        assert!(!WEB_IFC_NODE_API.is_empty());
        assert!(!WEB_IFC_NODE_WASM.is_empty());
    }
}
