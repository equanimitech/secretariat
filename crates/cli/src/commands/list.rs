//! `sec list` — list inbox / drafts / peers cache contents.

use anyhow::{Context, Result};
use clap::Parser;
use secretariat_core::application::{list_draft_files, list_inbox_files};
use std::fs;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    /// Which slice of the substrate to list.
    #[arg(value_enum, default_value_t = Target::Drafts)]
    target: Target,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum Target {
    Inbox,
    Drafts,
    Peers,
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    match args.target {
        Target::Inbox => {
            let listed = list_inbox_files(&paths.root)
                .with_context(|| format!("listing inbox under {}", paths.root.display()))?;
            for e in listed {
                println!("{}", e.file_path);
            }
        }
        Target::Drafts => {
            let listed = list_draft_files(&paths.root)
                .with_context(|| format!("listing drafts under {}", paths.root.display()))?;
            for e in listed {
                println!("{}", e.file_path);
            }
        }
        Target::Peers => {
            // Peers cache is still a flat directory of `did.json`
            // files keyed by `paths.peers_cache`. Walk it raw — this
            // is the diagnostic browse path, not the envelope surface.
            let dir = paths.peers_cache;
            if !dir.exists() {
                return Ok(());
            }
            walk(&dir, &dir).with_context(|| format!("listing {}", dir.display()))?;
        }
    }
    Ok(())
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
