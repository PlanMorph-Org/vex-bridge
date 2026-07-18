use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("bridge is not paired with architur yet — run `vex-bridge pair`")]
    NotPaired,
    #[error(
        "this device is marked as paired, but its local signing key or registered key id is unavailable — pair this device again"
    )]
    PairingKeyUnavailable,
    #[error(
        "the cloud service rejected this device's pairing credentials — pair this device again"
    )]
    PairingCredentialsRejected,
    #[error("the cloud projects service was unavailable after retrying: {0}")]
    CloudProjectsUnavailable(String),
    #[error("the cloud projects service returned an invalid response: {0}")]
    CloudProjectsInvalidResponse(String),
    #[error("the architur server returned an error: {0}")]
    UpstreamApi(String),
    #[error("vex CLI failed: {0}")]
    VexCli(String),
    #[error("invalid configuration: {0}")]
    Config(String),
    #[error("os keychain error: {0}")]
    Keychain(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
}

pub type BridgeResult<T> = Result<T, BridgeError>;
