//! Thin wrapper around the bundled `vex` binary. All arguments are passed via
//! `ProcessStartInfo`-equivalent argument vectors — never concatenated into a
//! shell string — so nothing the user types becomes a shell metacharacter.

use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

use crate::errors::{BridgeError, BridgeResult};

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

pub async fn add_all(bin: &str, dir: &Path) -> BridgeResult<()> {
    let r = run(bin, Some(dir), ["add", "."]).await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    Ok(())
}

pub async fn commit(
    bin: &str,
    dir: &Path,
    message: &str,
    author: Option<(&str, &str)>,
) -> BridgeResult<String> {
    let mut args: Vec<String> = vec!["commit".into(), "-m".into(), message.into()];
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
    // The CLI prints "Created commit <hash>" — pull the hash out.
    let hash = r
        .stdout
        .split_whitespace()
        .last()
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(hash)
}

pub async fn push(bin: &str, dir: &Path, remote: &str, branch: &str) -> BridgeResult<()> {
    let r = run(bin, Some(dir), ["push", remote, branch]).await?;
    if !r.ok() {
        return Err(BridgeError::VexCli(r.stderr.trim().to_string()));
    }
    Ok(())
}
