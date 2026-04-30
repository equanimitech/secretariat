//! `sec list` — list inbox / outbox / peers cache contents.

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    /// Which directory to list.
    #[arg(value_enum, default_value_t = Target::Outbox)]
    target: Target,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum Target {
    Inbox,
    Outbox,
    Peers,
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    let dir = match args.target {
        Target::Inbox => paths.inbox,
        Target::Outbox => paths.outbox,
        Target::Peers => paths.peers_cache,
    };
    if !dir.exists() {
        return Ok(());
    }
    walk(&dir, &dir).with_context(|| format!("listing {}", dir.display()))
}

fn walk(root: &std::path::Path, dir: &std::path::Path) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        if path.is_dir() {
            println!("{}/", rel.display());
            walk(root, &path)?;
        } else {
            println!("{}", rel.display());
        }
    }
    Ok(())
}
