//! `sec view <path>` — open a markdown file in the Secretariat desktop app.
//!
//! Spawns the bundled Secretariat binary with the file path as argv[1].
//! The Tauri single-instance plugin in the running app picks up the path
//! and routes it through `PendingOpens` → `RunEvent::Opened`-style
//! window spawning.
//!
//! On macOS the app may be launched via `open -a Secretariat <path>` so
//! we delegate to LaunchServices; falls back to invoking the binary
//! directly when the bundle is not present (development checkouts).

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser, Debug)]
pub struct Args {
    /// Path to the markdown file to open.
    pub path: PathBuf,
}

pub fn run(args: Args) -> Result<()> {
    let abs = std::fs::canonicalize(&args.path)
        .with_context(|| format!("could not resolve {}", args.path.display()))?;

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .args(["-a", "Secretariat"])
            .arg(&abs)
            .status()
            .context("failed to exec `open -a Secretariat`")?;
        if !status.success() {
            anyhow::bail!(
                "`open -a Secretariat` exited {} — is the app installed?",
                status
            );
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        anyhow::bail!("`sec view` is macOS-only for now (Tauri shell is Mac-only).")
    }
}
