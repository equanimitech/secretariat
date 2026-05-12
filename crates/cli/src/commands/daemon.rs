//! `sec daemon` — clap surface for the long-running plumbing process.
//!
//! The actual work — serve loop, one-shot tick, relay registration,
//! LaunchAgent install/uninstall/status — lives in `secretariat-daemon`.
//! This file is the CLI's thin entry point: argument parsing, principal
//! identity resolution (`key_paths` / `load_did` / `load_signing_key`),
//! and dispatch.
//!
//! See `crates/daemon/src/lib.rs` for the library surface and
//! `docs/ideas/2026-05-12-daemon-evolution.md` for the v0.3+ direction
//! (9 subsystems landing across phases A–E).

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use secretariat_core::infrastructure::keys::load_signing_key;
use secretariat_daemon::{
    init_tracing, install_launchagent, register, report_status, serve, tick_via_ipc_or_inproc,
    uninstall_launchagent,
};

use super::paths::{key_paths, load_did};

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Register with a relay (one-time per relay).
    Register {
        /// Relay endpoint URL, e.g. `wss://relay.rafa.equanimi.tech`
        /// or `http://127.0.0.1:8443` for dev.
        #[arg(long)]
        endpoint: String,
    },
    /// Run the foreground daemon loop.
    Serve,
    /// Run a single sync cycle and exit. Useful for cron, post-stamp
    /// pushes, and Tauri's "Sync now" debugging.
    Tick,
    /// Install the daemon as a macOS LaunchAgent. Survives reboot, runs
    /// in the background. Idempotent — safe to re-run after upgrades.
    Install,
    /// Uninstall the LaunchAgent.
    Uninstall,
    /// Report whether the LaunchAgent is loaded + last-known status.
    Status,
}

pub fn run(args: Args) -> Result<()> {
    init_tracing();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;
    runtime.block_on(async move {
        let paths = key_paths()?;
        match args.cmd {
            Cmd::Register { endpoint } => {
                let did = load_did(&paths)?;
                let key = load_signing_key(&paths.signing_key).with_context(|| {
                    format!("loading signing key from {}", paths.signing_key.display())
                })?;
                register(&paths, &did, &key, &endpoint).await
            }
            Cmd::Serve => {
                let did = load_did(&paths)?;
                let key = load_signing_key(&paths.signing_key).with_context(|| {
                    format!("loading signing key from {}", paths.signing_key.display())
                })?;
                serve(&paths, &did, &key).await
            }
            Cmd::Tick => {
                // Prefer the running daemon's IPC socket so we don't
                // race against its `RelayState` saves; fall back to
                // in-proc when no daemon is listening (v0.2.16
                // behavior preserved).
                let did = load_did(&paths)?;
                let key = load_signing_key(&paths.signing_key).with_context(|| {
                    format!("loading signing key from {}", paths.signing_key.display())
                })?;
                tick_via_ipc_or_inproc(&paths, &did, &key).await
            }
            Cmd::Install => install_launchagent(&paths).await,
            Cmd::Uninstall => uninstall_launchagent().await,
            Cmd::Status => report_status(&paths).await,
        }
    })
}
