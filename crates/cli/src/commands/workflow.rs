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
    let ledger = paths.root.join("usage.jsonl");
    match args.cmd {
        Cmd::List { repo } => list(repo),
        Cmd::Run { doc, dry_run } => run_doc(&paths.preferences, &ledger, doc, dry_run),
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

fn run_doc(prefs: &Path, ledger: &Path, doc: PathBuf, dry_run: bool) -> Result<()> {
    let repo = repo_root_of(&doc)?;
    let abs_doc = std::fs::canonicalize(&doc)?;
    let doc_rel = abs_doc
        .strip_prefix(&repo)
        .context("doc not under its repo root")?;
    let hits =
        workflow_ops::match_workflows(prefs, &repo, doc_rel).context("matching workflows")?;
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
            dispatch(prefs, ledger, &repo, doc_rel, &w.name, &w.prompt)?;
        }
    }
    Ok(())
}

/// Run the configured cognition CLI (`launch_command`) headless with the
/// workflow prompt, cwd = the repo, the doc path injected. Captures the JSON
/// result envelope to record the dispatch's cost + tokens in the usage ledger.
fn dispatch(
    prefs: &Path,
    ledger: &Path,
    repo: &Path,
    doc_rel: &Path,
    name: &str,
    prompt: &str,
) -> Result<()> {
    use secretariat_core::infrastructure::preferences::Preferences;
    use secretariat_core::infrastructure::usage_ledger::{
        append, now_epoch_secs, parse_cli_usage, UsageRecord,
    };

    let cog = Preferences::load(prefs)
        .context("loading preferences")?
        .cognition;
    let full_prompt = format!(
        "Document: {doc}\n(The current working directory is the repo root; read \
         the document there.)\n\n{prompt}",
        doc = doc_rel.display(),
    );
    let mut cmd = std::process::Command::new(&cog.launch_command);
    cmd.args(&cog.launch_args)
        .arg("-p")
        .arg(&full_prompt)
        .arg("--output-format")
        .arg("json")
        .current_dir(repo);
    for (k, v) in &cog.launch_env {
        cmd.env(k, v);
    }
    eprintln!("[sec] dispatching `{name}` via `{}`…", cog.launch_command);
    let out = cmd
        .output()
        .with_context(|| format!("dispatching `{name}` via `{}`", cog.launch_command))?;
    if !out.status.success() {
        return Err(anyhow!(
            "workflow `{name}` exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    if let Some((cost, input, output)) = parse_cli_usage(&stdout) {
        let _ = append(
            ledger,
            &UsageRecord {
                at: now_epoch_secs(),
                source: format!("workflow:{name}"),
                repo: repo.display().to_string(),
                doc: doc_rel.display().to_string(),
                cost_usd: cost,
                input_tokens: input,
                output_tokens: output,
            },
        );
        eprintln!("[sec] workflow `{name}` done — ${cost:.4} ({input} in / {output} out)");
    } else {
        eprintln!("[sec] workflow `{name}` done (no usage data in output)");
    }
    Ok(())
}
