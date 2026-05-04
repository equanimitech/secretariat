//! `sec profile` — manage the principal's display name.
//!
//! Profile is *presence* (a human-readable name) and is distinct from
//! identity (the DID). Stored locally only at `~/.secretariat/profile.json`.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

use secretariat_core::domain::DisplayName;
use secretariat_core::infrastructure::profile_store::{
    load_profile, save_profile, PrincipalProfile,
};

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print the current display name. Exits 1 when no profile is set yet.
    Show,
    /// Set (or replace) the principal's display name.
    Set {
        /// Human-readable name. e.g. "Rafa", "Christophe Marchand".
        name: String,
    },
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;
    match args.cmd {
        Cmd::Show => show(&paths),
        Cmd::Set { name } => set(&paths, &name),
    }
}

fn show(paths: &secretariat_core::infrastructure::keys::KeyPaths) -> Result<()> {
    match load_profile(&paths.profile).context("loading profile")? {
        Some(p) => {
            println!("{}", p.display_name);
            Ok(())
        }
        None => {
            eprintln!("[sec] no profile set — run `sec profile set <name>`");
            std::process::exit(1);
        }
    }
}

fn set(
    paths: &secretariat_core::infrastructure::keys::KeyPaths,
    name: &str,
) -> Result<()> {
    let parsed = DisplayName::parse(name).map_err(|e| anyhow!("invalid name: {e}"))?;
    let profile = PrincipalProfile {
        display_name: parsed.clone(),
    };
    save_profile(&paths.profile, &profile).context("saving profile")?;
    eprintln!("[sec] profile saved: {}", parsed);
    Ok(())
}
