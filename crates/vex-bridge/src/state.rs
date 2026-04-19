//! Persisted state — what the daemon needs to remember across restarts that
//! is *not* secret (the secret lives in the OS keychain).

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config::Paths;
use crate::errors::BridgeResult;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    pub pairing: PairingState,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PairingState {
    #[default]
    Unpaired,
    Pending {
        code: String,
        pair_url: String,
        expires_at_unix: i64,
        device_label: String,
    },
    Paired {
        device_label: String,
        key_fingerprint: String,
        key_id: String, // architur-side UserSshKey.Id
        paired_at_unix: i64,
    },
}

impl State {
    pub fn load(paths: &Paths) -> BridgeResult<Self> {
        if !paths.state_file.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&paths.state_file)?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn save(&self, paths: &Paths) -> BridgeResult<()> {
        paths.ensure_dirs()?;
        let body = serde_json::to_string_pretty(self)?;
        fs::write(&paths.state_file, body)?;
        Ok(())
    }
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
