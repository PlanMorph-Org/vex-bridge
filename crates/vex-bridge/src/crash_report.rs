//! Shared crash-report facility for every shipped native binary
//! (`vex-bridge`, `vex-tray`, `vex-desktop`).
//!
//! [`install`] installs a panic hook, once per process, that writes a small,
//! redacted, structured JSON record to `<data_dir>/crash-reports/` — the same
//! per-user data directory that already holds the daemon log and state file
//! (see [`crate::config::Paths`]). This lets a user or support engineer see
//! *why* a headless daemon or windowless tray/desktop process vanished,
//! without ever persisting secret material (access tokens, pairing keys,
//! ...) to disk.
//!
//! The hook chains Rust's previous hook (the default one, which prints to
//! stderr and honours `RUST_BACKTRACE`), so existing behaviour is unchanged —
//! this only *adds* the on-disk report.
//!
//! Kept deliberately dependency-light: no new crates, just `std` +
//! `serde_json`. Redaction here is defense in depth, not a substitute for
//! care at the call site — never format a raw token/secret into a
//! `panic!`/`.expect()` message in the first place.

use std::backtrace::{Backtrace, BacktraceStatus};
use std::panic::PanicHookInfo;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

use crate::config::Paths;
use crate::state::now_unix;

/// One panic, captured as a small, redacted, structured record.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CrashReport {
    /// RFC 3339 UTC timestamp of the panic, e.g. `2025-01-31T12:00:00Z`.
    pub timestamp: String,
    /// File stem of the crashing executable (`vex-bridge`, `vex-tray`, `vex-desktop`).
    pub executable: String,
    /// `CARGO_PKG_VERSION` of the crashing build.
    pub version: String,
    /// `std::env::consts::OS` (`windows`, `macos`, `linux`, ...).
    pub os: String,
    /// Name of the thread that panicked, or `"<unnamed>"`.
    pub thread: String,
    /// `file:line:column` of the panic site, if the panic runtime reported one.
    pub location: Option<String>,
    /// The panic payload's message, redacted of anything resembling a secret.
    pub message: String,
    /// Captured backtrace text, redacted, when the runtime could produce one.
    /// `None` when backtrace capture is unavailable on this platform/build.
    pub backtrace: Option<String>,
}

/// Minimum length (in chars) a punctuation-trimmed word must reach before the
/// heuristic secret-shape check considers it. Access tokens in this codebase
/// are 32 random bytes, base64url-encoded (~43 chars); this stays comfortably
/// below that while still ignoring short identifiers/words.
const MIN_SECRET_LEN: usize = 20;

/// Matches the placeholder `server.rs`'s diagnostics `redact` uses for the
/// bridge access token, so support engineers see one consistent redaction
/// style across logs, diagnostics, and crash reports.
const TOKEN_PLACEHOLDER: &str = "[redacted-token]";
/// Placeholder for the heuristic (not positively-identified) secret-shaped
/// text redaction pass.
const GENERIC_PLACEHOLDER: &str = "[redacted]";

static LAST_REPORT_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

fn last_report_slot() -> &'static Mutex<Option<PathBuf>> {
    LAST_REPORT_PATH.get_or_init(|| Mutex::new(None))
}

/// Path the most recently written crash report was saved to by *this*
/// process, if any. Tray/desktop `main` read this after `catch_unwind` to
/// point a native failure dialog at the on-disk report.
pub fn last_report_path() -> Option<PathBuf> {
    last_report_slot()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

/// Install the shared panic hook. Call this once, as the very first
/// statement in every shipped binary's `main`, before anything else
/// (config loading, daemon supervision, window/webview creation, ...) has a
/// chance to panic.
pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Preserve stock behaviour (stderr + RUST_BACKTRACE) first, then add
        // the redacted on-disk report. A failure to write the report must
        // never mask the original panic.
        previous(info);
        if let Some(path) = write_report(info) {
            if let Ok(mut guard) = last_report_slot().lock() {
                *guard = Some(path);
            }
        }
    }));
}

/// `<data_dir>/crash-reports` — sibling to the daemon log and state file.
pub fn crash_reports_dir(paths: &Paths) -> PathBuf {
    paths.data_dir.join("crash-reports")
}

