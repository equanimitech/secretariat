//! `sec` — Secretariat CLI.

use anyhow::Result;
use clap::{Parser, Subcommand};

mod commands;

#[derive(Parser, Debug)]
#[command(name = "sec", version, about = "Secretariat — biometric-attested document stamping.")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// One-time setup: generate signing key, write ~/.secretariat/* defaults.
    Init(commands::init::Args),

    /// Scaffold an AG-shaped envelope into ~/.secretariat/outbox/.
    Compose(commands::compose::Args),

    /// Embed a biometric-attested stamp into a markdown file.
    Stamp(commands::stamp::Args),

    /// Verify a stamped markdown file against the signer's did:web document.
    Verify(commands::verify::Args),

    /// List inbox / outbox / recent stamps.
    List(commands::list::Args),

    /// Manage the contact book: add, list, show, remove peers.
    Contact(commands::contact::Args),

    /// Run the daemon: register with relays + serve the poll/send loop.
    Daemon(commands::daemon::Args),

    /// Decrypt + print the body of an envelope (encrypted or plaintext).
    Read(commands::read::Args),

    /// Create or claim invite tokens against the relay.
    Invite(commands::invite::Args),

    /// Wire `sec-mcp` into Claude Desktop / Claude Code (no JSON editing).
    Mcp(commands::mcp::Args),

    /// Manage the principal's display name (presence, distinct from identity).
    Profile(commands::profile::Args),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Init(a) => commands::init::run(a),
        Cmd::Compose(a) => commands::compose::run(a),
        Cmd::Stamp(a) => commands::stamp::run(a),
        Cmd::Verify(a) => commands::verify::run(a),
        Cmd::List(a) => commands::list::run(a),
        Cmd::Contact(a) => commands::contact::run(a),
        Cmd::Daemon(a) => commands::daemon::run(a),
        Cmd::Read(a) => commands::read::run(a),
        Cmd::Invite(a) => commands::invite::run(a),
        Cmd::Mcp(a) => commands::mcp::run(a),
        Cmd::Profile(a) => commands::profile::run(a),
    }
}
