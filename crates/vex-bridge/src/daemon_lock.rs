//! Advisory daemon lockfile (`<data_dir>/daemon.lock`).
//!
//! The bound TCP port (`127.0.0.1:<port>`) remains the *hard* singleton: only
//! one process can ever hold it, so two daemons can never both serve the API.
//! This lockfile is purely advisory metadata layered on top of that guarantee.
//! It records the running daemon's PID, version, port, and start time so that:
//!
//! * launchers (tray/desktop) can detect a *version-skewed* daemon and know
//!   which PID to ask to shut down — and, as a last resort, force-kill;
//! * the diagnostics endpoint can report exactly which build is live;
//! * the Repair flow can reap a hung daemon that has stopped answering HTTP.
//!
//! Because the port is the real lock, a *stale* lockfile (left behind by a
//! crash) is never fatal: the next daemon simply overwrites it on startup. We
//! therefore never block startup on the lockfile, avoiding stale-lock deadlocks.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::Paths;

/// On-disk record of the currently running daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonLock {
    /// OS process id of the running daemon.
    pub pid: u32,
    /// `CARGO_PKG_VERSION` the daemon was built from.
    pub version: String,
    /// TCP port the daemon bound (the real singleton anchor).
    pub port: u16,
    /// Unix epoch seconds at which the daemon claimed the lock.
    pub started_at: u64,
}

impl DaemonLock {
    /// Build a lock record for *this* process.
    pub fn for_current(port: u16) -> Self {
        Self {
            pid: std::process::id(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            port,
            started_at: now_unix(),
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Write (or overwrite) the lockfile for this process. Best-effort: failures are
/// logged and swallowed, never fatal — the bound port is the real guarantee.
pub fn write(paths: &Paths, port: u16) {
    let lock = DaemonLock::for_current(port);
    write_record(&paths.daemon_lock_file, &lock);
}

fn write_record(path: &Path, lock: &DaemonLock) {
    match serde_json::to_vec_pretty(lock) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(path, bytes) {
                tracing::warn!(error = ?e, path = %path.display(), "failed to write daemon lockfile");
            }
        }
        Err(e) => tracing::warn!(error = ?e, "failed to serialize daemon lockfile"),
    }
}

/// Read and parse the lockfile, if present and well-formed.
pub fn read(paths: &Paths) -> Option<DaemonLock> {
    read_path(&paths.daemon_lock_file)
}

fn read_path(path: &Path) -> Option<DaemonLock> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Remove the lockfile (called on graceful shutdown). Missing file is fine.
pub fn remove(paths: &Paths) {
    match std::fs::remove_file(&paths.daemon_lock_file) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            tracing::warn!(error = ?e, "failed to remove daemon lockfile");
        }
    }
}

/// Force-terminate a process by PID, cross-platform. Returns `true` if the kill
/// command was dispatched successfully. This is a *last resort* used only after
/// a graceful `/v1/daemon/shutdown` has timed out.
pub fn kill_pid(pid: u32) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = pid;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("vex-lock-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("daemon.lock");
        let original = DaemonLock {
            pid: 4242,
            version: "0.2.36".to_string(),
            port: 7878,
            started_at: 1_700_000_000,
        };
        write_record(&path, &original);
        let parsed = read_path(&path).expect("lockfile should parse");
        assert_eq!(parsed, original);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_missing_lock_returns_none() {
        let path = std::env::temp_dir().join("vex-lock-does-not-exist-zzz.lock");
        std::fs::remove_file(&path).ok();
        assert!(read_path(&path).is_none());
    }

    #[test]
    fn for_current_uses_this_process() {
        let lock = DaemonLock::for_current(1234);
        assert_eq!(lock.pid, std::process::id());
        assert_eq!(lock.port, 1234);
        assert_eq!(lock.version, env!("CARGO_PKG_VERSION"));
    }
}
