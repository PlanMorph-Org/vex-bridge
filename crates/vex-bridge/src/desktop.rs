//! Native standalone desktop window for Vex Atlas.
//!
//! The window hosts the system webview (WebView2 on Windows, WKWebView on
//! macOS, WebKitGTK on Linux) pointed at the local daemon's `/ui` page. The
//! daemon bakes the access token directly into that page, so the webview needs
//! no extra authentication. All of the existing Three.js / web-ifc viewer code
//! runs unchanged inside the webview.
//!
//! A tiny JS->Rust IPC bridge exposes native capabilities the webview can't
//! provide — currently a native folder picker (used by the "Add project" flow)
//! and opening account-pairing links in the user's real browser.

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Icon, WindowBuilder};
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
    crate::daemon_supervisor::ensure_daemon_ready(&paths, cfg.port)?;
    let url = dashboard_url(cfg.port, std::env::args().nth(1).as_deref());
    open_desktop_window(&url)
}

fn open_desktop_window(url: &str) -> BridgeResult<()> {
    run_native_window(url)
}

/// Build the native window + webview and run the event loop. On success this
/// never returns (the event loop drives the app until the window closes and the
/// process exits). It returns `Err` only when the window/webview cannot be
/// created; desktop startup deliberately never falls back to an external
/// browser.
fn run_native_window(url: &str) -> BridgeResult<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window_icon = app_icon()?;

    let window = WindowBuilder::new()
        .with_title("Vex Atlas")
        .with_inner_size(LogicalSize::new(1280.0, 820.0))
        .with_min_inner_size(LogicalSize::new(960.0, 640.0))
        .with_resizable(true)
        .with_window_icon(Some(window_icon))
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
            Event::UserEvent(UserEvent::OpenExternal { url })
                if url.starts_with("http://") || url.starts_with("https://") =>
            {
                let _ = open::that(&url);
            }
            _ => {}
        }
    });
}

/// Show startup failures even when Windows suppresses the console for the
/// desktop executable.
pub fn show_startup_error(message: &str) {
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Vex Atlas could not start")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

/// Show a clear native failure dialog after the desktop window's event loop
/// panics, instead of letting the windowed process disappear with no visible
/// explanation. `report_path` is the on-disk crash report
/// [`crate::crash_report::install`] wrote for this panic, if the write
/// succeeded.
pub fn show_crash_dialog(report_path: Option<std::path::PathBuf>) {
    let description = match report_path {
        Some(path) => format!(
            "Vex Atlas stopped unexpectedly and needs to close.\n\nA crash report was saved to:\n{}",
            path.display()
        ),
        None => "Vex Atlas stopped unexpectedly and needs to close. No crash report could be saved."
            .to_string(),
    };
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Vex Atlas stopped unexpectedly")
        .set_description(&description)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

fn app_icon() -> BridgeResult<Icon> {
    let (rgba, width, height) = crate::desktop_assets::vex_tray_icon_rgba()?;
    Icon::from_rgba(rgba, width, height)
        .map_err(|error| BridgeError::Config(format!("could not create application icon: {error}")))
}

fn dashboard_url(port: u16, requested_url: Option<&str>) -> String {
    let dashboard = format!("http://127.0.0.1:{port}/ui");
    requested_url
        .filter(|url| is_dashboard_url(url, &dashboard))
        .map(str::to_owned)
        .unwrap_or(dashboard)
}

fn is_dashboard_url(url: &str, dashboard: &str) -> bool {
    url.strip_prefix(dashboard).is_some_and(|suffix| {
        suffix.is_empty() || suffix.starts_with('?') || suffix.starts_with('#')
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_local_dashboard_by_default() {
        assert_eq!(dashboard_url(7878, None), "http://127.0.0.1:7878/ui");
    }

    #[test]
    fn preserves_local_dashboard_deep_links() {
        assert_eq!(
            dashboard_url(
                7878,
                Some("http://127.0.0.1:7878/ui?project=example&commit=abc")
            ),
            "http://127.0.0.1:7878/ui?project=example&commit=abc"
        );
    }

    #[test]
    fn rejects_non_dashboard_startup_urls() {
        for url in [
            "https://example.com",
            "http://127.0.0.1:7879/ui",
            "http://127.0.0.1:7878/v1/health",
            "http://127.0.0.1:7878/ui-not-dashboard",
        ] {
            assert_eq!(dashboard_url(7878, Some(url)), "http://127.0.0.1:7878/ui");
        }
    }
}
