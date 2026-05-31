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

    /// Run the daemon: register with relays + serve the poll/send loop.
    Daemon(commands::daemon::Args),

    /// Decrypt + print the body of an envelope (encrypted or plaintext).
    Read(commands::read::Args),

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
        Cmd::Launch(a) => commands::launch::run(a),
        Cmd::Mcp(a) => commands::mcp::run(a),
        Cmd::Profile(a) => commands::profile::run(a),
        Cmd::View(a) => commands::view::run(a),
    }
}
