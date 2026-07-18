//! Talks to architur's `/api/device-pairing/*` to convert the locally
//! generated SSH key into a registered `UserSshKey` for the logged-in user.

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tracing::warn;

use crate::config::Config;
use crate::errors::{BridgeError, BridgeResult};
use crate::keychain;

// architur's API is ASP.NET Core Minimal APIs with the default
// System.Text.Json contract: camelCase on output, case-insensitive on
// input. So we send + receive camelCase (`deviceLabel`, `publicKey`,
// `code`, `expiresAt`, `status`, `keyId`, `deviceLabel`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartReq {
    device_label: String,
    public_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartResp {
    code: String,
    expires_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PollResp {
    status: String,
    key_id: Option<String>,
    #[allow(dead_code)]
    device_label: Option<String>,
    account_id: Option<String>,
    account_email: Option<String>,
    account_name: Option<String>,
}

/// Identity + key returned once the user approves in the browser.
#[derive(Debug, Clone)]
pub struct PairApproval {
    pub key_id: String,
    pub account_id: Option<String>,
    pub account_email: Option<String>,
    pub account_name: Option<String>,
}

pub struct PairingOutcome {
    pub code: String,
    pub pair_url: String,
    pub expires_at: String,
    pub key_fingerprint: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudProjectResponse {
    id: String,
    owner_slug: String,
    owner_name: String,
    slug: String,
    full_name: String,
    default_branch: String,
    has_commits: bool,
}

pub async fn projects(
    cfg: &Config,
    key_id: &str,
) -> BridgeResult<Vec<vex_bridge_protocol::CloudProject>> {
    let signing = keychain::load()?.ok_or(BridgeError::PairingKeyUnavailable)?;
    projects_with_signing(cfg, key_id, &signing).await
}

const PROJECTS_REQUEST_TIMEOUT: Duration = Duration::from_secs(12);
const PROJECTS_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const PROJECTS_MAX_ATTEMPTS: u8 = 3;

async fn projects_with_signing(
    cfg: &Config,
    key_id: &str,
    signing: &SigningKey,
) -> BridgeResult<Vec<vex_bridge_protocol::CloudProject>> {
    if key_id.trim().is_empty() {
        return Err(BridgeError::PairingKeyUnavailable);
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let message = format!("GET\n/api/device-pairing/projects\n{timestamp}");
    let signature = base64::engine::general_purpose::STANDARD
        .encode(signing.sign(message.as_bytes()).to_bytes());
    let url = format!(
        "{}/api/device-pairing/projects",
        cfg.api_base.trim_end_matches('/')
    );

    let client = reqwest::Client::builder()
        .connect_timeout(PROJECTS_CONNECT_TIMEOUT)
        .timeout(PROJECTS_REQUEST_TIMEOUT)
        .build()?;

    for attempt in 1..=PROJECTS_MAX_ATTEMPTS {
        let response = client
            .get(&url)
            .header("X-Vex-Key-Id", key_id.trim())
            .header("X-Vex-Timestamp", timestamp)
            .header("X-Vex-Signature", &signature)
            .send()
            .await;

        match response {
            Ok(response) if response.status().is_success() => {
                let projects =
                    response
                        .json::<Vec<CloudProjectResponse>>()
                        .await
                        .map_err(|error| {
                            BridgeError::CloudProjectsInvalidResponse(format!(
                                "expected a project list: {error}"
                            ))
                        })?;
                return Ok(projects
                    .into_iter()
                    .map(|project| vex_bridge_protocol::CloudProject {
                        id: project.id,
                        owner_slug: project.owner_slug,
                        owner_name: project.owner_name,
                        slug: project.slug,
                        full_name: project.full_name,
                        default_branch: project.default_branch,
                        has_commits: project.has_commits,
                    })
                    .collect());
            }
            Ok(response)
                if response.status() == StatusCode::UNAUTHORIZED
                    || response.status() == StatusCode::FORBIDDEN =>
            {
                return Err(BridgeError::PairingCredentialsRejected);
            }
            Ok(response) if should_retry_status(response.status()) => {
                let status = response.status();
                if attempt == PROJECTS_MAX_ATTEMPTS {
                    return Err(BridgeError::CloudProjectsUnavailable(format!(
                        "HTTP {status}"
                    )));
                }
                warn!(
                    attempt,
                    max_attempts = PROJECTS_MAX_ATTEMPTS,
                    %status,
                    "cloud projects request failed transiently; retrying"
                );
            }
            Ok(response) => {
                return Err(BridgeError::CloudProjectsInvalidResponse(format!(
                    "HTTP {}",
                    response.status()
                )));
            }
            Err(error) => {
                if attempt == PROJECTS_MAX_ATTEMPTS {
                    return Err(BridgeError::CloudProjectsUnavailable(error.to_string()));
                }
                warn!(
                    attempt,
                    max_attempts = PROJECTS_MAX_ATTEMPTS,
                    error = %error,
                    "cloud projects request failed; retrying"
                );
            }
        }

        tokio::time::sleep(Duration::from_millis(250 * u64::from(attempt))).await;
    }

    unreachable!("projects request returns on every attempt")
}

fn should_retry_status(status: StatusCode) -> bool {
    status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
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

    let url = format!(
        "{}/api/device-pairing/start",
        cfg.api_base.trim_end_matches('/')
    );
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
        return Err(BridgeError::UpstreamApi(format!("{code}: {}", text.trim())));
    }
    let parsed: StartResp = resp.json().await?;

    let pair_url = format!(
        "{}/pair?code={}",
        cfg.web_base.trim_end_matches('/'),
        parsed.code
    );

    Ok(PairingOutcome {
        code: parsed.code,
        pair_url,
        expires_at: parsed.expires_at,
        key_fingerprint: fingerprint,
    })
}

/// Poll once. Returns `Ok(Some(approval))` when the user has approved in the
/// browser, `Ok(None)` while still pending. Errors on `expired` / `not_found`.
pub async fn poll(cfg: &Config, code: &str) -> BridgeResult<Option<PairApproval>> {
    let url = format!(
        "{}/api/device-pairing/{}",
        cfg.api_base.trim_end_matches('/'),
        code
    );
    let resp = reqwest::Client::new().get(&url).send().await?;
    let status = resp.status();
    let parsed: PollResp = resp.json().await?;
    match parsed.status.as_str() {
        "approved" => match parsed.key_id {
            Some(key_id) => Ok(Some(PairApproval {
                key_id,
                account_id: parsed.account_id,
                account_email: parsed.account_email,
                account_name: parsed.account_name,
            })),
            None => Err(BridgeError::UpstreamApi(
                "pairing approved but no key id returned".into(),
            )),
        },
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

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use axum::{response::IntoResponse, routing::get, Router};
    use reqwest::StatusCode;

    use super::{projects_with_signing, should_retry_status, BridgeError, Config, SigningKey};

    async fn start_projects_server(app: Router) -> (Config, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let cfg = Config {
            api_base: format!("http://{address}"),
            ..Config::default()
        };
        (cfg, task)
    }

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    const PROJECT_LIST: &str = r#"[{
        "id":"af92ef93-5e63-4c13-a2d7-9c722d175ccb",
        "ownerSlug":"northwind",
        "ownerName":"Northwind",
        "slug":"tower-a",
        "fullName":"northwind/tower-a",
        "defaultBranch":"main",
        "hasCommits":false
    }]"#;

    #[tokio::test]
    async fn projects_retries_a_transient_failure_then_returns_the_list() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let app = Router::new().route(
            "/api/device-pairing/projects",
            get({
                let attempts = attempts.clone();
                move || {
                    let attempts = attempts.clone();
                    async move {
                        if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                            (StatusCode::SERVICE_UNAVAILABLE, "temporarily unavailable")
                                .into_response()
                        } else {
                            (StatusCode::OK, PROJECT_LIST).into_response()
                        }
                    }
                }
            }),
        );
        let (cfg, task) = start_projects_server(app).await;

        let projects =
            projects_with_signing(&cfg, "af92ef93-5e63-4c13-a2d7-9c722d175ccb", &signing_key())
                .await
                .unwrap();

        task.abort();
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].full_name, "northwind/tower-a");
    }

    #[tokio::test]
    async fn projects_distinguishes_rejected_device_credentials() {
        let app = Router::new().route(
            "/api/device-pairing/projects",
            get(|| async { StatusCode::UNAUTHORIZED }),
        );
        let (cfg, task) = start_projects_server(app).await;

        let error =
            projects_with_signing(&cfg, "af92ef93-5e63-4c13-a2d7-9c722d175ccb", &signing_key())
                .await
                .unwrap_err();

        task.abort();
        assert!(matches!(error, BridgeError::PairingCredentialsRejected));
    }

    #[tokio::test]
    async fn projects_reports_an_invalid_success_payload_without_claiming_network_failure() {
        let app = Router::new().route(
            "/api/device-pairing/projects",
            get(|| async { (StatusCode::OK, r#"{"projects":[]}"#) }),
        );
        let (cfg, task) = start_projects_server(app).await;

        let error =
            projects_with_signing(&cfg, "af92ef93-5e63-4c13-a2d7-9c722d175ccb", &signing_key())
                .await
                .unwrap_err();

        task.abort();
        assert!(matches!(
            error,
            BridgeError::CloudProjectsInvalidResponse(_)
        ));
    }

    #[tokio::test]
    async fn projects_rejects_missing_registered_key_id_before_network_access() {
        let error = projects_with_signing(&Config::default(), " ", &signing_key())
            .await
            .unwrap_err();

        assert!(matches!(error, BridgeError::PairingKeyUnavailable));
    }

    #[test]
    fn only_transient_project_service_statuses_are_retried() {
        assert!(should_retry_status(StatusCode::REQUEST_TIMEOUT));
        assert!(should_retry_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(should_retry_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(!should_retry_status(StatusCode::UNAUTHORIZED));
        assert!(!should_retry_status(StatusCode::BAD_REQUEST));
    }
}
