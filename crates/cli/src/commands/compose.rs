//! `sec compose` — scaffold a draft envelope into the recipient queue's `_drafts/`.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;

use secretariat_core::application::{compose_envelope_with_ag, ComposeRequest};
use secretariat_core::infrastructure::preferences::load_or_migrate as load_or_migrate_preferences;
use secretariat_core::infrastructure::queue_dir::AliasMap;
use secretariat_core::domain::{
    Did, EnvelopeDepth, EnvelopeUrgency, QueueHandle, Recipient,
};

use super::paths::{key_paths, load_did};

#[derive(Parser, Debug)]
pub struct Args {
    /// Peer recipient DID. Required — for self-captures use `sec capture`.
    #[arg(long)]
    to: String,

    /// Recipient queue handle on the peer's machine. Defaults to
    /// `inbox:default` (the conventional handle for direct messages).
    /// Use a different value to address a non-default queue, e.g. a
    /// channel the peer publishes (`channel:book-progress`).
    #[arg(long, default_value_t = String::from("inbox:default"))]
    handle: String,

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

    /// Optional headline (AG gross signal — 2-6 words). When supplied,
    /// the AI auto-fill pass stands down for the envelope. When all
    /// three AG flags are omitted and a cognition adapter is configured,
    /// the scribe drafts `title` / `lede` / `summary` and tags
    /// `ag_source = "ai"`.
    #[arg(long)]
    title: Option<String>,

    /// Optional one-line lede (AG subtle layer). Supplying any one of
    /// `--title` / `--lede` / `--summary` makes the envelope
    /// author-attributed and disables auto-fill.
    #[arg(long)]
    lede: Option<String>,

    /// Optional multi-sentence summary (AG deepening pathway). See
    /// `--lede` for the auto-fill interaction.
    #[arg(long)]
    summary: Option<String>,
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
    let owner = Did::parse(&args.to).map_err(|e| anyhow!("invalid --to: {e}"))?;
    let handle = QueueHandle::parse(&args.handle)
        .map_err(|e| anyhow!("invalid --handle `{}`: {e}", args.handle))?;
    let recipient = Recipient::new(owner, handle);

    let req = ComposeRequest {
        from,
        recipient,
        depth: args.depth.into(),
        urgency: args.urgency.into(),
        source: args.source,
        cadence_hint: args.cadence_hint,
        body: args.body,
        title: args.title,
        lede: args.lede,
        summary: args.summary,
    };

    let self_did = load_did(&paths)?;
    let aliases = AliasMap::load(self_did, &paths).context("loading alias map")?;
    let prefs = load_or_migrate_preferences(
        &paths.preferences,
        &paths.legacy_cognition_config,
        &paths.legacy_cadence,
    )
    .unwrap_or_default();
    let runtime = tokio::runtime::Runtime::new().context("starting tokio runtime")?;
    let path = runtime
        .block_on(compose_envelope_with_ag(
            req,
            &paths.template,
            &paths.root,
            &aliases,
            &prefs.cognition,
            Utc::now(),
        ))
        .context("composing envelope")?;
    println!("{}", path.display());
    Ok(())
}

