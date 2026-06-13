//! Shared daemon supervision for the tray and desktop launchers.
//!
//! Both launchers must guarantee the same thing before they show any UI: a
//! *single*, *version-matched*, *responsive* daemon is serving the local API.
//! Historically the tray and desktop each rolled their own (divergent) startup
//! logic, so the desktop self-healed a stale daemon while the tray happily
//! attached to whatever was already running. That skew — an old daemon next to
//! new UI — was a recurring source of "just uninstall and reinstall" bugs.
//!
//! This module centralises the policy:
//!
//! 1. Probe `/v1/health`.
//! 2. If a daemon answers with *our* version → done, attach.
//! 3. If a daemon answers with a *different* version → retire it (graceful
//!    `/v1/daemon/shutdown`, then force-kill via the lockfile PID if it refuses)
//!    and launch a fresh one from this binary's sibling `vex-bridge`.
//! 4. If nothing answers but the port is *occupied* (a hung daemon holding the
//!    socket without serving) → reclaim it the same way, instead of letting the
//!    new daemon die on `AddrInUse`.
//! 5. If the port is free → just start a daemon.
//!
//! Force-kill is strictly a fallback after graceful shutdown times out.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::Paths;
use crate::daemon_lock;

/// What the launcher should do given the observed daemon state. Pure/testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupDecision {
    /// A daemon on the wanted version is already serving — attach to it.
    Healthy,
    /// A daemon on a different version is serving — retire it, then start fresh.
    RestartStale,
    /// Nothing is answering and the port is free — start a daemon.
    StartFresh,
    /// Nothing answers but the port is occupied (hung) — reclaim, then start.
    ReclaimHung,
}

/// Pure decision used by [`ensure_daemon`]; separated for unit testing.
pub(crate) fn decide(
    running_version: Option<&str>,
    port_in_use: bool,
    want: &str,
) -> StartupDecision {
    match running_version {
        Some(version) if version == want => StartupDecision::Healthy,
        Some(_) => StartupDecision::RestartStale,
        None if port_in_use => StartupDecision::ReclaimHung,
        None => StartupDecision::StartFresh,
    }
}

/// Ensure a single, version-matched, responsive daemon is running before the
/// caller attaches its UI. Best-effort: on failure we still let the UI come up
/// (it will simply show "daemon not reachable") rather than blocking the app.
pub fn ensure_daemon(paths: &Paths, port: u16) {
    let want = env!("CARGO_PKG_VERSION");
    match decide(daemon_version(port).as_deref(), port_in_use(port), want) {
        StartupDecision::Healthy => return,
        StartupDecision::RestartStale => {
            tracing::warn!(expected = %want, "daemon version mismatch; retiring stale daemon");
            retire(paths, port);
        }
        StartupDecision::ReclaimHung => {
            tracing::warn!(port, "port occupied by unresponsive daemon; reclaiming");
            retire(paths, port);
        }
        StartupDecision::StartFresh => {}
    }

    if let Err(error) = start_daemon() {
        tracing::error!(error = ?error, "failed to spawn vex-bridge daemon");
        return;
    }
    wait_for_version(port, want, Duration::from_secs(8));
}

/// Force a clean restart on demand (the Repair action): retire whatever is
/// there — gracefully if possible, by force if not — clear the stale lockfile,
/// and launch a fresh daemon. Returns `true` if a healthy daemon answered
/// afterwards.
pub fn force_repair(paths: &Paths, port: u16) -> bool {
    let want = env!("CARGO_PKG_VERSION");
    retire(paths, port);
    daemon_lock::remove(paths);
    if let Err(error) = start_daemon() {
        tracing::error!(error = ?error, "repair: failed to spawn vex-bridge daemon");
        return false;
    }
    wait_for_version(port, want, Duration::from_secs(8));
    daemon_version(port).as_deref() == Some(want)
}

/// Retire the daemon currently on `port`: ask it to shut down cleanly, and if it
/// refuses within the deadline, force-kill the PID recorded in the lockfile.
fn retire(paths: &Paths, port: u16) {
    request_shutdown(paths, port);
    wait_for_port_free(port, Duration::from_secs(5));
    if !port_in_use(port) {
        return;
    }
    // Graceful shutdown timed out — fall back to force-kill via the lockfile.
    if let Some(lock) = daemon_lock::read(paths) {
        tracing::warn!(
            pid = lock.pid,
            "graceful shutdown timed out; force-killing daemon"
        );
        daemon_lock::kill_pid(lock.pid);
        wait_for_port_free(port, Duration::from_secs(3));
    } else {
        tracing::warn!("graceful shutdown timed out and no lockfile PID is available");
    }
}

/// Poll until a daemon reports the wanted version, or the deadline passes.
fn wait_for_version(port: u16, want: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if daemon_version(port).as_deref() == Some(want) {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Probe `/v1/health` and return the daemon's reported version, or `None`.
fn daemon_version(port: u16) -> Option<String> {
    let body = health_body(port)?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    value
        .get("version")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// `true` if *something* is listening on the port (even if it won't serve HTTP).
fn port_in_use(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(500),
    )
    .is_ok()
}

/// Issue a raw `GET /v1/health` and return the JSON body if the daemon answered
/// `200 OK`.
fn health_body(port: u16) -> Option<String> {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    use std::io::{Read, Write};
    write!(
        stream,
        "GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    if !response.starts_with("HTTP/1.1 200") {
        return None;
    }
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.trim().to_string())
}

/// Best-effort graceful shutdown via the token-gated endpoint. Reads the access
/// token the daemon wrote to the per-user data dir to authenticate.
fn request_shutdown(paths: &Paths, port: u16) {
    let Ok(token) = std::fs::read_to_string(&paths.access_token_file) else {
        return;
    };
    let token = token.trim();
    if token.is_empty() {
        return;
    }
    let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) else {
        return;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    use std::io::{Read, Write};
    let request = format!(
        "POST /v1/daemon/shutdown HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         X-Vex-Bridge-Token: {token}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return;
    }
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
}

/// Poll until the daemon stops answering on the port, or the deadline passes.
fn wait_for_port_free(port: u16, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if health_body(port).is_none() && !port_in_use(port) {
            return;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Spawn `vex-bridge start` from this binary's sibling executable.
fn start_daemon() -> std::io::Result<()> {
    let mut command = Command::new(sibling_exe("vex-bridge"));
    command
        .arg("start")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console_window(&mut command);
    command.spawn().map(|_| ())
}

/// Resolve a sibling executable next to the current binary, falling back to the
/// bare name (resolved on `PATH`) if the sibling can't be found.
fn sibling_exe(name: &str) -> PathBuf {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    if let Ok(mut exe) = std::env::current_exe() {
        exe.set_file_name(&filename);
        if exe.is_file() {
            return exe;
        }
    }
    PathBuf::from(filename)
}

#[cfg(target_os = "windows")]
fn hide_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_console_window(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_version_is_healthy() {
        assert_eq!(
            decide(Some("0.2.36"), true, "0.2.36"),
            StartupDecision::Healthy
        );
    }

    #[test]
    fn different_version_restarts_stale() {
        assert_eq!(
            decide(Some("0.2.35"), true, "0.2.36"),
            StartupDecision::RestartStale
        );
    }

    #[test]
    fn free_port_starts_fresh() {
        assert_eq!(decide(None, false, "0.2.36"), StartupDecision::StartFresh);
    }

    #[test]
    fn occupied_unresponsive_port_is_reclaimed() {
        assert_eq!(decide(None, true, "0.2.36"), StartupDecision::ReclaimHung);
    }
}
