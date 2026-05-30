use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};
use vex_bridge_protocol as proto;

use crate::config::{Config, Paths};
use crate::device::default_device_label;
use crate::errors::{BridgeError, BridgeResult};

const MENU_OPEN: &str = "open-dashboard";
const MENU_SETUP: &str = "open-setup";
const MENU_PAIR: &str = "pair-device";
const MENU_REFRESH: &str = "refresh-status";
const MENU_QUIT: &str = "quit-tray";

#[derive(Debug)]
enum UserEvent {
    Menu(MenuEvent),
    Tray(TrayIconEvent),
}

#[derive(Debug, Clone)]
struct TrayState {
    port: u16,
    dashboard_url: String,
    health_url: String,
    watch_url: String,
    activity_url: String,
    access_token_file: PathBuf,
    last_health: Option<HealthSnapshot>,
    last_watch: Option<proto::WatchStatus>,
    latest_activity: Option<proto::ActivityEvent>,
    last_seen_activity_id: Option<String>,
    last_notified_activity_id: Option<String>,
}

#[derive(Debug, Clone)]
struct HealthSnapshot {
    paired: bool,
    vex_version: Option<String>,
}

pub fn run() -> BridgeResult<()> {
    let paths = Paths::discover()?;
    paths.ensure_dirs()?;
    let cfg = Config::load_or_default(&paths)?;
    let mut state = TrayState::new(cfg.port, &paths);

    if state.probe_health().is_none() {
        let _ = start_daemon();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && state.probe_health().is_none() {
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    state.refresh_from_daemon();
    state.last_seen_activity_id = state.latest_activity.as_ref().map(|event| event.id.clone());
    state.last_notified_activity_id = state.last_seen_activity_id.clone();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let menu = Menu::new();
    let status = MenuItem::with_id("status", state.status_text(), false, None);
    let inbox = MenuItem::with_id("inbox", state.inbox_text(), false, None);
    let activity = MenuItem::with_id("activity", state.activity_text(), false, None);
    let open = MenuItem::with_id(MENU_OPEN, "Open Dashboard", true, None);
    let setup = MenuItem::with_id(MENU_SETUP, "Choose IFC Inbox", true, None);
    let pair = MenuItem::with_id(MENU_PAIR, "Pair Device", true, None);
    let refresh = MenuItem::with_id(MENU_REFRESH, "Refresh Status", true, None);
    let quit = MenuItem::with_id(MENU_QUIT, "Quit Tray", true, None);
    let sep_one = PredefinedMenuItem::separator();
    let sep_two = PredefinedMenuItem::separator();
    menu.append_items(&[
        &status, &inbox, &activity, &sep_one, &open, &setup, &pair, &refresh, &sep_two, &quit,
    ])
    .map_err(|error| BridgeError::Config(format!("tray menu failed: {error}")))?;

    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Menu(event));
    }));

    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Tray(event));
    }));

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Vex Atlas")
        .with_icon(vex_icon()?)
        .build()
        .map_err(|error| BridgeError::Config(format!("tray icon failed: {error}")))?;

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        let _keep_alive = &tray;

        match event {
            Event::NewEvents(StartCause::Init) => {
                update_menu(&state, &tray, &status, &inbox, &activity);
            }
            Event::UserEvent(UserEvent::Tray(_event)) => {
                open_dashboard(&state.latest_activity_url());
            }
            Event::UserEvent(UserEvent::Menu(event)) => match event.id.as_ref() {
                MENU_OPEN => open_dashboard(&state.latest_activity_url()),
                MENU_SETUP => open_dashboard(&state.dashboard_url),
                MENU_PAIR => pair_device(),
                MENU_REFRESH => {
                    if let Some(event) = state.refresh_from_daemon() {
                        notify_activity(&event, &state.activity_url_for(&event));
                    }
                    update_menu(&state, &tray, &status, &inbox, &activity);
                }
                MENU_QUIT => *control_flow = ControlFlow::Exit,
                _ => {}
            },
            Event::MainEventsCleared => {
                if let Some(event) = state.refresh_from_daemon() {
                    notify_activity(&event, &state.activity_url_for(&event));
                }
                update_menu(&state, &tray, &status, &inbox, &activity);
                *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_secs(15));
            }
            _ => {}
        }
    });
}

impl TrayState {
    fn new(port: u16, paths: &Paths) -> Self {
        Self {
            port,
            dashboard_url: format!("http://127.0.0.1:{port}/ui"),
            health_url: format!("http://127.0.0.1:{port}/v1/health"),
            watch_url: format!("http://127.0.0.1:{port}/v1/watch/status"),
            activity_url: format!("http://127.0.0.1:{port}/v1/activity/recent"),
            access_token_file: paths.access_token_file.clone(),
            last_health: None,
            last_watch: None,
            latest_activity: None,
            last_seen_activity_id: None,
            last_notified_activity_id: None,
        }
    }

