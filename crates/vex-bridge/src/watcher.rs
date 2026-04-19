//! Tier 3 — generic file watcher. The universal CAD adapter.
//!
//! Any CAD that can export IFC works without a plugin: the user picks
//! `Export → IFC` to a watched folder, this module sees the file appear,
//! debounces for 2s (so we don't fire on a half-written file), then runs
//! `vex add . && vex commit && vex push` via [`crate::vex_cli`].
//!
//! Future: Tier 2 hooks live here too — per-CAD scripts that drive the host's
//! "Export to IFC" command from the CLI.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tracing::{info, warn};

use crate::config::WatchEntry;
use crate::errors::BridgeResult;

const DEBOUNCE: Duration = Duration::from_secs(2);

pub struct WatchHandle {
    _watcher: RecommendedWatcher,
}

/// Spawn a background thread that observes file changes under `entry.path`
/// and invokes `on_change` once the file has been quiet for [`DEBOUNCE`].
pub fn spawn<F>(entry: WatchEntry, on_change: F) -> BridgeResult<WatchHandle>
where
    F: Fn(PathBuf) + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<Event>();
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            if let Ok(ev) = res {
                let _ = tx.send(ev);
            }
        },
        Config::default(),
    )
    .map_err(|e| crate::errors::BridgeError::Config(format!("notify: {e}")))?;

    watcher
        .watch(std::path::Path::new(&entry.path), RecursiveMode::Recursive)
        .map_err(|e| crate::errors::BridgeError::Config(format!("notify watch: {e}")))?;

    let entry_path = entry.path.clone();
    std::thread::spawn(move || {
        let mut pending: Vec<(PathBuf, Instant)> = Vec::new();
        loop {
            // Block briefly for a new event, then drain anything else.
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(ev) => {
                    if matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                        for p in ev.paths {
                            // Replace any prior pending entry for this path
                            // with a refreshed deadline.
                            pending.retain(|(pp, _)| pp != &p);
                            pending.push((p, Instant::now()));
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    warn!(path = %entry_path, "watcher disconnected");
                    return;
                }
            }
            // Fire any path whose debounce window has elapsed.
            let now = Instant::now();
            let (ready, still_pending): (Vec<_>, Vec<_>) = pending
                .into_iter()
                .partition(|(_, t)| now.duration_since(*t) >= DEBOUNCE);
            pending = still_pending;
            for (p, _) in ready {
                info!(path = %p.display(), "watcher: change settled");
                on_change(p);
            }
        }
    });

    Ok(WatchHandle { _watcher: watcher })
}
