//! vex-bridge — local daemon + CLI.
//!
//! This crate exports a `lib` so integration tests and embedded use cases can
//! drive the same code paths as the binary. The binary in `src/main.rs` is a
//! thin wrapper around [`cli::run`].

pub mod cli;
pub mod config;
pub mod errors;
pub mod keychain;
pub mod pairing;
pub mod pipeline;
pub mod server;
pub mod state;
pub mod vex_cli;
pub mod watcher;