    fn refresh_from_daemon(&mut self) -> Option<proto::ActivityEvent> {
        self.probe_health();
        self.probe_watch_status();
        self.probe_activity()
    }

    fn status_text(&self) -> String {
        let watching = self
            .last_watch
            .as_ref()
            .map(|watch| {
                format!(
                    " / {}/{} watching",
                    watch.active_watchers, watch.configured_projects
                )
            })
            .unwrap_or_default();
        match &self.last_health {
            Some(health) if health.paired => format!(
                "Vex is running / paired / vex {}{}",
                health.vex_version.as_deref().unwrap_or("unknown"),
                watching
            ),
            Some(health) => format!(
                "Vex is running / not paired / vex {}{}",
                health.vex_version.as_deref().unwrap_or("unknown"),
                watching
            ),
            None => format!("Vex daemon is not reachable on port {}", self.port),
        }
    }

    fn inbox_text(&self) -> String {
        let Some(watch) = &self.last_watch else {
            return "Inbox: loading".to_string();
        };
        let active: Vec<_> = watch
            .projects
            .iter()
            .filter(|project| project.active)
            .collect();
        match active.as_slice() {
            [] => "Inbox: no active watcher".to_string(),
            [project] => format!("Inbox: {}", shorten_path(&project.local_path)),
            projects => format!("Inboxes: {} active", projects.len()),
        }
    }

    fn activity_text(&self) -> String {
        match &self.latest_activity {
            Some(event) => format!("Last: {}", event.message),
            None => "Last: no activity yet".to_string(),
        }
    }

    fn probe_health(&mut self) -> Option<HealthSnapshot> {
        let response = match http_get(&self.health_url, Duration::from_secs(2), None) {
            Ok(response) => response,
            Err(_) => {
                self.last_health = None;
                return None;
            }
        };
        let paired = json_bool(&response, "paired").unwrap_or(false);
        let vex_version = json_string(&response, "vex_version");
        let health = HealthSnapshot {
            paired,
            vex_version,
        };
        self.last_health = Some(health.clone());
        Some(health)
    }

    fn probe_watch_status(&mut self) -> Option<proto::WatchStatus> {
        let token = self.access_token()?;
        let response = http_get(&self.watch_url, Duration::from_secs(2), Some(&token)).ok()?;
        let watch = serde_json::from_str::<proto::WatchStatus>(&response).ok()?;
        self.last_watch = Some(watch.clone());
        Some(watch)
    }

    fn probe_activity(&mut self) -> Option<proto::ActivityEvent> {
        let token = self.access_token()?;
        let response = http_get(&self.activity_url, Duration::from_secs(2), Some(&token)).ok()?;
        let activity = serde_json::from_str::<proto::RecentActivityResponse>(&response).ok()?;
        let latest = activity.events.first()?.clone();
        let notify_event = activity
            .events
            .iter()
            .take_while(|event| self.last_seen_activity_id.as_deref() != Some(event.id.as_str()))
            .find(|event| should_notify(event))
            .cloned();
        self.latest_activity = Some(latest.clone());
        self.last_seen_activity_id = Some(latest.id.clone());
        if let Some(event) = notify_event
            .filter(|event| self.last_notified_activity_id.as_deref() != Some(event.id.as_str()))
        {
            self.last_notified_activity_id = Some(event.id.clone());
            Some(event)
        } else {
            None
        }
    }

    fn access_token(&self) -> Option<String> {
        std::fs::read_to_string(&self.access_token_file)
            .ok()
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
    }

    fn latest_activity_url(&self) -> String {
        self.latest_activity
            .as_ref()
            .map(|event| self.activity_url_for(event))
            .unwrap_or_else(|| self.dashboard_url.clone())
    }

    fn activity_url_for(&self, event: &proto::ActivityEvent) -> String {
        let mut url = format!(
            "{}?project={}",
            self.dashboard_url,
            url_component(&event.project_id)
        );
        if let Some(commit) = event.commit_hash.as_deref() {
            url.push_str("&commit=");
            url.push_str(&url_component(commit));
        }
        url
    }
}

fn update_menu(
    state: &TrayState,
    tray: &tray_icon::TrayIcon,
    status: &MenuItem,
    inbox: &MenuItem,
    activity: &MenuItem,
) {
    let status_text = state.status_text();
    status.set_text(&status_text);
    inbox.set_text(state.inbox_text());
    activity.set_text(state.activity_text());
    let _ = tray.set_tooltip(Some(status_text));
}

fn open_dashboard(url: &str) {
    if let Err(error) = open::that(url) {
        eprintln!("could not open dashboard: {error}");
    }
}

fn should_notify(event: &proto::ActivityEvent) -> bool {
    matches!(
        event.kind,
        proto::ActivityKind::CommitCreated | proto::ActivityKind::Error
    )
}

