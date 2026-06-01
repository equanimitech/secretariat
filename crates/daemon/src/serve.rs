//! Minimal keepalive for the LaunchAgent-supervised process.
//!
//! The federation poll loop, IPC socket, outbox watcher, and `sync_now`
//! tick that used to live here were removed in the git-native teardown
//! (cut A). The installed LaunchAgent plist targets `sec daemon serve`
//! (with `RunAtLoad` + `KeepAlive`), so this entry point must continue to
//! exist and behave: it blocks until a shutdown signal so launchd doesn't
//! crash-loop a missing subcommand.
//!
//! When the git-native substrate grows its own delivery path, the real
//! subsystems re-land behind this same `serve` entry point.

use anyhow::{Context, Result};
use ed25519_dalek::SigningKey;
use secretariat_core::infrastructure::keys::KeyPaths;
use secretariat_core::Did;
use tracing::info;

/// Block until SIGTERM (what `launchctl unload` sends) or SIGINT
/// (Ctrl-C in a foreground `sec daemon serve`). Brings nothing online
/// today — see the module docs.
pub async fn serve(paths: &KeyPaths, did: &Did, _key: &SigningKey) -> Result<()> {
    paths.ensure_dirs()?;
    info!(did = %did, "daemon serve (keepalive only) starting");

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("installing SIGTERM handler")?;

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("daemon shutdown signal received");
        }
        _ = sigterm.recv() => {
            info!("daemon shutdown signal received");
        }
    }
    Ok(())
}
