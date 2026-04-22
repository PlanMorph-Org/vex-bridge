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
use std::path::PathBuf;

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
    /// Base URL of the architur API, e.g. `https://api.planmorph.software`.
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
}

fn default_api_base() -> String {
    "https://api.planmorph.software".into()
}
fn default_vex_bin() -> String {
    "vex".into()
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
        toml::from_str(&raw).map_err(|e| BridgeError::Config(e.to_string()))
    }

    pub fn save(&self, paths: &Paths) -> BridgeResult<()> {
        paths.ensure_dirs()?;
        let body = toml::to_string_pretty(self).map_err(|e| BridgeError::Config(e.to_string()))?;
        fs::write(&paths.config_file, body)?;
        Ok(())
    }
}
