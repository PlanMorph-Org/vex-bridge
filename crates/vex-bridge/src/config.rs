//! Filesystem layout + user config (TOML).
//!
//! Locations follow the platform-native conventions via the `directories` crate:
//!
//! | Linux                                 | macOS                                          | Windows                                  |
//! |---------------------------------------|------------------------------------------------|------------------------------------------|
//! | `~/.config/vex-bridge/config.toml`    | `~/Library/Application Support/vex-bridge/…`   | `%APPDATA%\vex-bridge\config.toml`       |
//! | `~/.local/share/vex-bridge/state.json`| `~/Library/Application Support/vex-bridge/…`   | `%APPDATA%\vex-bridge\state.json`        |
//!
//! The access token (used by plugins to authenticate to the daemon) lives at
//! `<config_dir>/access-token` with mode `0600` on Unix.

use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::errors::{BridgeError, BridgeResult};

const QUALIFIER: &str = "com";
const ORG: &str = "Architur";
const APP: &str = "vex-bridge";

#[derive(Debug, Clone)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
    pub state_file: PathBuf,
    pub access_token_file: PathBuf,
    pub log_file: PathBuf,
    /// Advisory record of the running daemon (`{pid, version, port, started_at}`).
    /// The bound TCP port is the hard singleton; this file adds PID + version
    /// visibility so launchers can self-heal a stale daemon and the Repair flow
    /// can force-kill a hung one.
    pub daemon_lock_file: PathBuf,
}

impl Paths {
    pub fn discover() -> BridgeResult<Self> {
        let dirs = ProjectDirs::from(QUALIFIER, ORG, APP)
            .ok_or_else(|| BridgeError::Config("no platform home directory".into()))?;
        let config_dir = dirs.config_dir().to_path_buf();
        let data_dir = dirs.data_dir().to_path_buf();
        Ok(Self {
            config_file: config_dir.join("config.toml"),
            access_token_file: config_dir.join("access-token"),
            log_file: data_dir.join("vex-bridge.log"),
            state_file: data_dir.join("state.json"),
            daemon_lock_file: data_dir.join("daemon.lock"),
            config_dir,
            data_dir,
        })
    }

    pub fn ensure_dirs(&self) -> BridgeResult<()> {
        fs::create_dir_all(&self.config_dir)?;
        fs::create_dir_all(&self.data_dir)?;
        Ok(())
    }
}

/// User-editable settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Base URL of the architur API, e.g. `https://studio.planmorph.software`.
    ///
    /// NOTE: `planmorph.software` expired and is currently unregistered/parked.
    /// Until DNS is repointed, this defaults straight to the Azure Container
    /// Apps FQDN for the prototype deployment (`ca-vexatlas-api`). Swap back
    /// to the custom domain once it's re-registered and DNS is live.
    #[serde(default = "default_api_base")]
    pub api_base: String,

    /// Base URL of the browser-facing web app that serves the `/pair` page,
    /// e.g. `https://studio.planmorph.software`. Kept separate from
    /// `api_base` because on the Azure prototype the API and web frontend
    /// are two different Container Apps with different hostnames.
    #[serde(default = "default_web_base")]
    pub web_base: String,

    /// Host (no scheme) of the `vex-sshd` remote used for `git push`-style
    /// repo sync, e.g. `vex.planmorph.software` or a bare IP. Previously
    /// this was derived by string-mangling `api_base`'s subdomain, which
    /// only worked when API and SSH shared a root domain; the Azure
    /// prototype's API/SSH hosts are unrelated, so it's now explicit.
    #[serde(default = "default_vex_serve_host")]
    pub vex_serve_host: String,

    /// Path to the bundled `vex` binary. Defaults to "vex" (resolved on PATH).
    #[serde(default = "default_vex_bin")]
    pub vex_bin: String,

    /// HTTP listen port for the local daemon. Default 7878 (avoids common dev ports).
    #[serde(default = "default_port")]
    pub port: u16,

    /// Default author identity stamped into vex commits if a plugin omits it.
    #[serde(default)]
    pub default_author_name: Option<String>,
    #[serde(default)]
    pub default_author_email: Option<String>,

    /// Folders the daemon should auto-watch in Tier 3 mode. Each entry maps a
    /// project id to a local directory; any IFC file appearing under that
    /// directory triggers a commit + push.
    #[serde(default)]
    pub watch: Vec<WatchEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEntry {
    pub project_id: String,
    /// Architur repository GUID selected as this local folder's push target.
    #[serde(default)]
    pub cloud_project_id: Option<String>,
    pub path: String,
    /// File globs to commit. Defaults to "*.ifc".
    #[serde(default = "default_globs")]
    pub include: Vec<String>,
    /// Optional IFC project GlobalId routed to this Vex project.
    #[serde(default)]
    pub ifc_project_guid: Option<String>,
    /// Human project name shown in generated commit messages.
    #[serde(default)]
    pub project_name: Option<String>,
}

