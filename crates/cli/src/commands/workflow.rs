//! `sec workflow` — inspect + fire `.secretariat/workflows/*.md`.
//!
//! - `sec workflow list [<repo>]`            — parsed workflows in a repo
//! - `sec workflow run <doc> [--dry-run]`    — fire matching workflows for a doc

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

use secretariat_core::application::workflow_ops;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// List parsed workflows in a repo (defaults to cwd).
    List { repo: Option<PathBuf> },
    /// Fire workflows matching a doc. `--dry-run` renders without dispatching.
    Run {
        /// Path to the stamped doc.
        doc: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;
    match args.cmd {
        Cmd::List { repo } => list(repo),
        Cmd::Run { doc, dry_run } => run_doc(&paths.preferences, doc, dry_run),
    }
}

fn list(repo: Option<PathBuf>) -> Result<()> {
    let repo = match repo {
        Some(r) => r,
        None => std::env::current_dir()?,
    };
    let wfs = workflow_ops::load_workflows(&repo).context("loading workflows")?;
    if wfs.is_empty() {
        eprintln!(
            "[sec] no workflows in {}/.secretariat/workflows/",
            repo.display()
        );
        return Ok(());
    }
    for w in &wfs {
        println!(
            "{name}\ton={on:?}\ttype={types:?}\ttags={tags:?}",
            name = w.name,
            on = w.trigger.on,
            types = w.trigger.match_.types,
            tags = w.trigger.match_.tags,
        );
    }
    Ok(())
}

/// Walk up from `doc` to the nearest enclosing git repo root.
fn repo_root_of(doc: &Path) -> Result<PathBuf> {
    let abs = std::fs::canonicalize(doc).context("resolving doc path")?;
    let mut dir = abs.parent();
    while let Some(d) = dir {
        if d.join(".git").exists() {
            return Ok(d.to_path_buf());
        }
        dir = d.parent();
    }
    Err(anyhow!("{} is not inside a git repo", doc.display()))
}

fn run_doc(prefs: &Path, doc: PathBuf, dry_run: bool) -> Result<()> {
    let repo = repo_root_of(&doc)?;
    let abs_doc = std::fs::canonicalize(&doc)?;
    let doc_rel = abs_doc
        .strip_prefix(&repo)
        .context("doc not under its repo root")?;
    let hits = workflow_ops::match_workflows(prefs, &repo, doc_rel).context("matching workflows")?;
    if hits.is_empty() {
        eprintln!("[sec] no workflows match {}", doc.display());
        return Ok(());
    }
    for w in &hits {
        if dry_run {
            println!("--- would dispatch workflow `{}` ---", w.name);
            println!("cwd: {}", repo.display());
            println!("doc: {}", doc_rel.display());
            println!("prompt:\n{}", w.prompt);
        } else {
            dispatch(prefs, &repo, doc_rel, &w.name, &w.prompt)?;
        }
    }
    Ok(())
}

/// Run the configured cognition CLI (`launch_command`) headless with the
/// workflow prompt, cwd = the repo, the doc path injected so the scribe knows
/// what it is acting on.
fn dispatch(prefs: &Path, repo: &Path, doc_rel: &Path, name: &str, prompt: &str) -> Result<()> {
    use secretariat_core::infrastructure::preferences::Preferences;

    let cog = Preferences::load(prefs).context("loading preferences")?.cognition;
    let full_prompt = format!(
        "Document: {doc}\n(The current working directory is the repo root; read \
         the document there.)\n\n{prompt}",
        doc = doc_rel.display(),
    );
    let mut cmd = std::process::Command::new(&cog.launch_command);
    cmd.args(&cog.launch_args)
        .arg("-p")
        .arg(&full_prompt)
        .current_dir(repo);
    for (k, v) in &cog.launch_env {
        cmd.env(k, v);
    }
    eprintln!("[sec] dispatching `{name}` via `{}`…", cog.launch_command);
    let status = cmd
        .status()
        .with_context(|| format!("dispatching `{name}` via `{}`", cog.launch_command))?;
    if !status.success() {
        return Err(anyhow!("workflow `{name}` exited with {status}"));
    }
    eprintln!("[sec] workflow `{name}` done");
    Ok(())
}
