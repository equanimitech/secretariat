//! `sec compose` — write a doc into a registered repo through the
//! substrate: placed by convention (`docs/<bucket>/<date>-<slug>.md`),
//! signed at birth with the scribe's `$signature`, committed
//! pathspec-scoped. The write-side of the three-layer trust model.
//!
//! Body comes from stdin by default, or `--body-file <path>`.

use std::io::Read as _;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;

use secretariat_core::application::compose_ops::ComposeRequest;
use secretariat_core::application::repo_ops::list_repos;
use secretariat_core::application::{compose_document, resolve_sole_scribe, DocType};
use secretariat_core::infrastructure::open_in_secretariat;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    /// Doc type: idea | pain | decision | pitch | note.
    #[arg(long = "type")]
    doc_type: String,
    /// Title — drives the slug and the commit message.
    #[arg(long)]
    title: String,
    /// Target repo (must be enrolled via `sec repo add`). Defaults to cwd.
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    /// Read the body from a file instead of stdin.
    #[arg(long)]
    body_file: Option<PathBuf>,
    /// Do not open the composed doc in the Secretariat desktop app
    /// (default is to open it; pass this for scripted/headless use).
    #[arg(long)]
    no_open: bool,
}

pub fn run(args: Args) -> Result<()> {
    let doc_type = DocType::parse(&args.doc_type).map_err(|e| anyhow!("invalid --type: {e}"))?;

    let body = match &args.body_file {
        Some(p) => std::fs::read_to_string(p)
            .with_context(|| format!("reading body file {}", p.display()))?,
        None => {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .context("reading body from stdin")?;
            buf
        }
    };

    let paths = key_paths()?;
    let (scribe_did, scribe_key) = resolve_sole_scribe(&paths).context("resolving the scribe")?;
    let registry = list_repos(&paths.preferences, None).context("loading repo registry")?;

    let outcome = compose_document(ComposeRequest {
        registry: &registry,
        repo_path: &args.repo,
        doc_type,
        title: &args.title,
        body: &body,
        signer: scribe_did,
        signing_key: &scribe_key,
        now: Utc::now(),
    })?;

    println!("{}", outcome.path.display());
    if outcome.committed {
        eprintln!(
            "[sec] composed + signed + committed: docs({doc_type}): {}",
            args.title
        );
    } else {
        eprintln!(
            "[sec] composed + signed; commit skipped: {}",
            outcome
                .commit_skipped
                .unwrap_or_else(|| "unknown reason".to_string())
        );
    }

    // Open the fresh doc in the desktop app so the author sees it land.
    // Best-effort: a missing GUI session (headless/CI) must not fail compose.
    if !args.no_open {
        if let Err(e) = open_in_secretariat(&outcome.path) {
            eprintln!("[sec] composed but could not open in the app: {e}");
        }
    }
    Ok(())
}