fn default_api_base() -> String {
    // planmorph.software expired and is parked; point at the Azure
    // prototype's API Container App directly until DNS is restored.
    "https://ca-vexatlas-api.calmtree-edee8174.northeurope.azurecontainerapps.io".into()
}
fn default_web_base() -> String {
    "https://ca-vexatlas-web.calmtree-edee8174.northeurope.azurecontainerapps.io".into()
}
fn default_vex_serve_host() -> String {
    // Stable Azure DNS label for aci-vex-serve (rg-vexatlas-prod-ne).
    "vexatlas-serve-ne.northeurope.azurecontainer.io".into()
}
fn default_vex_bin() -> String {
    bundled_vex_bin().unwrap_or_else(|| "vex".into())
}
fn default_port() -> u16 {
    7878
}
fn default_globs() -> Vec<String> {
    vec!["*.ifc".into()]
}

fn is_expired_planmorph_host(value: &str, subdomain: &str) -> bool {
    let trimmed = value.trim().trim_end_matches('/');
    matches!(
        trimmed,
        "https://planmorph.software" | "http://planmorph.software"
    ) || trimmed == format!("https://{subdomain}.planmorph.software")
        || trimmed == format!("http://{subdomain}.planmorph.software")
}

fn migrate_expired_endpoints(cfg: &mut Config) -> bool {
    let mut changed = false;
    if is_expired_planmorph_host(&cfg.api_base, "studio")
        || is_expired_planmorph_host(&cfg.api_base, "api")
    {
        cfg.api_base = default_api_base();
        changed = true;
    }
    if is_expired_planmorph_host(&cfg.web_base, "studio")
        || is_expired_planmorph_host(&cfg.web_base, "app")
    {
        cfg.web_base = default_web_base();
        changed = true;
    }
    if matches!(
        cfg.vex_serve_host.trim().trim_end_matches('/'),
        "vex.planmorph.software" | "planmorph.software" | "20.223.15.197" | "20.166.239.248"
    ) {
        cfg.vex_serve_host = default_vex_serve_host();
        changed = true;
    }
    changed
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_base: default_api_base(),
            web_base: default_web_base(),
            vex_serve_host: default_vex_serve_host(),
            vex_bin: default_vex_bin(),
            port: default_port(),
            default_author_name: None,
            default_author_email: None,
            watch: Vec::new(),
        }
    }
}

impl Config {
    pub fn load_or_default(paths: &Paths) -> BridgeResult<Self> {
        if !paths.config_file.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&paths.config_file)?;
        let mut cfg: Self = toml::from_str(&raw).map_err(|e| BridgeError::Config(e.to_string()))?;
        // Re-resolve the engine binary on every load. `save` always serialises
        // `vex_bin`, so a previously-persisted value — the bare "vex" placeholder
        // written on dev boxes, or an absolute path from an older install that no
        // longer exists after an update moved the app — would otherwise stick
        // forever, and every engine call would fail with a cryptic
        // "No such file or directory (os error 2)". Self-heal by preferring a
        // user-pinned path that still exists, else the bundled engine.
        cfg.vex_bin = resolve_vex_bin(&cfg.vex_bin);
        // Existing installs retain endpoint overrides in config.toml. Move
        // only the known expired PlanMorph defaults to the Azure deployment;
        // custom domains and self-hosted endpoints remain untouched.
        if migrate_expired_endpoints(&mut cfg) {
            cfg.save(paths)?;
        }
        Ok(cfg)
    }

    pub fn save(&self, paths: &Paths) -> BridgeResult<()> {
        paths.ensure_dirs()?;
        let body = toml::to_string_pretty(self).map_err(|e| BridgeError::Config(e.to_string()))?;
        fs::write(&paths.config_file, body)?;
        Ok(())
    }

    pub fn remove_watch(&mut self, project_id: &str) -> Option<WatchEntry> {
        self.watch
            .iter()
            .position(|watch| watch.project_id == project_id)
            .map(|index| self.watch.remove(index))
    }
}

fn bundled_vex_bin() -> Option<String> {
    bundled_vex_bin_next_to(&std::env::current_exe().ok()?)
}

/// Pick the engine binary to use given whatever was persisted in `config.toml`.
///
/// Order of preference:
/// 1. A non-placeholder path that points at an existing file — the user (or a
///    prior install) pinned a specific engine build, so honour it.
/// 2. The `vex`/`vex.exe` bundled next to the current executable — the normal
///    installed layout, and always version-matched to this bridge.
/// 3. The stored value as-is (e.g. bare `vex` resolved from `PATH` on a dev
///    machine), or `vex` when nothing was stored.
fn resolve_vex_bin(stored: &str) -> String {
    let trimmed = stored.trim();
    // An explicit path that still points at a real file wins — the user (or a
    // prior install) pinned a specific engine build.
    if !trimmed.is_empty() && Path::new(trimmed).is_file() {
        return trimmed.to_string();
    }
    // A bare command name the user chose (e.g. `vex-nightly`) is a PATH lookup
    // worth preserving — but not the default `vex` placeholder, which we'd
    // rather satisfy from the bundled engine when one is present.
    let looks_like_path = trimmed.contains('/') || trimmed.contains('\\');
    if !trimmed.is_empty() && trimmed != "vex" && !looks_like_path {
        return trimmed.to_string();
    }
    // Placeholder, blank, or a stale path that no longer exists: prefer the
    // bundled engine, else fall back to a bare `vex` PATH lookup.
    bundled_vex_bin().unwrap_or_else(|| "vex".to_string())
}

