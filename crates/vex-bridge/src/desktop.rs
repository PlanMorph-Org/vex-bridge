//! Native standalone desktop window for Vex Atlas.
//!
//! The window hosts the system webview (WebView2 on Windows, WKWebView on
//! macOS, WebKitGTK on Linux) pointed at the local daemon's `/ui` page. The
//! daemon bakes the access token directly into that page, so the webview needs
//! no extra authentication. All of the existing Three.js / web-ifc viewer code
//! runs unchanged inside the webview.
//!
//! A tiny JS->Rust IPC bridge exposes native capabilities the browser can't
//! provide — currently a native folder picker (used by the "Add project" flow)
//! and opening links in the user's real browser (used by account pairing).

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

use crate::config::{Config, Paths};
use crate::errors::{BridgeError, BridgeResult};

/// Custom events posted from the webview's IPC handler onto the event loop so
/// native work (dialogs, opening the system browser) happens on the main
/// thread and can call back into the webview.
#[derive(Debug, Clone)]
enum UserEvent {
    /// Open a native folder picker and return the chosen path to the webview.
    PickFolder { request_id: String },
    /// Open a URL in the user's default system browser.
    OpenExternal { url: String },
}

pub fn run() -> BridgeResult<()> {
    let paths = Paths::discover()?;
    paths.ensure_dirs()?;
    let cfg = Config::load_or_default(&paths)?;
    ensure_daemon(&paths, cfg.port)?;
    let url = std::env::args()
        .nth(1)
        .filter(|value| value.starts_with("http://127.0.0.1:"))
        .unwrap_or_else(|| format!("http://127.0.0.1:{}/ui", cfg.port));
    open_desktop_window(&url)
}

/// Ensure a *compatible* daemon is running before we attach the UI.
///
/// A common failure after an in-place update was a stale daemon from the
/// previous build still holding the port: the new desktop window would attach
/// to old code and behave inconsistently (mismatched API contract, missing
/// routes). Here we treat a version mismatch as "unhealthy": we ask the stale
/// daemon to shut down cleanly, wait for the port to free, then launch a fresh
/// daemon built from this binary's sibling. Self-healing instead of requiring
/// an uninstall/reinstall.
fn ensure_daemon(paths: &Paths, port: u16) -> BridgeResult<()> {
    let want = env!("CARGO_PKG_VERSION");
    match daemon_version(port) {
        // Healthy and matching: nothing to do.
        Some(version) if version == want => return Ok(()),
        // Healthy but stale: retire it deterministically before relaunching.
        Some(version) => {
            tracing::warn!(
                running = %version,
                expected = %want,
                "daemon version mismatch; requesting clean restart"
            );
            request_shutdown(paths, port);
            wait_for_port_free(port, Duration::from_secs(5));
        }
        // Not running (or not answering): just start one.
        None => {}
    }

    start_daemon()?;
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        match daemon_version(port) {
            Some(version) if version == want => return Ok(()),
            _ => std::thread::sleep(Duration::from_millis(250)),
        }
    }
    Ok(())
}

/// Probe `/v1/health` and return the daemon's reported version, or `None` if
/// nothing healthy is answering on the port.
fn daemon_version(port: u16) -> Option<String> {
    let body = health_body(port)?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    value
        .get("version")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// Issue a raw `GET /v1/health` and return the JSON response body, if the
/// daemon answered `200 OK`.
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
    // Body follows the blank line separating headers from payload.
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.trim().to_string())
}

/// Best-effort: ask a stale daemon to shut down gracefully via the token-gated
/// shutdown endpoint. We read the access token the daemon wrote to the per-user
/// data dir (same user, same machine) to authenticate.
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

/// Poll until the daemon stops answering on the port (it has shut down) or the
/// deadline passes.
fn wait_for_port_free(port: u16, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if health_body(port).is_none() {
            return;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
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
    match run_native_window(url) {
        Ok(()) => Ok(()),
        Err(error) => {
            tracing::warn!(error = %error, "native webview unavailable; falling back to browser");
            open_in_browser(url)
        }
    }
}

/// Build the native window + webview and run the event loop. On success this
/// never returns (the event loop drives the app until the window closes and the
/// process exits). It returns `Err` only when the window/webview cannot be
/// created, so the caller can fall back to a browser window.
fn run_native_window(url: &str) -> BridgeResult<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Vex Atlas")
        .with_inner_size(LogicalSize::new(1280.0, 820.0))
        .with_min_inner_size(LogicalSize::new(960.0, 640.0))
        .build(&event_loop)
        .map_err(|error| BridgeError::Config(format!("could not create window: {error}")))?;

    let ipc_proxy = proxy.clone();
    let init_script = include_str!("desktop_bridge.js");

    let builder = WebViewBuilder::new()
        .with_url(url)
        .with_initialization_script(init_script)
        .with_ipc_handler(move |request| {
            handle_ipc(request.body().as_str(), &ipc_proxy);
        });

    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    let webview = builder
        .build(&window)
        .map_err(|error| BridgeError::Config(format!("could not create webview: {error}")))?;

    // On Linux the webview must be attached to the GTK window's child container.
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().ok_or_else(|| {
            BridgeError::Config("could not access window container for webview".into())
        })?;
        builder
            .build_gtk(vbox)
            .map_err(|error| BridgeError::Config(format!("could not create webview: {error}")))?
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::UserEvent(UserEvent::PickFolder { request_id }) => {
                let chosen = rfd::FileDialog::new()
                    .set_title("Choose a project folder")
                    .pick_folder()
                    .map(|path| path.to_string_lossy().to_string());
                let payload = serde_json::json!({
                    "requestId": request_id,
                    "path": chosen,
                });
                let script = format!(
                    "window.__vexNative && window.__vexNative._onFolderPicked({});",
                    payload
                );
                let _ = webview.evaluate_script(&script);
            }
            Event::UserEvent(UserEvent::OpenExternal { url }) => {
                if url.starts_with("http://") || url.starts_with("https://") {
                    let _ = open::that(&url);
                }
            }
            _ => {}
        }
    });
}

/// Parse a JSON IPC message from the webview and forward it onto the event loop.
fn handle_ipc(body: &str, proxy: &EventLoopProxy<UserEvent>) {
    let Ok(message) = serde_json::from_str::<serde_json::Value>(body) else {
        return;
    };
    match message.get("type").and_then(|value| value.as_str()) {
        Some("pickFolder") => {
            let request_id = message
                .get("requestId")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            let _ = proxy.send_event(UserEvent::PickFolder { request_id });
        }
        Some("openExternal") => {
            if let Some(url) = message.get("url").and_then(|value| value.as_str()) {
                let _ = proxy.send_event(UserEvent::OpenExternal {
                    url: url.to_string(),
                });
            }
        }
        _ => {}
    }
}

/// Last-resort fallback when no native webview is available: open the UI in the
/// system browser (app-mode if a Chromium browser is installed).
fn open_in_browser(url: &str) -> BridgeResult<()> {
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