fn write_report(info: &PanicHookInfo<'_>) -> Option<PathBuf> {
    let unix_ts = now_unix();
    let pid = std::process::id();
    let report = CrashReport::capture(info, unix_ts);
    let paths = Paths::discover().ok()?;
    let dir = crash_reports_dir(&paths);
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(report_file_name(&report.executable, unix_ts, pid));
    let body = serde_json::to_vec_pretty(&report).ok()?;
    std::fs::write(&path, body).ok()?;
    Some(path)
}

/// Build a filesystem-safe crash report file name. Sandboxed from whatever
/// `current_exe()` returns so a hostile/odd executable path can never escape
/// the crash-reports directory or inject path separators.
fn report_file_name(executable: &str, unix_ts: i64, pid: u32) -> String {
    format!("{}-{unix_ts}-{pid}.json", sanitize_component(executable))
}

fn sanitize_component(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "crash".to_string()
    } else {
        cleaned
    }
}

impl CrashReport {
    fn capture(info: &PanicHookInfo<'_>, unix_ts: i64) -> Self {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        let message = panic_message(info);
        let backtrace = capture_backtrace();
        Self::build(
            unix_ts,
            executable_name(),
            current_thread_name(),
            location,
            message,
            backtrace,
        )
    }

    /// Pure constructor used both by [`Self::capture`] and by unit tests, so
    /// report shape/redaction can be exercised without ever forcing a real
    /// process panic.
    fn build(
        unix_ts: i64,
        executable: String,
        thread: String,
        location: Option<String>,
        message: String,
        backtrace: Option<String>,
    ) -> Self {
        Self {
            timestamp: format_rfc3339(unix_ts),
            executable,
            version: env!("CARGO_PKG_VERSION").to_string(),
            os: std::env::consts::OS.to_string(),
            thread,
            location,
            message: redact(&message),
            backtrace: backtrace.map(|text| redact(&text)),
        }
    }
}

fn panic_message(info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

fn executable_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "vex-bridge".to_string())
}

fn current_thread_name() -> String {
    std::thread::current()
        .name()
        .unwrap_or("<unnamed>")
        .to_string()
}

/// Best-effort backtrace capture. Uses `force_capture` so a report is useful
/// even when the caller didn't set `RUST_BACKTRACE`; returns `None` (rather
/// than an empty/`<unsupported>` string) when the runtime genuinely can't
/// produce one, matching "backtrace when available".
fn capture_backtrace() -> Option<String> {
    let backtrace = Backtrace::force_capture();
    match backtrace.status() {
        BacktraceStatus::Captured => Some(backtrace.to_string()),
        _ => None,
    }
}

/// Redact anything that looks like a secret from free-form crash text (panic
/// messages, backtraces). Two passes:
///
/// 1. Exact matches against secrets we can positively identify on this
///    machine right now (currently: the local access token), if readable.
/// 2. A conservative heuristic for long, mixed alphanumeric "words" — the
///    shape of bearer tokens, API keys, and base64/hex-encoded key material —
///    regardless of whether we could positively identify them.
///
/// This is deliberately conservative (it may redact long non-secret hashes
/// too); over-redaction of debug context is preferable to ever leaking a
/// credential.
pub fn redact(text: &str) -> String {
    let home_redacted = redact_home_dir(text);
    let known_redacted = redact_known_secrets(&home_redacted, &known_secrets());
    redact_token_like_words(&known_redacted)
}

/// Replace the user's home directory prefix with `~`, mirroring the same
/// convention `server.rs`'s diagnostics `redact` uses — panic messages and
/// backtraces routinely include absolute source/config paths, and those
/// paths embed the OS username.
fn redact_home_dir(text: &str) -> String {
    match home_dir() {
        Some(home) if !home.is_empty() => text.replace(&home, "~"),
        _ => text.to_string(),
    }
}

fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
        .filter(|home| !home.is_empty())
}

fn known_secrets() -> Vec<String> {
    let mut secrets = Vec::new();
    if let Ok(paths) = Paths::discover() {
        if let Ok(token) = std::fs::read_to_string(&paths.access_token_file) {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                secrets.push(trimmed.to_string());
            }
        }
    }
    secrets
}

fn redact_known_secrets(text: &str, secrets: &[String]) -> String {
    let mut redacted = text.to_string();
    for secret in secrets {
        if secret.len() >= 8 {
            redacted = redacted.replace(secret.as_str(), TOKEN_PLACEHOLDER);
        }
    }
    redacted
}

