//! `sec compose` — scaffold an envelope into the outbox.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;

use secretariat_core::application::{compose_envelope, ComposeRequest};
use secretariat_core::domain::{Did, EnvelopeDepth, EnvelopeUrgency};

use super::paths::{key_paths, load_did};

#[derive(Parser, Debug)]
pub struct Args {
    /// Recipient DID. If omitted, the envelope is self-addressed
    /// (lands in `outbox/_self/`).
    #[arg(long)]
    to: Option<String>,

    /// Sender DID. Defaults to the principal's DID, derived from the seeded
    /// did.json. Pass `--from` only if you maintain multiple identities.
    #[arg(long)]
    from: Option<String>,

    /// Declared depth.
    #[arg(long, value_enum, default_value_t = DepthArg::Subtle)]
    depth: DepthArg,

    /// Declared urgency.
    #[arg(long, value_enum, default_value_t = UrgencyArg::Soon)]
    urgency: UrgencyArg,

    /// Free-form origin string (e.g. claude session id).
    #[arg(long, default_value_t = String::from("manual"))]
    source: String,

    /// Optional cadence hint for the receiver (e.g. "morning", "weekly").
    #[arg(long)]
    cadence_hint: Option<String>,

    /// Raw markdown body. When supplied, replaces the AG template entirely —
    /// the caller is responsible for shape. When omitted, the user's
    /// `~/.secretariat/template.md` scaffold is used.
    #[arg(long)]
    body: Option<String>,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum DepthArg {
    Gross,
    Subtle,
}

impl From<DepthArg> for EnvelopeDepth {
    fn from(v: DepthArg) -> Self {
        match v {
            DepthArg::Gross => EnvelopeDepth::Gross,
            DepthArg::Subtle => EnvelopeDepth::Subtle,
        }
    }
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum UrgencyArg {
    Now,
    Soon,
    Whenever,
}

impl From<UrgencyArg> for EnvelopeUrgency {
    fn from(v: UrgencyArg) -> Self {
        match v {
            UrgencyArg::Now => EnvelopeUrgency::Now,
            UrgencyArg::Soon => EnvelopeUrgency::Soon,
            UrgencyArg::Whenever => EnvelopeUrgency::Whenever,
        }
    }
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;

    let from = match args.from {
        Some(s) => Did::parse(s).map_err(|e| anyhow!("invalid --from: {e}"))?,
        None => load_did(&paths)?,
    };
    let to = args
        .to
        .map(Did::parse)
        .transpose()
        .map_err(|e| anyhow!("invalid --to: {e}"))?;

    let req = ComposeRequest {
        from,
        to,
        depth: args.depth.into(),
        urgency: args.urgency.into(),
        source: args.source,
        cadence_hint: args.cadence_hint,
        body: args.body,
    };

    let path = compose_envelope(req, &paths.template, &paths.outbox, Utc::now())
        .context("composing envelope")?;
    println!("{}", path.display());
    Ok(())
}

