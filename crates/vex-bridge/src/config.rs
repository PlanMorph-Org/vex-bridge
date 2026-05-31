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
    #[serde(default = "default_api_base")]
    pub api_base: String,

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
    "https://studio.planmorph.software".into()
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

impl Default for Config {
    fn default() -> Self {
        Self {
            api_base: default_api_base(),
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
        let explicit_vex_bin = raw
            .parse::<toml::Value>()
            .ok()
            .and_then(|value| value.get("vex_bin").cloned())
            .is_some();
        let mut cfg: Self = toml::from_str(&raw).map_err(|e| BridgeError::Config(e.to_string()))?;
        if !explicit_vex_bin {
            cfg.vex_bin = default_vex_bin();
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
    fn removes_watch_by_project_id() {
        let mut cfg = Config {
            watch: vec![
                WatchEntry {
                    project_id: "a".to_string(),
                    path: "/tmp/a".to_string(),
                    include: vec!["*.ifc".to_string()],
                    ifc_project_guid: None,
                    project_name: None,
                },
                WatchEntry {
                    project_id: "b".to_string(),
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
