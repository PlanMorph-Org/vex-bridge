//! Talks to architur's `/api/device-pairing/*` to convert the locally
//! generated SSH key into a registered `UserSshKey` for the logged-in user.

use base64::Engine;
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::errors::{BridgeError, BridgeResult};
use crate::keychain;

#[derive(Debug, Clone, Serialize)]
struct StartReq {
    device_label: String,
    public_key: String,
}

#[derive(Debug, Clone, Deserialize)]
struct StartResp {
    code: String,
    expires_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PollResp {
    status: String,
    key_id: Option<String>,
    device_label: Option<String>,
}

pub struct PairingOutcome {
    pub code: String,
    pub pair_url: String,
    pub expires_at: String,
    pub key_fingerprint: String,
}

/// Generate a new key (or reuse the existing one), POST to
/// `/api/device-pairing/start`, and return the user-facing code + URL.
pub async fn start(cfg: &Config, device_label: &str) -> BridgeResult<PairingOutcome> {
    let signing = match keychain::load()? {
        Some(k) => k,
        None => keychain::generate_and_store()?,
    };
    let public_key = keychain::openssh_public(&signing);
    let fingerprint = fingerprint_for(&signing);

    let url = format!("{}/api/device-pairing/start", cfg.api_base.trim_end_matches('/'));
    let body = StartReq {
        device_label: device_label.to_string(),
        public_key: public_key.clone(),
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let resp = client.post(&url).json(&body).send().await?;
    if !resp.status().is_success() {
        let code = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(BridgeError::UpstreamApi(format!(
            "{code}: {}",
            text.trim()
        )));
    }
    let parsed: StartResp = resp.json().await?;

    let pair_url = format!(
        "{}/pair?code={}",
        cfg.api_base
            .trim_end_matches('/')
            .replace("api.", "app.")
            .replace("//api.", "//app."),
        parsed.code
    );

    Ok(PairingOutcome {
        code: parsed.code,
        pair_url,
        expires_at: parsed.expires_at,
        key_fingerprint: fingerprint,
    })
}

/// Poll once. Returns `Ok(Some(key_id))` when the user has approved in the
/// browser, `Ok(None)` while still pending. Errors on `expired` / `not_found`.
pub async fn poll(cfg: &Config, code: &str) -> BridgeResult<Option<String>> {
    let url = format!(
        "{}/api/device-pairing/{}",
        cfg.api_base.trim_end_matches('/'),
        code
    );
    let resp = reqwest::Client::new().get(&url).send().await?;
    let status = resp.status();
    let parsed: PollResp = resp.json().await?;
    match parsed.status.as_str() {
        "approved" => Ok(parsed.key_id),
        "pending" => Ok(None),
        "expired" => Err(BridgeError::UpstreamApi("pairing code expired".into())),
        "not_found" => Err(BridgeError::UpstreamApi("pairing code not found".into())),
        other => Err(BridgeError::UpstreamApi(format!(
            "unexpected pairing status {other} (HTTP {status})"
        ))),
    }
}

/// SHA256 fingerprint of the SSH wire-format public key (matches
/// `ssh-keygen -lf` output: `SHA256:<base64-no-pad>`).
pub fn fingerprint_for(key: &SigningKey) -> String {
    let pk = key.verifying_key().to_bytes();
    let mut blob = Vec::with_capacity(4 + 11 + 4 + 32);
    blob.extend_from_slice(&(11u32).to_be_bytes());
    blob.extend_from_slice(b"ssh-ed25519");
    blob.extend_from_slice(&(32u32).to_be_bytes());
    blob.extend_from_slice(&pk);
    let digest = Sha256::digest(&blob);
    format!(
        "SHA256:{}",
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest)
    )
}
