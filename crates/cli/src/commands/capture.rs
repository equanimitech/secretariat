//! `sec capture` — drop a body into a local queue.
//!
//! Captures are envelopes whose `recipient.owner == self_did`: same
//! primitive as a peer letter, but the routing rule keeps them on disk.
//! Use them for ideas, journal entries, future-self notes, agent bids —
//! anything to surface again at the next review session. Stamps are
//! optional (tamper-evident self-attestation), never required.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;

use secretariat_core::application::{capture_to_queue, show_org, CaptureRequest, CaptureRoots};
use secretariat_core::domain::{OrgAlias, QueueHandle};
use secretariat_core::infrastructure::org_store::org_channels_root;

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

    /// Optional org alias (`themia.pro`, `equanimi.tech`). When set AND
    /// the queue handle starts with `channel:`, the capture lands in
    /// that org's channel tree. Omit for personal captures.
    #[arg(long)]
    org: Option<String>,
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

    let channel_tree = match args.org.as_deref() {
        None => paths.channels.clone(),
        Some(s) => {
            let alias = OrgAlias::parse(s)
                .map_err(|e| anyhow!("invalid --org `{s}`: {e}"))?;
            if show_org(&paths.orgs_root, &alias)
                .context("looking up org")?
                .is_none()
            {
                return Err(anyhow!(
                    "org `{}` does not exist — create it with `sec orgs create {}` first",
                    alias.as_str(),
                    alias.as_str()
                ));
            }
            org_channels_root(&paths.orgs_root, &alias)
        }
    };
    let roots = CaptureRoots {
        flat_queues: &paths.queues,
        channel_tree: &channel_tree,
    };
    let path = capture_to_queue(req, roots, Utc::now())
        .context("writing capture into local queue")?;
    println!("{}", path.display());
    Ok(())
}
