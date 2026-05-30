use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::{Config, Paths};
use crate::errors::{BridgeError, BridgeResult};

pub fn run() -> BridgeResult<()> {
    let paths = Paths::discover()?;
    paths.ensure_dirs()?;
    let cfg = Config::load_or_default(&paths)?;
    ensure_daemon(cfg.port)?;
    let url = std::env::args()
        .nth(1)
        .filter(|value| value.starts_with("http://127.0.0.1:"))
        .unwrap_or_else(|| format!("http://127.0.0.1:{}/ui", cfg.port));
    open_desktop_window(&url)
}

fn ensure_daemon(port: u16) -> BridgeResult<()> {
    if daemon_is_healthy(port) {
        return Ok(());
    }

    start_daemon()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if daemon_is_healthy(port) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}

fn daemon_is_healthy(port: u16) -> bool {
    let Ok(mut stream) = std::net::TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    use std::io::{Read, Write};
    if write!(
        stream,
        "GET /v1/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .is_err()
    {
        return false;
    }
    let mut response = String::new();
    stream.read_to_string(&mut response).is_ok() && response.starts_with("HTTP/1.1 200")
}

fn start_daemon() -> BridgeResult<()> {
    let mut command = Command::new(sibling_exe("vex-bridge"));
    command
        .arg("start")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console_window(&mut command);
    command.spawn().map(|_| ()).map_err(BridgeError::Io)
}

fn open_desktop_window(url: &str) -> BridgeResult<()> {
    if try_app_window(url) {
        return Ok(());
    }
    open::that(url).map_err(|error| BridgeError::Config(format!("could not open Vex UI: {error}")))
}

fn try_app_window(url: &str) -> bool {
    for browser in browser_candidates() {
        if !browser.is_file() {
            continue;
        }
        if Command::new(browser)
            .arg(format!("--app={url}"))
            .arg("--new-window")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
        {
            return true;
        }
    }
    false
}

#[cfg(target_os = "windows")]
fn hide_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_console_window(_command: &mut Command) {}

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
fn browser_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for root in [
        std::env::var_os("ProgramFiles"),
        std::env::var_os("ProgramFiles(x86)"),
        std::env::var_os("LocalAppData"),
    ]
    .into_iter()
    .flatten()
    {
        let root = PathBuf::from(root);
        candidates.push(root.join("Microsoft/Edge/Application/msedge.exe"));
        candidates.push(root.join("Google/Chrome/Application/chrome.exe"));
    }
    candidates
}

#[cfg(target_os = "macos")]
fn browser_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
    ]
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn browser_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/bin/microsoft-edge"),
        PathBuf::from("/usr/bin/google-chrome"),
        PathBuf::from("/usr/bin/chromium"),
        PathBuf::from("/usr/bin/chromium-browser"),
    ]
}
