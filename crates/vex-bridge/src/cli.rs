//! `vex-bridge` CLI: small surface, mostly maintenance + manual ops. Most of
//! the time the daemon runs in the background and plugins talk to it via
//! HTTP.

use std::sync::Arc;
use std::time::Instant;

use base64::Engine;
use clap::{Parser, Subcommand};
use rand::rngs::OsRng;
use rand::RngCore;
use tokio::sync::RwLock;
use tracing::info;

use crate::config::{Config, Paths};
use crate::errors::{BridgeError, BridgeResult};
use crate::keychain;
use crate::pairing;
use crate::server::{self, AppState};
use crate::state::{now_unix, PairingState, State};

#[derive(Debug, Parser)]
#[command(name = "vex-bridge", version, about = "Local CAD↔architur bridge daemon.")]
struct Args {
    #[arg(global = true, long, default_value = "info")]
    log: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Run the HTTP daemon in the foreground. Use a service manager (launchd /
    /// systemd / nssm) to keep it alive across reboots.
    Start,
    /// One-shot: print health JSON to stdout and exit.
    Status,
    /// Pair this machine with an architur account. Prints the code + URL,
    /// then waits for browser approval.
    Pair {
        #[arg(long, default_value = "Unnamed device")]
        device_label: String,
        /// Automatically open the pairing URL in the default browser.
        /// Used by the install script so users don't need to copy/paste.
        #[arg(long)]
        open_browser: bool,
    },
    /// Forget the keypair and unregister from architur.
    Unpair,
}

pub fn run() -> BridgeResult<()> {
    let args = Args::parse();
    init_tracing(&args.log);
    let paths = Paths::discover()?;
    paths.ensure_dirs()?;

    match args.cmd {
        Cmd::Start => run_start(paths),
        Cmd::Status => run_status(paths),
        Cmd::Pair { device_label, open_browser } => run_pair(paths, device_label, open_browser),
        Cmd::Unpair => run_unpair(paths),
    }
}

fn init_tracing(level: &str) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| level.into()),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .try_init();
}

fn ensure_access_token(paths: &Paths) -> BridgeResult<String> {
    if let Ok(existing) = std::fs::read_to_string(&paths.access_token_file) {
        let trimmed = existing.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    let mut buf = [0u8; 32];
    OsRng.fill_bytes(&mut buf);
    let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf);
    std::fs::write(&paths.access_token_file, &token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            &paths.access_token_file,
            std::fs::Permissions::from_mode(0o600),
        )?;
    }
    Ok(token)
}

fn run_start(paths: Paths) -> BridgeResult<()> {
    let cfg = Config::load_or_default(&paths)?;
    let state = State::load(&paths)?;
    let token = ensure_access_token(&paths)?;
    let port = cfg.port;
    let app = AppState {
        config: Arc::new(RwLock::new(cfg.clone())),
        state: Arc::new(RwLock::new(state)),
        paths: Arc::new(paths),
        access_token: Arc::new(token),
        started_at: Instant::now(),
    };
    let rt = tokio::runtime::Runtime::new().map_err(BridgeError::Io)?;
    rt.block_on(async move {
        // Start configured watch → add+commit+push pipelines. The handles
        // must outlive `serve`, so bind them in this scope.
        let _watchers = crate::pipeline::spawn_all(&cfg, tokio::runtime::Handle::current());
        if let Err(e) = server::serve(app, port).await {
            tracing::error!(error = ?e, "server exited");
        }
    });
    Ok(())
}

fn run_status(paths: Paths) -> BridgeResult<()> {
    let state = State::load(&paths)?;
    println!("{}", serde_json::to_string_pretty(&state)?);
    Ok(())
}

fn run_pair(paths: Paths, device_label: String, open_browser: bool) -> BridgeResult<()> {
    let cfg = Config::load_or_default(&paths)?;
    let rt = tokio::runtime::Runtime::new().map_err(BridgeError::Io)?;
    rt.block_on(async {
        let outcome = pairing::start(&cfg, &device_label).await?;
        println!();
        println!("  Pairing code:  {}", outcome.code);
        println!("  Open this URL: {}", outcome.pair_url);
        println!("  Expires at:    {}", outcome.expires_at);
        println!("  Fingerprint:   {}", outcome.key_fingerprint);
        println!();

        if open_browser {
            // Best-effort: don't fail the whole command if the browser won't open.
            if let Err(e) = open::that(&outcome.pair_url) {
                eprintln!("Note: could not open browser automatically ({e}). Visit the URL above.");
            } else {
                println!("Opening your browser to complete setup...");
            }
        }

        println!("Waiting for browser approval (Ctrl-C to cancel)...");

        let mut state = State::load(&paths)?;
        state.pairing = PairingState::Pending {
            code: outcome.code.clone(),
            pair_url: outcome.pair_url.clone(),
            expires_at_unix: now_unix() + 600,
            device_label: device_label.clone(),
        };
        state.save(&paths)?;

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            match pairing::poll(&cfg, &outcome.code).await? {
                Some(key_id) => {
                    let mut state = State::load(&paths)?;
                    state.pairing = PairingState::Paired {
                        device_label,
                        key_fingerprint: outcome.key_fingerprint,
                        key_id,
                        paired_at_unix: now_unix(),
                    };
                    state.save(&paths)?;
                    info!("paired successfully");
                    println!("✓ Paired.");
                    return Ok::<(), BridgeError>(());
                }
                None => continue,
            }
        }
    })?;
    Ok(())
}

fn run_unpair(paths: Paths) -> BridgeResult<()> {
    keychain::forget()?;
    let mut state = State::load(&paths)?;
    state.pairing = PairingState::Unpaired;
    state.save(&paths)?;
    println!("✓ Unpaired. The remote-side key registration must be removed via the web UI.");
    Ok(())
}
