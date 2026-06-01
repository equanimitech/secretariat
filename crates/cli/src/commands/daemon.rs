//! `sec daemon` — clap surface for the macOS LaunchAgent supervision.
//!
//! The federation column (relay registration, the serve poll loop, the
//! one-shot `tick`) was removed in the git-native teardown (cut A). What
//! remains is the LaunchAgent ceremony — install / uninstall / status —
//! plus a keepalive `serve` entry point the installed plist targets.
//!
//! See `crates/daemon/src/lib.rs` for the library surface.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use secretariat_core::infrastructure::keys::load_signing_key;
use secretariat_daemon::{
    init_tracing, install_launchagent, report_status, serve, uninstall_launchagent,
};

use super::paths::{key_paths, load_did};

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Run the foreground keepalive loop. This is the LaunchAgent plist's
    /// target (`sec daemon serve`); it blocks until SIGTERM/SIGINT and
    /// brings no subsystems online today (git-native teardown, cut A).
    Serve,
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
            Cmd::Serve => {
                let did = load_did(&paths)?;
                let key = load_signing_key(&paths.signing_key).with_context(|| {
                    format!("loading signing key from {}", paths.signing_key.display())
                })?;
                serve(&paths, &did, &key).await
            }
            Cmd::Install => install_launchagent(&paths).await,
            Cmd::Uninstall => uninstall_launchagent().await,
            Cmd::Status => report_status(&paths).await,
        }
    })
}
