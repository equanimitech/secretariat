//! `sec` — Secretariat CLI.

use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser, Debug)]
#[command(
    name = "sec",
    version,
    about = "Secretariat — biometric-attested document stamping."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// One-time setup: generate signing key, write ~/.secretariat/* defaults.
    Init(commands::init::Args),

    /// Manage authorized agents (scribes + future roles).
    Agent(commands::agent::Args),

    /// Embed a biometric-attested stamp into a markdown file.
    Stamp(commands::stamp::Args),

    /// Verify a stamped markdown file against the signer's did:web document.
    Verify(commands::verify::Args),

    /// Manage the macOS LaunchAgent: install / uninstall / status + keepalive serve.
    Daemon(commands::daemon::Args),

    /// Decrypt + print the body of a markdown document (encrypted or plaintext).
    Read(commands::read::Args),

    /// Manage the substrate manifest: enroll / list / unenroll git repos.
    Repo(commands::repo::Args),

    /// Open Claude Code (or the configured cognition CLI) in a channel-bound cwd.
    Launch(commands::launch::Args),

    /// Wire `sec-mcp` into Claude Desktop / Claude Code (no JSON editing).
    Mcp(commands::mcp::Args),

    /// Manage the principal's display name (presence, distinct from identity).
    Profile(commands::profile::Args),

    /// Open a markdown file in the Secretariat desktop app.
    View(commands::view::Args),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Init(a) => commands::init::run(a),
        Cmd::Agent(a) => commands::agent::run(a),
        Cmd::Stamp(a) => commands::stamp::run(a),
        Cmd::Verify(a) => commands::verify::run(a),
        Cmd::Daemon(a) => commands::daemon::run(a),
        Cmd::Read(a) => commands::read::run(a),
        Cmd::Repo(a) => commands::repo::run(a),
        Cmd::Launch(a) => commands::launch::run(a),
        Cmd::Mcp(a) => commands::mcp::run(a),
        Cmd::Profile(a) => commands::profile::run(a),
        Cmd::View(a) => commands::view::run(a),
    }
}
