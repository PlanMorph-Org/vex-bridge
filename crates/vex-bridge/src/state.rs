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
    pub ifc_snapshots: Vec<IfcSnapshot>,
    #[serde(default)]
    pub recent_activity: Vec<proto::ActivityEvent>,
    /// Commits that were recorded locally but whose push to the remote has
    /// not yet succeeded. Drained by the daemon's background outbox with
    /// exponential backoff so a transient network failure never silently
    /// loses a push.
    #[serde(default)]
    pub pending_push: Vec<PendingPush>,
}

/// One queued push awaiting a successful sync. A `vex push` advances the
/// remote ref to the local HEAD, so only the *latest* commit per
/// `(project_id, refspec)` matters: enqueuing replaces any earlier entry for
/// the same target, which keeps the queue naturally bounded to one row per
/// project/branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingPush {
    pub project_id: String,
    /// Absolute path to the local vex repo directory.
    pub dir: String,
    pub remote: String,
    /// Full refspec, e.g. `refs/heads/main`.
    pub refspec: String,
    /// Commit the local repo was at when this push was queued (for display).
    pub commit_hash: String,
    pub enqueued_at_unix: i64,
    /// Number of failed attempts so far.
    #[serde(default)]
    pub attempts: u32,
    /// Earliest unix time the outbox should retry this entry.
    #[serde(default)]
    pub next_attempt_unix: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeenIfcHash {
    pub hash: String,
    pub project_id: String,
    #[serde(default)]
    pub ifc_project_guid: Option<String>,
    pub imported_at_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfcSnapshot {
    pub project_id: String,
    pub commit_hash: String,
    pub content_hash: String,
    pub path: String,
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
        #[serde(default)]
        account_id: Option<String>,
        #[serde(default)]
        account_email: Option<String>,
        #[serde(default)]
        account_name: Option<String>,
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

    pub fn record_ifc_snapshot(
        &mut self,
        project_id: String,
        commit_hash: String,
        content_hash: String,
        path: String,
        ifc_project_guid: Option<String>,
    ) {
        self.ifc_snapshots.retain(|snapshot| {
            !(snapshot.project_id == project_id && snapshot.commit_hash == commit_hash)
        });
        self.ifc_snapshots.push(IfcSnapshot {
            project_id,
            commit_hash,
            content_hash,
            path,
            ifc_project_guid,
            imported_at_unix: now_unix(),
        });
        const MAX_IFC_SNAPSHOTS: usize = 4096;
        if self.ifc_snapshots.len() > MAX_IFC_SNAPSHOTS {
            let excess = self.ifc_snapshots.len() - MAX_IFC_SNAPSHOTS;
            self.ifc_snapshots.drain(0..excess);
        }
    }

    pub fn ifc_snapshot(&self, project_id: &str, commit_hash: &str) -> Option<IfcSnapshot> {
        self.ifc_snapshots
            .iter()
            .rev()
            .find(|snapshot| {
                snapshot.project_id == project_id
                    && (snapshot.commit_hash == commit_hash
                        || snapshot.commit_hash.starts_with(commit_hash)
                        || commit_hash.starts_with(&snapshot.commit_hash))
            })
            .cloned()
    }

    /// Most recently recorded IFC snapshot for a project. Snapshots are pushed
    /// in commit order, so the last matching entry corresponds to the latest
    /// commit's full model. Used to serve the complete current model even when
    /// no changes have been made since the last commit.
    pub fn latest_ifc_snapshot_for_project(&self, project_id: &str) -> Option<IfcSnapshot> {
        self.ifc_snapshots
            .iter()
            .rev()
            .find(|snapshot| snapshot.project_id == project_id)
            .cloned()
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

    /// Queue (or refresh) a pending push for a project/branch. Any existing
    /// entry for the same `(project_id, refspec)` is replaced, because a push
    /// always advances the remote ref to the latest local HEAD.
    pub fn enqueue_push(&mut self, mut push: PendingPush) {
        if push.next_attempt_unix == 0 {
            push.next_attempt_unix = push.enqueued_at_unix;
        }
        if let Some(existing) = self.pending_push.iter_mut().find(|p| {
            p.project_id == push.project_id && p.refspec == push.refspec
        }) {
            // Preserve the original enqueue time and attempt count so backoff
            // keeps growing across repeated failures for the same target.
            push.enqueued_at_unix = existing.enqueued_at_unix.min(push.enqueued_at_unix);
            push.attempts = existing.attempts;
            *existing = push;
        } else {
            self.pending_push.push(push);
        }

        // Hard safety cap. With per-target dedup this should never be hit in
        // practice, but bound it so a pathological config cannot grow state
        // without limit. Drop the oldest entries first.
        const MAX_PENDING_PUSH: usize = 1024;
        if self.pending_push.len() > MAX_PENDING_PUSH {
            let excess = self.pending_push.len() - MAX_PENDING_PUSH;
            self.pending_push.drain(0..excess);
        }
    }

    /// Pending pushes whose backoff window has elapsed, oldest first.
    pub fn pending_push_due(&self, now: i64) -> Vec<PendingPush> {
        let mut due: Vec<PendingPush> = self
            .pending_push
            .iter()
            .filter(|p| p.next_attempt_unix <= now)
            .cloned()
            .collect();
        due.sort_by_key(|p| p.enqueued_at_unix);
        due
    }

    /// Remove a queued push (called after it syncs successfully). Returns
    /// true if an entry was removed.
    pub fn remove_pending_push(&mut self, project_id: &str, refspec: &str) -> bool {
        let before = self.pending_push.len();
        self.pending_push
            .retain(|p| !(p.project_id == project_id && p.refspec == refspec));
        self.pending_push.len() != before
    }

    /// Record a failed retry: bump the attempt count and schedule the next
    /// attempt with capped exponential backoff (base 15s, ceiling 1h).
    pub fn record_push_failure(&mut self, project_id: &str, refspec: &str, error: String, now: i64) {
        if let Some(p) = self
            .pending_push
            .iter_mut()
            .find(|p| p.project_id == project_id && p.refspec == refspec)
        {
            p.attempts = p.attempts.saturating_add(1);
            p.last_error = Some(error);
            p.next_attempt_unix = now + backoff_seconds(p.attempts);
        }
    }

    pub fn pending_push_count(&self) -> usize {
        self.pending_push.len()
    }
}

/// Capped exponential backoff in seconds: 15s, 30s, 60s, … ceiling 3600s.
fn backoff_seconds(attempts: u32) -> i64 {
    const BASE: i64 = 15;
    const CEILING: i64 = 3600;
    let shift = attempts.saturating_sub(1).min(8); // cap shift so 15 << n can't overflow
    (BASE.saturating_mul(1i64 << shift)).min(CEILING)
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_snapshot_list_defaults_empty() {
        let state: State = serde_json::from_str(
            r#"{"pairing":{"status":"unpaired"},"seen_ifc_hashes":[],"recent_activity":[]}"#,
        )
        .unwrap();

        assert!(state.ifc_snapshots.is_empty());
    }

    #[test]
    fn records_and_finds_snapshot_by_prefix() {
        let mut state = State::default();
        state.record_ifc_snapshot(
            "project-1".to_string(),
            "abcdef123456".to_string(),
            "content".to_string(),
            "/tmp/model.ifc".to_string(),
            Some("ifc-guid".to_string()),
        );

        let snapshot = state.ifc_snapshot("project-1", "abcdef").unwrap();
        assert_eq!(snapshot.commit_hash, "abcdef123456");
        assert_eq!(snapshot.path, "/tmp/model.ifc");
        assert_eq!(snapshot.ifc_project_guid.as_deref(), Some("ifc-guid"));
    }

    fn sample_push(project: &str, refspec: &str, commit: &str, at: i64) -> PendingPush {
        PendingPush {
            project_id: project.to_string(),
            dir: "/tmp/repo".to_string(),
            remote: "origin".to_string(),
            refspec: refspec.to_string(),
            commit_hash: commit.to_string(),
            enqueued_at_unix: at,
            attempts: 0,
            next_attempt_unix: at,
            last_error: None,
        }
    }

    #[test]
    fn enqueue_push_dedups_per_target_and_keeps_attempts() {
        let mut state = State::default();
        state.enqueue_push(sample_push("p1", "refs/heads/main", "aaa", 100));
        state.record_push_failure("p1", "refs/heads/main", "offline".into(), 100);

        // A newer commit for the same target replaces the row but keeps the
        // accumulated attempt count and original enqueue time.
        state.enqueue_push(sample_push("p1", "refs/heads/main", "bbb", 200));
        assert_eq!(state.pending_push_count(), 1);
        let entry = &state.pending_push[0];
        assert_eq!(entry.commit_hash, "bbb");
        assert_eq!(entry.attempts, 1);
        assert_eq!(entry.enqueued_at_unix, 100);

        // A different branch is a distinct target.
        state.enqueue_push(sample_push("p1", "refs/heads/dev", "ccc", 210));
        assert_eq!(state.pending_push_count(), 2);
    }

    #[test]
    fn pending_push_due_respects_backoff_and_removal() {
        let mut state = State::default();
        state.enqueue_push(sample_push("p1", "refs/heads/main", "aaa", 100));
        assert_eq!(state.pending_push_due(100).len(), 1);

        // After a failure the next attempt is pushed into the future.
        state.record_push_failure("p1", "refs/heads/main", "offline".into(), 100);
        assert!(state.pending_push_due(100).is_empty());
        assert_eq!(state.pending_push_due(100 + 15).len(), 1);

        assert!(state.remove_pending_push("p1", "refs/heads/main"));
        assert_eq!(state.pending_push_count(), 0);
    }

    #[test]
    fn pending_push_survives_serde_round_trip() {
        let mut state = State::default();
        state.enqueue_push(sample_push("p1", "refs/heads/main", "aaa", 100));
        let json = serde_json::to_string(&state).unwrap();
        let restored: State = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.pending_push_count(), 1);
        assert_eq!(restored.pending_push[0].commit_hash, "aaa");
    }

    #[test]
    fn legacy_state_without_pending_push_loads() {
        let state: State = serde_json::from_str(
            r#"{"pairing":{"status":"unpaired"},"seen_ifc_hashes":[],"recent_activity":[]}"#,
        )
        .unwrap();
        assert_eq!(state.pending_push_count(), 0);
    }
}