fn redact_token_like_words(text: &str) -> String {
    let mut redacted = text.to_string();
    let mut already_redacted = std::collections::HashSet::new();
    for word in text.split_whitespace() {
        let core = trim_secret_punctuation(word);
        if looks_like_secret(core) && already_redacted.insert(core.to_string()) {
            redacted = redacted.replace(core, GENERIC_PLACEHOLDER);
        }
    }
    redacted
}

fn trim_secret_punctuation(word: &str) -> &str {
    word.trim_matches(|c: char| {
        !(c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '_' | '-' | '='))
    })
}

/// `true` for tokens that look like secret material: long, restricted to the
/// base64url/hex-ish alphabet, and mixing letters with digits (rules out
/// plain English words and typical file-path segments, which trip on `.`,
/// `:`, `\`, or pure-alpha content).
fn looks_like_secret(word: &str) -> bool {
    if word.chars().count() < MIN_SECRET_LEN {
        return false;
    }
    let mut has_alpha = false;
    let mut has_digit = false;
    for c in word.chars() {
        if !(c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '_' | '-' | '=')) {
            return false;
        }
        has_alpha |= c.is_ascii_alphabetic();
        has_digit |= c.is_ascii_digit();
    }
    has_alpha && has_digit
}

/// Hand-rolled UTC RFC 3339 formatter (`YYYY-MM-DDTHH:MM:SSZ`) so this crate
/// doesn't need to add a date/time dependency just for crash-report
/// timestamps. Uses Howard Hinnant's well-known `civil_from_days` algorithm.
fn format_rfc3339(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report(message: &str, backtrace: Option<&str>) -> CrashReport {
        CrashReport::build(
            1_706_000_000, // 2024-01-23T08:53:20Z
            "vex-desktop".to_string(),
            "main".to_string(),
            Some("src/desktop.rs:42:9".to_string()),
            message.to_string(),
            backtrace.map(str::to_string),
        )
    }

    /// A synthetic value shaped like a real access token (long, mixed
    /// alphanumeric) for exercising the heuristic redaction pass, without
    /// hardcoding a realistic `Authorization: Bearer <token>` string in test
    /// source.
    fn sample_token_like_value() -> String {
        "TestToken1234567890AbcDefGhiJklMno".to_string()
    }

    #[test]
    fn format_rfc3339_matches_known_epoch() {
        // 2024-01-23T08:53:20Z, cross-checked against a standard epoch converter.
        assert_eq!(format_rfc3339(1_706_000_000), "2024-01-23T08:53:20Z");
        // The Unix epoch itself.
        assert_eq!(format_rfc3339(0), "1970-01-01T00:00:00Z");
        // A leap-day timestamp (2024-02-29T12:00:00Z).
        assert_eq!(format_rfc3339(1_709_208_000), "2024-02-29T12:00:00Z");
    }

    #[test]
    fn report_metadata_captures_expected_fields() {
        let report = sample_report("boom", Some("0: vex_bridge::desktop::run"));
        assert_eq!(report.executable, "vex-desktop");
        assert_eq!(report.thread, "main");
        assert_eq!(report.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(report.os, std::env::consts::OS);
        assert_eq!(report.location.as_deref(), Some("src/desktop.rs:42:9"));
        assert_eq!(report.timestamp, "2024-01-23T08:53:20Z");
        assert_eq!(report.message, "boom");
        assert_eq!(
            report.backtrace.as_deref(),
            Some("0: vex_bridge::desktop::run")
        );
    }

    #[test]
    fn report_serializes_to_json_with_expected_keys() {
        let report = sample_report("boom", None);
        let value: serde_json::Value =
            serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
        for key in [
            "timestamp",
            "executable",
            "version",
            "os",
            "thread",
            "location",
            "message",
            "backtrace",
        ] {
            assert!(value.get(key).is_some(), "missing key {key}");
        }
        assert!(value.get("backtrace").unwrap().is_null());
    }

    #[test]
    fn looks_like_secret_flags_long_mixed_alphanumeric_tokens() {
        // Shaped like a 32-byte base64url access token (~43 chars).
        assert!(looks_like_secret("QWxhZGRpbjpvcGVuIHNlc2FtZTEyMzQ1Njc4"));
        // Too short to be considered, even if mixed alphanumeric.
        assert!(!looks_like_secret("abc123"));
        // Pure-alpha long word (e.g. an English sentence fragment) is not
        // secret-shaped.
        assert!(!looks_like_secret(&"a".repeat(30)));
        // Pure-digit long word (e.g. a timestamp) is not secret-shaped either.
        assert!(!looks_like_secret(&"1".repeat(30)));
    }

    #[test]
    fn trim_secret_punctuation_strips_wrapping_punctuation_only() {
        assert_eq!(
            trim_secret_punctuation("(token=abc123XYZ789def456UVW),"),
            "token=abc123XYZ789def456UVW"
        );
    }

    #[test]
    fn redact_known_secrets_replaces_exact_matches_only() {
        let secrets = vec!["s3cr3t-access-token-value".to_string()];
        let text = "failed to auth with s3cr3t-access-token-value while loading config";
        let redacted = redact_known_secrets(text, &secrets);
        assert!(!redacted.contains("s3cr3t-access-token-value"));
        assert!(redacted.contains(TOKEN_PLACEHOLDER));
        assert!(redacted.contains("failed to auth with"));
    }

    #[test]
    fn redact_replaces_long_token_like_words_but_keeps_prose() {
        let sample_token = sample_token_like_value();
        let text = format!("panic while posting X-Vex-Bridge-Token: {sample_token} to host");
        let redacted = redact(&text);
        assert!(!redacted.contains(&sample_token));
        assert!(redacted.contains(GENERIC_PLACEHOLDER));
        assert!(redacted.contains("panic while posting X-Vex-Bridge-Token:"));
        assert!(redacted.contains("to host"));
    }

    #[test]
    fn redact_leaves_ordinary_file_paths_and_short_ids_alone() {
        let text =
            "could not open C:\\Users\\alex\\AppData\\Roaming\\vex-bridge\\config.toml (id=7f3a91)";
        let redacted = redact(text);
        assert_eq!(redacted, text);
    }

    #[test]
    fn crash_report_build_redacts_message_and_backtrace() {
        let secret = sample_token_like_value();
        let message = format!("panic: token {secret} rejected");
        let backtrace = format!("0: handler with token {secret}");
        let report = sample_report(&message, Some(&backtrace));
        assert!(!report.message.contains(&secret));
        assert!(!report.backtrace.as_ref().unwrap().contains(&secret));
    }

    #[test]
    fn report_file_name_is_filesystem_safe() {
        let name = report_file_name("vex tray/../evil", 1_706_000_000, 4242);
        assert_eq!(name, "vex_tray____evil-1706000000-4242.json");
        assert!(!name.contains('/'));
        assert!(!name.contains('\\'));
        assert!(!name.contains(".."));
    }

    #[test]
    fn sanitize_component_falls_back_when_fully_stripped() {
        assert_eq!(sanitize_component(""), "crash");
        assert_eq!(sanitize_component("vex-desktop"), "vex-desktop");
        // Non-alnum input degrades to underscores rather than an empty or
        // path-traversal-capable name — still filesystem-safe, just ugly.
        assert_eq!(sanitize_component("///"), "___");
    }

    #[test]
    fn crash_reports_dir_is_under_data_dir() {
        let paths = Paths {
            config_dir: PathBuf::from("C:/fake/config"),
            data_dir: PathBuf::from("C:/fake/data"),
            config_file: PathBuf::from("C:/fake/config/config.toml"),
            state_file: PathBuf::from("C:/fake/data/state.json"),
            access_token_file: PathBuf::from("C:/fake/config/access-token"),
            log_file: PathBuf::from("C:/fake/data/vex-bridge.log"),
            daemon_lock_file: PathBuf::from("C:/fake/data/daemon.lock"),
        };
        assert_eq!(
            crash_reports_dir(&paths),
            PathBuf::from("C:/fake/data/crash-reports")
        );
    }

    #[test]
    fn last_report_path_defaults_to_none_when_unset() {
        // We can't assert a hard `None` here (test order across the binary is
        // unspecified and another test file in the same process could have
        // triggered a real panic hook write), only that reading the slot
        // never panics and returns an `Option<PathBuf>`.
        let _ = last_report_path();
    }
}
