//! `sec profile` — manage the principal's display name + full name.
//!
//! Display name is presence (UI surfaces). Full name is the formal
//! variant (envelope signatures, legal artifacts). Both fields live
//! in the principal's `identity.md` frontmatter.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

use secretariat_core::domain::DisplayName;
use secretariat_core::infrastructure::identity_store::{load_identity, save_identity};
use secretariat_core::infrastructure::keys::load_signing_key;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print the current display name (exits 1 when no identity is set yet).
    Show,
    /// Set (or replace) the principal's display name.
    Set {
        /// Human-readable name. e.g. "Rafa", "Christophe Marchand".
        name: String,
        /// Optional formal name for envelope signatures + legal artifacts.
        #[arg(long)]
        full_name: Option<String>,
    },
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;
    match args.cmd {
        Cmd::Show => show(&paths),
        Cmd::Set { name, full_name } => set(&paths, &name, full_name.as_deref()),
    }
}

fn show(paths: &secretariat_core::infrastructure::keys::KeyPaths) -> Result<()> {
    match load_identity(&paths.identity_md).context("loading identity")? {
        Some(id) => {
            println!("{}", id.display_name);
            if let Some(fname) = id.full_name {
                println!("({})", fname);
            }
            Ok(())
        }
        None => {
            eprintln!("[sec] no identity yet — run `sec init` first");
            std::process::exit(1);
        }
    }
}

fn set(
    paths: &secretariat_core::infrastructure::keys::KeyPaths,
    name: &str,
    full_name: Option<&str>,
) -> Result<()> {
    let parsed = DisplayName::parse(name).map_err(|e| anyhow!("invalid name: {e}"))?;
    let mut identity = load_identity(&paths.identity_md)
        .context("loading identity")?
        .ok_or_else(|| anyhow!("no identity yet — run `sec init` first"))?;
    identity.display_name = parsed.clone();
    if let Some(fname) = full_name {
        identity.full_name = Some(fname.to_string());
    }
    let signing_key =
        load_signing_key(&paths.signing_key).context("loading signing key for identity re-sign")?;
    save_identity(&paths.identity_md, &identity, Some(&signing_key))
        .context("saving identity")?;
    eprintln!("[sec] profile saved: {}", parsed);
    Ok(())
}
