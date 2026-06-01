//! `sec repo` — manage the substrate manifest (`preferences.toml` `[[repos]]`).
//!
//! - `sec repo add <path> [--role project|home] [--tag <t>]...`
//! - `sec repo list [--tag <t>] [--json]`
//! - `sec repo remove <path>`

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

use secretariat_core::application::repo_ops;
use secretariat_core::infrastructure::RepoRole;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Enroll (or update) a repo in the substrate manifest.
    Add {
        /// Path to the repo (must be a git repo).
        path: PathBuf,
        /// project (default) or home. `home` = private cross-cutting PKM.
        #[arg(long, default_value = "project")]
        role: String,
        /// Free-form grouping tag; repeatable (e.g. --tag themia).
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// List enrolled repos.
    List {
        /// Only repos carrying this tag.
        #[arg(long)]
        tag: Option<String>,
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Unenroll a repo by path.
    Remove {
        /// Path to the repo to unenroll.
        path: PathBuf,
    },
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;
    match args.cmd {
        Cmd::Add { path, role, tags } => add(&paths.preferences, path, role, tags),
        Cmd::List { tag, json } => list(&paths.preferences, tag, json),
        Cmd::Remove { path } => remove(&paths.preferences, path),
    }
}

fn add(prefs: &std::path::Path, path: PathBuf, role: String, tags: Vec<String>) -> Result<()> {
    let role = RepoRole::parse(&role).map_err(|e| anyhow!("invalid role: {e}"))?;
    let entry = repo_ops::register_repo(prefs, &path, role, tags).context("enrolling repo")?;
    eprintln!(
        "[sec] repo enrolled: {} ({}){}",
        entry.path.display(),
        entry.role.as_str(),
        if entry.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", entry.tags.join(", "))
        }
    );
    Ok(())
}

fn list(prefs: &std::path::Path, tag: Option<String>, json: bool) -> Result<()> {
    let repos = repo_ops::list_repos(prefs, tag.as_deref()).context("listing repos")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&repos)?);
        return Ok(());
    }
    if repos.is_empty() {
        eprintln!("[sec] no repos enrolled — `sec repo add <path>` to enroll one");
        return Ok(());
    }
    for r in &repos {
        println!(
            "{role}\t{tags}\t{path}",
            role = r.role.as_str(),
            tags = r.tags.join(","),
            path = r.path.display()
        );
    }
    Ok(())
}

fn remove(prefs: &std::path::Path, path: PathBuf) -> Result<()> {
    let removed = repo_ops::unregister_repo(prefs, &path).context("unenrolling repo")?;
    if removed {
        eprintln!("[sec] repo unenrolled: {}", path.display());
    } else {
        eprintln!("[sec] not enrolled: {}", path.display());
    }
    Ok(())
}
