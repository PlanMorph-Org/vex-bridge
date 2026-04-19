//! OS keychain integration for the SSH private key.
//!
//! We deliberately use the *OS-native* secret store (Keychain on macOS,
//! Credential Manager on Windows, libsecret/kwallet on Linux) rather than
//! a file under `~/.ssh/` because:
//!
//! * It is encrypted at rest by the OS.
//! * macOS/Windows tie unlock to user login; the daemon cannot read the key
//!   when the screen is locked, which limits blast-radius.
//! * The user can revoke it from the OS UI without touching our config.
//!
//! The keychain stores the 32-byte ed25519 *seed* base64-encoded. The full
//! secret + signing key are derived on demand and zeroised after use.

use base64::Engine;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rand::RngCore;

use crate::errors::{BridgeError, BridgeResult};

const SERVICE: &str = "com.architur.vex-bridge";
const ACCOUNT: &str = "ssh-signing-key";

/// Generate a fresh ed25519 keypair and store the seed in the OS keychain.
/// Overwrites any previous entry, so callers should confirm with the user
/// before invoking this.
pub fn generate_and_store() -> BridgeResult<SigningKey> {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let signing = SigningKey::from_bytes(&seed);
    let entry =
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| BridgeError::Keychain(e.to_string()))?;
    entry
        .set_password(&base64::engine::general_purpose::STANDARD.encode(seed))
        .map_err(|e| BridgeError::Keychain(e.to_string()))?;
    Ok(signing)
}

/// Load the existing signing key from the keychain. Returns `Ok(None)` if no
/// key has been generated yet — callers can decide whether to auto-generate.
pub fn load() -> BridgeResult<Option<SigningKey>> {
    let entry =
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| BridgeError::Keychain(e.to_string()))?;
    match entry.get_password() {
        Ok(b64) => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64.trim())
                .map_err(|e| BridgeError::Keychain(format!("base64: {e}")))?;
            if bytes.len() != 32 {
                return Err(BridgeError::Keychain(format!(
                    "ssh seed wrong length: got {}",
                    bytes.len()
                )));
            }
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&bytes);
            Ok(Some(SigningKey::from_bytes(&seed)))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(BridgeError::Keychain(e.to_string())),
    }
}

/// Forget the key (used by `vex-bridge unpair`).
pub fn forget() -> BridgeResult<()> {
    let entry =
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| BridgeError::Keychain(e.to_string()))?;
    match entry.delete_password() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(BridgeError::Keychain(e.to_string())),
    }
}

/// OpenSSH-format public key line (ready to paste into authorized_keys).
pub fn openssh_public(key: &SigningKey) -> String {
    // SSH wire format: string("ssh-ed25519") || string(key)
    let pk = key.verifying_key().to_bytes();
    let mut blob = Vec::with_capacity(4 + 11 + 4 + 32);
    blob.extend_from_slice(&(11u32).to_be_bytes());
    blob.extend_from_slice(b"ssh-ed25519");
    blob.extend_from_slice(&(32u32).to_be_bytes());
    blob.extend_from_slice(&pk);
    format!(
        "ssh-ed25519 {}",
        base64::engine::general_purpose::STANDARD.encode(blob)
    )
}