fn bundled_vex_bin_next_to(executable: &Path) -> Option<String> {
    let mut path = executable.to_path_buf();
    path.set_file_name(if cfg!(windows) { "vex.exe" } else { "vex" });
    path.is_file().then(|| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_case_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "vex-bridge-config-test-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn finds_bundled_vex_next_to_executable() {
        let dir = temp_case_dir("bundled");
        let executable = dir.join(if cfg!(windows) {
            "vex-bridge.exe"
        } else {
            "vex-bridge"
        });
        let vex = dir.join(if cfg!(windows) { "vex.exe" } else { "vex" });
        fs::write(&executable, b"").unwrap();
        fs::write(&vex, b"").unwrap();

        assert_eq!(
            bundled_vex_bin_next_to(&executable),
            Some(vex.to_string_lossy().to_string())
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn ignores_missing_bundled_vex() {
        let dir = temp_case_dir("missing");
        let executable = dir.join(if cfg!(windows) {
            "vex-bridge.exe"
        } else {
            "vex-bridge"
        });
        fs::write(&executable, b"").unwrap();

        assert_eq!(bundled_vex_bin_next_to(&executable), None);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resolve_vex_bin_keeps_existing_custom_path() {
        let dir = temp_case_dir("resolve-custom");
        let custom = dir.join(if cfg!(windows) {
            "engine.exe"
        } else {
            "engine"
        });
        fs::write(&custom, b"").unwrap();
        let custom_str = custom.to_string_lossy().to_string();

        // A pinned path that exists on disk is honoured verbatim.
        assert_eq!(resolve_vex_bin(&custom_str), custom_str);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resolve_vex_bin_drops_stale_absolute_path() {
        // An absolute path from an older install that no longer exists must not
        // stick: with no bundled engine available we fall back to the bare
        // `vex` (PATH lookup) rather than the dead absolute path.
        let stale = if cfg!(windows) {
            "C:/old/install/vex.exe"
        } else {
            "/opt/old-install/vex"
        };
        assert_eq!(resolve_vex_bin(stale), "vex");
    }

    #[test]
    fn resolve_vex_bin_normalises_placeholder() {
        // The bare placeholder and blank values both collapse to `vex` when no
        // bundled engine sits next to the test binary.
        assert_eq!(resolve_vex_bin("vex"), "vex");
        assert_eq!(resolve_vex_bin("   "), "vex");
    }

    #[test]
    fn migrates_only_known_expired_planmorph_endpoints() {
        let mut cfg = Config {
            api_base: "https://api.planmorph.software".to_string(),
            web_base: "https://studio.planmorph.software".to_string(),
            vex_serve_host: "vex.planmorph.software".to_string(),
            ..Config::default()
        };

        assert!(migrate_expired_endpoints(&mut cfg));
        assert_eq!(cfg.api_base, default_api_base());
        assert_eq!(cfg.web_base, default_web_base());
        assert_eq!(cfg.vex_serve_host, default_vex_serve_host());

        for old_ip in ["20.223.15.197", "20.166.239.248"] {
            let mut cfg = Config {
                vex_serve_host: old_ip.to_string(),
                ..Config::default()
            };
            assert!(migrate_expired_endpoints(&mut cfg));
            assert_eq!(cfg.vex_serve_host, default_vex_serve_host());
        }

        let mut custom = Config {
            api_base: "https://vex.example.com".to_string(),
            web_base: "https://studio.example.com".to_string(),
            vex_serve_host: "ssh.example.com".to_string(),
            ..Config::default()
        };
        assert!(!migrate_expired_endpoints(&mut custom));
        assert_eq!(custom.api_base, "https://vex.example.com");
        assert_eq!(custom.web_base, "https://studio.example.com");
        assert_eq!(custom.vex_serve_host, "ssh.example.com");
    }

    #[test]
    fn removes_watch_by_project_id() {
        let mut cfg = Config {
            watch: vec![
                WatchEntry {
                    project_id: "a".to_string(),
                    cloud_project_id: None,
                    path: "/tmp/a".to_string(),
                    include: vec!["*.ifc".to_string()],
                    ifc_project_guid: None,
                    project_name: None,
                },
                WatchEntry {
                    project_id: "b".to_string(),
                    cloud_project_id: None,
                    path: "/tmp/b".to_string(),
                    include: vec!["*.ifc".to_string()],
                    ifc_project_guid: Some("guid".to_string()),
                    project_name: Some("Project B".to_string()),
                },
            ],
            ..Config::default()
        };

        let removed = cfg.remove_watch("b").unwrap();

        assert_eq!(removed.project_id, "b");
        assert_eq!(cfg.watch.len(), 1);
        assert_eq!(cfg.watch[0].project_id, "a");
        assert!(cfg.remove_watch("missing").is_none());
    }
}
