//! Thin wrapper around the bundled `vex` binary. All arguments are passed via
//! `ProcessStartInfo`-equivalent argument vectors — never concatenated into a
//! shell string — so nothing the user types becomes a shell metacharacter.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use tokio::process::Command;
use tokio::sync::Mutex as AsyncMutex;

use crate::errors::{BridgeError, BridgeResult};
use crate::ifc::IfcIntake;
use vex_bridge_protocol::schema;

/// Per-repository async lock. The vex object store (`.vex/objects.redb`) takes
/// an exclusive OS-level lock, so two `vex` processes touching the same repo at
/// once make the second fail with "Database already open. Cannot acquire lock."
/// That happened whenever the watcher's import/commit overlapped the dashboard's
/// periodic `log`/`changes` polling, surfacing as a 502. Serialise every
/// repo-scoped invocation (any call with a working directory) per canonical
/// path so concurrent calls queue instead of racing the lock.
fn repo_lock(dir: &Path) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<StdMutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let key = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let mut map = LOCKS
        .get_or_init(|| StdMutex::new(HashMap::new()))
        .lock()
        .expect("repo lock map poisoned");
    map.entry(key)
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

/// Reject a `vex.visual-diff` payload whose schema name/major differs from
/// what this bridge build understands. Forwarding an incompatible payload to
/// the web viewer would silently mis-render a diff; failing here surfaces an
/// engine/bridge version skew as a clear, actionable error instead.
fn ensure_visual_diff_schema(value: &serde_json::Value) -> BridgeResult<()> {
    match value.get("schema").and_then(|s| s.as_str()) {
        Some(tag) if schema::is_compatible(tag, schema::VISUAL_DIFF) => Ok(()),
        Some(tag) => Err(BridgeError::VexCli(format!(
            "vex returned visual diff schema `{tag}`, but this bridge expects \
             `{}`. Update the bundled vex engine or vex-bridge so their \
             contract versions match.",
            schema::VISUAL_DIFF
        ))),
        None => Err(BridgeError::VexCli(format!(
            "vex visual diff output is missing the required `schema` field \
             (expected `{}`)",
            schema::VISUAL_DIFF
        ))),
    }
}

pub struct VexRun {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl VexRun {
    pub fn ok(&self) -> bool {
        self.status == 0
    }
}

pub async fn run<I, S>(bin: &str, cwd: Option<&Path>, args: I) -> BridgeResult<VexRun>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = Command::new(bin);
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Hold the per-repo lock across the whole child process so repo-scoped vex
    // commands never contend for the exclusive redb lock. Commands without a
    // working directory (e.g. `--version`, `ifc-intake`) touch no repo and run
    // unserialised.
    let lock = cwd.map(repo_lock);
    let _guard = match lock.as_ref() {
        Some(l) => Some(l.lock().await),
        None => None,
    };

    let out = match cmd.output().await {
        Ok(out) => out,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(engine_not_found(bin));
        }
        Err(err) => return Err(BridgeError::Io(err)),
    };
    Ok(VexRun {
        status: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Wall-clock cap on a single `vex push`. Push is the only network-bound engine
/// call: it opens an SSH connection to the remote object store. An unreachable
/// or slow remote would otherwise block the per-repo pipeline indefinitely —
/// which is exactly what made imports appear to "stick at 0", because the
/// commit/hash/snapshot/archive bookkeeping only runs *after* push returns.
/// Bounding it lets a stalled push fail fast and be queued for retry instead.
const PUSH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);
/// Checkout is local but can still be expensive for a large historical model.
/// Bound it so a corrupted object store or an unexpectedly pathological commit
/// does not leave a dashboard request holding the repository lock forever.
const CHECKOUT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Engine-binary-missing error shared by [`run`] and [`run_bounded`].
fn engine_not_found(bin: &str) -> BridgeError {
    BridgeError::VexCli(format!(
        "could not launch the vex engine binary `{bin}` — it was not found. \
         Reinstall Vex Atlas, or set `vex_bin` in config.toml to a valid engine path."
    ))
}

/// Like [`run`], but abandons (and kills) the child if it exceeds `timeout`.
/// Used for the network-bound `push` so a wedged remote cannot stall the
/// pipeline forever. The child is spawned with `kill_on_drop`, so timing out —
/// which drops the handle — reaps the process and releases the per-repo lock.
async fn run_bounded<I, S>(
    bin: &str,
    cwd: Option<&Path>,
    args: I,
    timeout: std::time::Duration,
) -> BridgeResult<VexRun>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = Command::new(bin);
    if let Some(d) = cwd {
        cmd.current_dir(d);
    }
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let lock = cwd.map(repo_lock);
    let _guard = match lock.as_ref() {
        Some(l) => Some(l.lock().await),
        None => None,
    };

    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(engine_not_found(bin));
        }
        Err(err) => return Err(BridgeError::Io(err)),
    };

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => Ok(VexRun {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }),
        Ok(Err(err)) => Err(BridgeError::Io(err)),
        Err(_elapsed) => Err(BridgeError::VexCli(format!(
            "engine command timed out after {}s (remote unreachable?); will retry",
            timeout.as_secs()
        ))),
    }
}

pub async fn version(bin: &str) -> BridgeResult<Option<String>> {
    let r = run(bin, None, ["--version"]).await?;
    if !r.ok() {
        return Ok(None);
    }
    // clap prints e.g. "vex 0.1.0"
    let line = r.stdout.lines().next().unwrap_or("").trim().to_string();
    Ok(line.split_whitespace().nth(1).map(str::to_string))
}

