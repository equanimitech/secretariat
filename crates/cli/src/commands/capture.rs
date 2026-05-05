//! `sec capture` — drop a body into a local queue.
//!
//! Local-queue captures (substrate v0.3) are envelopes addressed to
//! `Recipient::LocalQueue(handle)` rather than to a peer. They never
//! leave the principal's machine and cannot be stamped — by domain
//! invariant. Use them for ideas, journal entries, future-self notes,
//! agent bids, anything you want to surface again at the next review
//! session.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;

use secretariat_core::application::{capture_to_queue, CaptureRequest};
use secretariat_core::domain::QueueHandle;

use super::paths::{key_paths, load_did};

#[derive(Parser, Debug)]
pub struct Args {
    /// Target queue handle, of the form `<namespace>:<slug>`. Examples:
    /// `inbox:triage`, `area:health`, `project:autonomous-enterprise`.
    /// Namespaces are free-form lowercase letters; slugs are lowercase
    /// letters / digits / hyphens.
    #[arg(long)]
    queue: String,

    /// Body of the capture. Pass either inline as a single string, or
    /// read from stdin with `--stdin`.
    #[arg(long, conflicts_with = "stdin")]
    body: Option<String>,

    /// Read the body from stdin (terminated by EOF).
    #[arg(long, default_value_t = false)]
    stdin: bool,

    /// Free-form origin marker, e.g. `idea-skill`, `quick-pane`,
    /// `mcp-capture`. Surfaces in the review session as grouping hint.
    #[arg(long, default_value_t = String::from("manual"))]
    source: String,
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;
    let from = load_did(&paths)?;

    let queue = QueueHandle::parse(&args.queue)
        .map_err(|e| anyhow!("invalid --queue `{}`: {e}", args.queue))?;

    let body = match (args.body, args.stdin) {
        (Some(b), false) => b,
        (None, true) => {
            use std::io::Read;
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .context("reading body from stdin")?;
            s
        }
        (Some(_), true) => unreachable!("clap conflicts_with prevents this"),
        (None, false) => {
            return Err(anyhow!(
                "must supply either --body <text> or --stdin"
            ));
        }
    };

    let req = CaptureRequest {
        from,
        queue,
        body,
        source: args.source,
    };

    let path = capture_to_queue(req, &paths.queues, Utc::now())
        .context("writing capture into local queue")?;
    println!("{}", path.display());
    Ok(())
}
