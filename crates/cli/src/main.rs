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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Init(a) => commands::init::run(a),
        Cmd::Compose(a) => commands::compose::run(a),
        Cmd::Stamp(a) => commands::stamp::run(a),
        Cmd::Verify(a) => commands::verify::run(a),
        Cmd::List(a) => commands::list::run(a),
    }
}