pub async fn init_repo(bin: &str, dir: &Path) -> BridgeResult<()> {
    let r = run(bin, Some(dir), ["init"]).await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    Ok(())
}

pub async fn import_file(bin: &str, dir: &Path, file: &Path) -> BridgeResult<String> {
    let args: Vec<OsString> = vec![
        "--json".into(),
        "import".into(),
        file.as_os_str().to_os_string(),
    ];
    let r = run(bin, Some(dir), args).await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    let value: serde_json::Value = serde_json::from_str(&r.stdout)?;
    if let Some(timings) = value.get("timings_ms") {
        tracing::info!(
            nodes = value.get("nodes").and_then(serde_json::Value::as_u64),
            edges = value.get("edges").and_then(serde_json::Value::as_u64),
            parse_ms = timings.get("parse").and_then(serde_json::Value::as_u64),
            persist_ms = timings.get("persist").and_then(serde_json::Value::as_u64),
            total_ms = timings.get("total").and_then(serde_json::Value::as_u64),
            "vex IFC import timings"
        );
    }
    value
        .get("tree")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| BridgeError::VexCli("vex import did not return a tree hash".into()))
}

pub async fn ifc_intake(bin: &str, file: &Path) -> BridgeResult<IfcIntake> {
    let args: Vec<OsString> = vec![
        "--json".into(),
        "ifc-intake".into(),
        file.as_os_str().to_os_string(),
    ];
    let r = run(bin, None, args).await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    Ok(serde_json::from_str(&r.stdout)?)
}

pub async fn commit(
    bin: &str,
    dir: &Path,
    message: &str,
    author: Option<(&str, &str)>,
) -> BridgeResult<String> {
    let mut args: Vec<String> = vec![
        "--json".into(),
        "commit".into(),
        "-m".into(),
        message.into(),
    ];
    if let Some((name, email)) = author {
        args.push("--author".into());
        args.push(name.into());
        args.push("--email".into());
        args.push(email.into());
    }
    let r = run(bin, Some(dir), args).await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    let value: serde_json::Value = serde_json::from_str(&r.stdout)?;
    value
        .get("commit")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| BridgeError::VexCli("vex commit did not return a commit hash".into()))
}

pub async fn push(bin: &str, dir: &Path, remote: &str, branch: &str) -> BridgeResult<()> {
    let r = run_bounded(bin, Some(dir), ["push", remote, branch], PUSH_TIMEOUT).await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    Ok(())
}

pub async fn log_json(bin: &str, dir: &Path) -> BridgeResult<serde_json::Value> {
    let r = run(bin, Some(dir), ["--json", "log"]).await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    Ok(serde_json::from_str(&r.stdout)?)
}

pub async fn changes_json(bin: &str, dir: &Path) -> BridgeResult<serde_json::Value> {
    let r = run(bin, Some(dir), ["--json", "changes"]).await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    let value: serde_json::Value = serde_json::from_str(&r.stdout)?;
    ensure_visual_diff_schema(&value)?;
    Ok(value)
}

pub async fn compare_json(
    bin: &str,
    dir: &Path,
    from: &str,
    to: &str,
) -> BridgeResult<serde_json::Value> {
    let r = run(bin, Some(dir), ["--json", "compare", from, to]).await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    let value: serde_json::Value = serde_json::from_str(&r.stdout)?;
    ensure_visual_diff_schema(&value)?;
    Ok(value)
}

/// Authoritative element inventory at `reference` via `vex elements --rooted`.
///
/// Returns the parsed `vex.elements/1` payload. Reads node blobs directly from
/// the committed tree, so it reflects exactly what was committed — no IFC
/// re-parsing or filename guessing. Errors (including older engines that lack
/// the `elements` subcommand) propagate so callers can fall back gracefully.
pub async fn elements_json(
    bin: &str,
    dir: &Path,
    reference: &str,
) -> BridgeResult<serde_json::Value> {
    let r = run(
        bin,
        Some(dir),
        ["--json", "elements", "--rooted", reference],
    )
    .await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    Ok(serde_json::from_str(&r.stdout)?)
}

/// Materialize the IFC model committed at `reference` to `out` via
/// `vex checkout`. Returns the number of bytes written. This reconstructs the
/// exact committed model from the object store — independent of whatever IFC
/// files currently sit on disk — so callers can serve commit-exact geometry
/// even for historical commits whose original snapshot is gone.
pub async fn checkout(bin: &str, dir: &Path, reference: &str, out: &Path) -> BridgeResult<u64> {
    let args: Vec<OsString> = vec![
        "--json".into(),
        "checkout".into(),
        reference.into(),
        "-o".into(),
        out.as_os_str().to_os_string(),
    ];
    let r = run_bounded(bin, Some(dir), args, CHECKOUT_TIMEOUT).await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    let value: serde_json::Value = serde_json::from_str(&r.stdout)?;
    Ok(value
        .get("bytes")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_matching_major() {
        let v = json!({ "schema": "vex.visual-diff/1", "elements": [] });
        assert!(ensure_visual_diff_schema(&v).is_ok());
    }

    #[test]
    fn rejects_incompatible_major() {
        let v = json!({ "schema": "vex.visual-diff/2", "elements": [] });
        let err = ensure_visual_diff_schema(&v).unwrap_err();
        assert!(matches!(err, BridgeError::VexCli(_)));
    }

    #[test]
    fn rejects_missing_schema() {
        let v = json!({ "elements": [] });
        assert!(ensure_visual_diff_schema(&v).is_err());
    }
}
