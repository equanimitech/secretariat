//! `sec compose` — scaffold a draft envelope directly into the recipient
//! queue's `envelopes/YYYY/MM/DD/` tree. The envelope's frontmatter omits
//! `delivered:` — absence is the substrate's draft signal (substrate-for-
//! themia Move 4).

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::Parser;

use secretariat_core::application::{
    compose_envelope_with_ag, ComposeRequest, ComposeSigner,
};
use secretariat_core::infrastructure::identity_store::load_identity;
use secretariat_core::infrastructure::keys::load_signing_key;
use secretariat_core::infrastructure::preferences::load_or_migrate as load_or_migrate_preferences;
use secretariat_core::infrastructure::queue_dir::AliasMap;
use secretariat_core::domain::{
    Agent, AgentRole, Did, EnvelopeDepth, EnvelopeUrgency, QueueHandle, Recipient, SignerRole,
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

    /// Sign with a specific authorized agent (substrate-for-themia
    /// Move 2). Defaults to the first scribe in the principal's
    /// `authorized_agents`. If no scribe is configured, falls back to
    /// the principal's own key + `signer_role: principal`.
    #[arg(long)]
    agent: Option<String>,
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
    let aliases =
        AliasMap::load(self_did.clone(), &paths).context("loading alias map")?;
    let prefs = load_or_migrate_preferences(
        &paths.preferences,
        &paths.legacy_cognition_config,
        &paths.legacy_cadence,
    )
    .unwrap_or_default();

    // Resolve the signing context per substrate-for-themia Move 2:
    // prefer the agent named by `--agent`, then the first scribe in
    // `authorized_agents`, then fall back to the principal's own key.
    let signing_ctx = resolve_compose_signer(&paths, &self_did, args.agent.as_deref())?;
    let signer = ComposeSigner::new(
        signing_ctx.signer_did,
        signing_ctx.signer_role,
        &signing_ctx.signing_key,
    );

    let runtime = tokio::runtime::Runtime::new().context("starting tokio runtime")?;
    let path = runtime
        .block_on(compose_envelope_with_ag(
            req,
            &signer,
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

/// Resolved signing context: owned key + DID + role. Built once at
/// command entry; lives across the async compose call.
struct ResolvedSigner {
    signer_did: Did,
    signer_role: SignerRole,
    signing_key: ed25519_dalek::SigningKey,
}

fn resolve_compose_signer(
    paths: &secretariat_core::infrastructure::keys::KeyPaths,
    self_did: &Did,
    agent_name: Option<&str>,
) -> Result<ResolvedSigner> {
    let identity = load_identity(&paths.identity_md)
        .context("loading identity")?
        .ok_or_else(|| anyhow!("no identity at {} — run `sec init` first", paths.identity_md.display()))?;

    // Pick the agent record we'll sign with.
    let chosen_agent: Option<&Agent> = match agent_name {
        Some(name) => {
            let found = identity
                .authorized_agents
                .iter()
                .find(|a| a.name.as_str() == name)
                .ok_or_else(|| {
                    anyhow!(
                        "no authorized agent named `{name}` — check `sec agent list`"
                    )
                })?;
            Some(found)
        }
        None => identity
            .authorized_agents
            .iter()
            .find(|a| a.role == AgentRole::Scribe),
    };

    match chosen_agent {
        Some(agent) => {
            let key_path = paths.agent_signing_key_path(agent.name.as_str());
            let key = load_signing_key(&key_path).with_context(|| {
                format!(
                    "loading agent signing key at {}",
                    key_path.display()
                )
            })?;
            Ok(ResolvedSigner {
                signer_did: agent.did.clone(),
                signer_role: SignerRole::Agent,
                signing_key: key,
            })
        }
        None => {
            // Fallback: principal signs with their own key. Substrate-
            // for-themia Move 2 prefers an agent, but a principal who
            // hasn't run `sec agent add` should still be able to compose.
            let key = load_signing_key(&paths.signing_key)
                .context("loading principal signing key")?;
            Ok(ResolvedSigner {
                signer_did: self_did.clone(),
                signer_role: SignerRole::Principal,
                signing_key: key,
            })
        }
    }
}

