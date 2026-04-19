//! Glue: watch entries from config → vex add+commit+push pipeline.
//!
//! For each `[[watch]]` block in `config.toml`, we spawn a debounced watcher
//! (see [`crate::watcher`]) on the directory. Whenever a file settles, we run
//! `vex add . && vex commit -m "<auto>" && vex push origin main` against the
//! local repo. If the directory isn't a vex repo yet we initialize it and
//! register the project's remote so the very first IFC upload works.
//!
//! Failures are logged but never crash the daemon — a misconfigured watch
//! must not take down the bridge for other projects.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::runtime::Handle;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::config::{Config, WatchEntry};
use crate::errors::BridgeResult;
use crate::vex_cli;
use crate::watcher::{self, WatchHandle};

/// One pipeline per configured watch. Held by the daemon for the lifetime
/// of the process; dropping the inner handle stops the watcher.
pub struct WatchPipeline {
    pub project_id: String,
    pub path: PathBuf,
    _handle: WatchHandle,
}

/// Spin up one debounced watcher per `[[watch]]` entry. Returns the live
/// handles so the caller can keep them alive.
pub fn spawn_all(cfg: &Config, runtime: Handle) -> Vec<WatchPipeline> {
    let mut out = Vec::new();
    for entry in &cfg.watch {
        match spawn_one(cfg, runtime.clone(), entry.clone()) {
            Ok(p) => out.push(p),
            Err(e) => {
                warn!(project = %entry.project_id, path = %entry.path, error = %e, "watch setup failed")
            }
        }
    }
    info!(count = out.len(), "watchers active");
    out
}

fn spawn_one(cfg: &Config, runtime: Handle, entry: WatchEntry) -> BridgeResult<WatchPipeline> {
    let dir = PathBuf::from(&entry.path);
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    let project_id = entry.project_id.clone();
    let bin = cfg.vex_bin.clone();
    let api_base = cfg.api_base.clone();
    let author_name = cfg.default_author_name.clone();
    let author_email = cfg.default_author_email.clone();

    // Coalesce concurrent change events on the same repo into one in-flight
    // add/commit/push attempt. notify can fire many events per save.
    let lock: Arc<Mutex<()>> = Arc::new(Mutex::new(()));

    let dir_for_cb = dir.clone();
    let handle = watcher::spawn(entry.clone(), move |changed| {
        let runtime = runtime.clone();
        let lock = lock.clone();
        let bin = bin.clone();
        let api_base = api_base.clone();
        let project_id = project_id.clone();
        let dir = dir_for_cb.clone();
        let author_name = author_name.clone();
        let author_email = author_email.clone();
        runtime.spawn(async move {
            let _g = lock.lock().await;
            if let Err(e) = run_pipeline(
                &bin,
                &dir,
                &api_base,
                &project_id,
                &changed,
                author_name.as_deref(),
                author_email.as_deref(),
            )
            .await
            {
                error!(error = %e, project = %project_id, "push pipeline failed");
            }
        });
    })?;

    Ok(WatchPipeline {
        project_id: entry.project_id,
        path: dir,
        _handle: handle,
    })
}

async fn run_pipeline(
    bin: &str,
    dir: &Path,
    api_base: &str,
    project_id: &str,
    changed: &Path,
    author_name: Option<&str>,
    author_email: Option<&str>,
) -> BridgeResult<()> {
    // Idempotent init: if `.vex` is missing, initialise the repo and register
    // the architur remote URL derived from `project_id`. This keeps the user's
    // first export working without a manual `vex init`.
    let dot_vex = dir.join(".vex");
    if !dot_vex.is_dir() {
        info!(path = %dir.display(), "initialising vex repo");
        vex_cli::init_repo(bin, dir).await?;
        if let Some(remote_url) = derive_remote_url(api_base, project_id) {
            // `vex remote add origin <url>` — failure here is non-fatal so a
            // bad api_base doesn't permanently break the watch loop.
            let r = vex_cli::run(bin, Some(dir), ["remote", "add", "origin", &remote_url]).await?;
            if !r.ok() {
                warn!(stderr = %r.stderr.trim(), "could not register origin remote");
            }
        }
    }

    info!(file = %changed.display(), "vex add+commit+push");
    vex_cli::add_all(bin, dir).await?;

    let msg = format!(
        "auto: {} via vex-bridge",
        changed
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("change")
    );
    let author = match (author_name, author_email) {
        (Some(n), Some(e)) => Some((n, e)),
        _ => None,
    };
    let hash = match vex_cli::commit(bin, dir, &msg, author).await {
        Ok(h) => h,
        Err(e) => {
            // "nothing to commit" is a normal no-op; downgrade to debug.
            let s = e.to_string();
            if s.contains("nothing to commit") || s.contains("no changes") {
                tracing::debug!("watcher: no changes to commit");
                return Ok(());
            }
            return Err(e);
        }
    };
    info!(commit = %hash, "committed");

    // Push to origin/main. The remote URL was registered above (or by the
    // user manually) and points at vex-sshd → vex-serve on the architur host.
    vex_cli::push(bin, dir, "origin", "refs/heads/main").await?;
    info!(project = %project_id, commit = %hash, "pushed");
    Ok(())
}

/// Convert `https://api.architur.com` → `ssh://vex@vex.architur.com:22/proj/<uuid>`.
/// Returns `None` if the api_base can't be parsed; the caller treats this as
/// a non-fatal "user must register the remote manually".
fn derive_remote_url(api_base: &str, project_id: &str) -> Option<String> {
    let host = api_base
        .strip_prefix("https://")
        .or_else(|| api_base.strip_prefix("http://"))?
        .split('/')
        .next()?
        .split(':')
        .next()?
        .trim_start_matches("api.");
    if host.is_empty() {
        return None;
    }
    Some(format!("ssh://vex@vex.{host}:22/proj/{project_id}"))
}
