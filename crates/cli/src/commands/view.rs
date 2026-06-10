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

use secretariat_core::infrastructure::open_in_secretariat;

#[derive(Parser, Debug)]
pub struct Args {
    /// Path to the markdown file to open.
    pub path: PathBuf,
}

pub fn run(args: Args) -> Result<()> {
    let abs = std::fs::canonicalize(&args.path)
        .with_context(|| format!("could not resolve {}", args.path.display()))?;
    open_in_secretariat(&abs).context("opening the file in the Secretariat desktop app")
}
