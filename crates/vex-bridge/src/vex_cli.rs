//! Thin wrapper around the bundled `vex` binary. All arguments are passed via
//! `ProcessStartInfo`-equivalent argument vectors — never concatenated into a
//! shell string — so nothing the user types becomes a shell metacharacter.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

use crate::errors::{BridgeError, BridgeResult};
use crate::ifc::IfcIntake;
use vex_bridge_protocol::schema;

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

    let out = cmd.output().await.map_err(BridgeError::Io)?;
    Ok(VexRun {
        status: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
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
    let r = run(bin, Some(dir), ["push", remote, branch]).await?;
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
