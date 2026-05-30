//! Persisted state — what the daemon needs to remember across restarts that
//! is *not* secret (the secret lives in the OS keychain).

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use vex_bridge_protocol as proto;

use crate::config::Paths;
use crate::errors::BridgeResult;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub pairing: PairingState,
    #[serde(default)]
    pub seen_ifc_hashes: Vec<SeenIfcHash>,
    #[serde(default)]
    pub recent_activity: Vec<proto::ActivityEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeenIfcHash {
    pub hash: String,
    pub project_id: String,
    #[serde(default)]
    pub ifc_project_guid: Option<String>,
    pub imported_at_unix: i64,
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
        #[serde(default)]
        key_fingerprint: String,
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

    pub fn has_seen_ifc_hash(&self, hash: &str) -> bool {
        self.seen_ifc_hashes.iter().any(|seen| seen.hash == hash)
    }

    pub fn mark_ifc_hash_seen(
        &mut self,
        hash: String,
        project_id: String,
        ifc_project_guid: Option<String>,
    ) {
        if self.has_seen_ifc_hash(&hash) {
            return;
        }
        self.seen_ifc_hashes.push(SeenIfcHash {
            hash,
            project_id,
            ifc_project_guid,
            imported_at_unix: now_unix(),
        });
        const MAX_SEEN_HASHES: usize = 4096;
        if self.seen_ifc_hashes.len() > MAX_SEEN_HASHES {
            let excess = self.seen_ifc_hashes.len() - MAX_SEEN_HASHES;
            self.seen_ifc_hashes.drain(0..excess);
        }
    }

    pub fn push_activity(&mut self, event: proto::ActivityEvent) {
        self.recent_activity.push(event);
        const MAX_RECENT_ACTIVITY: usize = 256;
        if self.recent_activity.len() > MAX_RECENT_ACTIVITY {
            let excess = self.recent_activity.len() - MAX_RECENT_ACTIVITY;
            self.recent_activity.drain(0..excess);
        }
    }

    pub fn recent_activity(&self, limit: usize) -> Vec<proto::ActivityEvent> {
        self.recent_activity
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
