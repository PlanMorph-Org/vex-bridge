//! vex-bridge — local daemon + CLI.
//!
//! This crate exports a `lib` so integration tests and embedded use cases can
//! drive the same code paths as the binary. The binary in `src/main.rs` is a
//! thin wrapper around [`cli::run`].

pub mod cli;
pub mod config;
pub mod dashboard;
#[cfg(feature = "desktop")]
pub mod desktop;
#[cfg(feature = "tray-icon-asset")]
pub(crate) mod desktop_assets;
pub mod device;
pub mod errors;
pub mod ifc;
pub mod keychain;
pub mod pairing;
pub mod pipeline;
pub mod server;
pub mod state;
#[cfg(feature = "tray")]
pub mod tray;
pub mod vex_cli;
pub mod watcher;