fn notify_activity(event: &proto::ActivityEvent, _url: &str) {
    let title = match event.kind {
        proto::ActivityKind::CommitCreated => "Vex Atlas - New Commit",
        proto::ActivityKind::Error => "Vex Atlas - Needs Attention",
        _ => "Vex Atlas",
    };
    let body = match event.kind {
        proto::ActivityKind::CommitCreated => format!(
            "{}\n{}",
            event
                .project_name
                .as_deref()
                .unwrap_or(event.project_id.as_str()),
            event.message
        ),
        proto::ActivityKind::Error => event
            .detail
            .as_deref()
            .unwrap_or(event.message.as_str())
            .to_string(),
        _ => event.message.clone(),
    };
    send_notification(title, &body);
}

#[cfg(target_os = "macos")]
fn send_notification(title: &str, body: &str) {
    let _ = Command::new("osascript")
        .arg("-e")
        .arg("on run argv")
        .arg("-e")
        .arg("display notification (item 2 of argv) with title (item 1 of argv)")
        .arg("-e")
        .arg("end run")
        .arg(title)
        .arg(body)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

#[cfg(target_os = "windows")]
fn send_notification(title: &str, body: &str) {
    let script = r#"
param($Title, $Body)
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$template = [Windows.UI.Notifications.ToastTemplateType]::ToastText02
$xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent($template)
$text = $xml.GetElementsByTagName('text')
$text.Item(0).AppendChild($xml.CreateTextNode($Title)) | Out-Null
$text.Item(1).AppendChild($xml.CreateTextNode($Body)) | Out-Null
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Vex Atlas').Show($toast)
"#;
    let _ = Command::new("powershell.exe")
        .arg("-NoProfile")
        .arg("-WindowStyle")
        .arg("Hidden")
        .arg("-Command")
        .arg(script)
        .arg(title)
        .arg(body)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn send_notification(title: &str, body: &str) {
    if Command::new("notify-send")
        .arg("--app-name")
        .arg("Vex Atlas")
        .arg(title)
        .arg(body)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_err()
    {
        eprintln!("{title}: {body}");
    }
}

fn pair_device() {
    let Ok(exe) = bridge_exe() else {
        eprintln!("could not find vex-bridge executable");
        return;
    };
    if let Err(error) = Command::new(exe)
        .arg("pair")
        .arg("--device-label")
        .arg(device_label())
        .arg("--open-browser")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        eprintln!("could not start pairing: {error}");
    }
}

fn start_daemon() -> BridgeResult<()> {
    let exe = bridge_exe()?;
    Command::new(exe)
        .arg("start")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(BridgeError::Io)
}

fn bridge_exe() -> BridgeResult<std::path::PathBuf> {
    let mut exe = std::env::current_exe().map_err(BridgeError::Io)?;
    exe.set_file_name(if cfg!(windows) {
        "vex-bridge.exe"
    } else {
        "vex-bridge"
    });
    if exe.is_file() {
        return Ok(exe);
    }
    Ok(std::path::PathBuf::from(if cfg!(windows) {
        "vex-bridge.exe"
    } else {
        "vex-bridge"
    }))
}

fn device_label() -> String {
    default_device_label()
}

fn http_get(url: &str, timeout: Duration, token: Option<&str>) -> std::io::Result<String> {
    let rest = url
        .strip_prefix("http://127.0.0.1:")
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "unsupported url"))?;
    let (port, path) = rest
        .split_once('/')
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing path"))?;
    let mut stream =
        std::net::TcpStream::connect(("127.0.0.1", port.parse::<u16>().unwrap_or(7878)))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    use std::io::{Read, Write};
    let auth = token
        .map(|token| format!("X-Vex-Bridge-Token: {token}\r\n"))
        .unwrap_or_default();
    write!(
        stream,
        "GET /{path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{auth}Connection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let (_, body) = response.split_once("\r\n\r\n").unwrap_or(("", ""));
    Ok(body.to_string())
}

fn shorten_path(path: &str) -> String {
    let home = std::env::var("HOME").ok();
    let shortened = home
        .as_deref()
        .and_then(|home| path.strip_prefix(home).map(|rest| format!("~{rest}")))
        .unwrap_or_else(|| path.to_string());
    const MAX_PATH_CHARS: usize = 56;
    if shortened.chars().count() <= MAX_PATH_CHARS {
        shortened
    } else {
        let tail: String = shortened
            .chars()
            .rev()
            .take(MAX_PATH_CHARS - 3)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        format!("...{tail}")
    }
}

fn url_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn json_bool(body: &str, key: &str) -> Option<bool> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get(key)?
        .as_bool()
}

fn json_string(body: &str, key: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()?
        .get(key)?
        .as_str()
        .map(str::to_string)
}

fn vex_icon() -> BridgeResult<Icon> {
    let (rgba, width, height) = crate::desktop_assets::vex_tray_icon_rgba()?;
    Icon::from_rgba(rgba, width, height)
        .map_err(|error| BridgeError::Config(format!("invalid tray icon: {error}")))
}
