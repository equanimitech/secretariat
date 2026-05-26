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

    /// Scaffold an AG-shaped envelope into the recipient queue's
    /// `envelopes/YYYY/MM/DD/` day-shard. Draft state is the absence
    /// of the `delivered:` frontmatter field.
    Compose(commands::compose::Args),

    /// Capture a body of text into a local queue (idea, journal, future-self note).
    Capture(commands::capture::Args),

    /// List and read the local channel tree (channel:foo:bar handles).
    Channels(commands::channels::Args),

    /// CRUD over organizations the principal owns locally.
    Orgs(commands::orgs::Args),

    /// Embed a biometric-attested stamp into a markdown file.
    Stamp(commands::stamp::Args),

    /// Verify a stamped markdown file against the signer's did:web document.
    Verify(commands::verify::Args),

    /// List inbox / drafts / peers cache.
    List(commands::list::Args),

    /// Run the daemon: register with relays + serve the poll/send loop.
    Daemon(commands::daemon::Args),

    /// Decrypt + print the body of an envelope (encrypted or plaintext).
    Read(commands::read::Args),

    /// Create or claim invite tokens against the relay.
    Invite(commands::invite::Args),

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
        Cmd::Compose(a) => commands::compose::run(a),
        Cmd::Capture(a) => commands::capture::run(a),
        Cmd::Channels(a) => commands::channels::run(a),
        Cmd::Orgs(a) => commands::orgs::run(a),
        Cmd::Stamp(a) => commands::stamp::run(a),
        Cmd::Verify(a) => commands::verify::run(a),
        Cmd::List(a) => commands::list::run(a),
        Cmd::Daemon(a) => commands::daemon::run(a),
        Cmd::Read(a) => commands::read::run(a),
        Cmd::Invite(a) => commands::invite::run(a),
        Cmd::Launch(a) => commands::launch::run(a),
        Cmd::Mcp(a) => commands::mcp::run(a),
        Cmd::Profile(a) => commands::profile::run(a),
        Cmd::View(a) => commands::view::run(a),
    }
}
