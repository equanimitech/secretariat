//! `sec-mcp` — Secretariat MCP server binary.
//!
//! Stdio transport. Wire into Claude Desktop via:
//!
//! ```json
//! // ~/Library/Application Support/Claude/claude_desktop_config.json
//! {
//!   "mcpServers": {
//!     "secretariat": {
//!       "command": "/Users/<you>/.cargo/bin/sec-mcp"
//!     }
//!   }
//! }
//! ```
//!
//! After editing, restart Claude Desktop; the 7 Secretariat tools appear in
//! the tool picker.

use anyhow::{Context, Result};
use rmcp::{transport::io::stdio, ServiceExt};
use secretariat_core::infrastructure::keys::KeyPaths;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod server;

use server::SecretariatServer;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let paths = resolve_key_paths()?;
    paths
        .ensure_dirs()
        .with_context(|| format!("ensuring {} exists", paths.root.display()))?;

    info!(root = %paths.root.display(), "sec-mcp starting on stdio");

    let server = SecretariatServer::new(paths);
    let service = server
        .serve(stdio())
        .await
        .context("failed to start MCP service over stdio")?;

    service.waiting().await.context("MCP service errored")?;
    Ok(())
}

fn init_tracing() {
    // Stdio is reserved for the MCP wire — log to stderr only.
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,sec_mcp=info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr).compact())
        .try_init();
}

fn resolve_key_paths() -> Result<KeyPaths> {
    if let Ok(p) = std::env::var("SECRETARIAT_HOME") {
        return Ok(KeyPaths::under(PathBuf::from(p)));
    }
    KeyPaths::discover().context("resolving ~/.secretariat")
}
