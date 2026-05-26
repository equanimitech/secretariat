//! `sec invite` — create + claim invite tokens against the relay.
//!
//! ```text
//! sec invite create [--purpose <text>] [--ttl-hours <n>] [--endpoint <url>]
//! sec invite claim <claim-url> [--name <inviter-display-name>]
//! ```
//!
//! Mirrors the MCP `invite_create` / `invite_claim` tools — same parameters,
//! same return shape (per AGENTS.md "Every principal-facing primitive ships
//! on both interfaces").

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};

use secretariat_core::application::{
    claim_invite, create_invite, persist_org_membership, view_invite, AcceptMembershipRequest,
    OrgInviteContext, DEFAULT_INVITE_TTL_HOURS,
};
use secretariat_core::domain::{OrgAlias, QueueHandle, ScopeIntent};
use secretariat_core::infrastructure::keys::load_signing_key;
use secretariat_core::infrastructure::transport::RelayState;

use super::paths::{key_paths, load_did};

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Create a one-shot invite at the relay. The relay endpoint defaults
    /// to the first registered relay in `~/.secretariat/relay-state.json`
    /// (set by `sec daemon register`); pass `--endpoint` to override.
    Create {
        /// Free-form purpose hint (e.g. "first-contact").
        #[arg(long)]
        purpose: Option<String>,

        /// Token TTL in hours. Default: 168 (7 days). Server caps at 720
        /// (30 days).
        #[arg(long)]
        ttl_hours: Option<i64>,

        /// Override the relay endpoint. Otherwise uses the first one from
        /// `relay-state.json`.
        #[arg(long)]
        endpoint: Option<String>,

        /// Org alias granted by this invite (e.g. `equanimi.tech`). When
        /// set, the claimant joins this org on accept. Pair with `--role`
        /// and `--channels`.
        #[arg(long)]
        org: Option<String>,

        /// Role granted on org channels: `subscribe` / `publish` /
        /// `collaborator` / `admin`. Required when `--org` is set.
        #[arg(long)]
        role: Option<String>,

        /// Channel scope. Accepts:
        ///   `*`             → live org participant (every current + future channel)
        ///   `<handle>`      → that handle plus its subtree
        ///   `h1,h2,h3`      → exactly this list, no future additions
        /// Required when `--org` is set.
        #[arg(long)]
        channels: Option<String>,

        /// Relay endpoint where the org's channels live. Defaults to
        /// the invite-creation endpoint.
        #[arg(long)]
        channel_relay_endpoint: Option<String>,
    },

    /// Claim an invite. Auto-registers your DID with the relay if not yet
    /// registered.
    Claim {
        /// Claim URL the inviter shared, e.g.
        /// `https://secretariat.equanimi.tech/v0/invite/<token>`.
        url: String,

        /// Kept for backward compatibility. The local contact book was
        /// removed in the substrate-for-themia slice (Move 3b); this
        /// flag is now a no-op.
        #[arg(long, hide = true)]
        name: Option<String>,
    },
}

pub fn run(args: Args) -> Result<()> {
    match args.cmd {
        Cmd::Create {
            purpose,
            ttl_hours,
            endpoint,
            org,
            role,
            channels,
            channel_relay_endpoint,
        } => run_create(
            purpose.as_deref(),
            ttl_hours,
            endpoint.as_deref(),
            org.as_deref(),
            role.as_deref(),
            channels.as_deref(),
            channel_relay_endpoint.as_deref(),
        ),
        Cmd::Claim { url, name } => run_claim(&url, name.as_deref()),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_create(
    purpose: Option<&str>,
    ttl_hours: Option<i64>,
    endpoint_override: Option<&str>,
    org: Option<&str>,
    role: Option<&str>,
    channels: Option<&str>,
    channel_relay_endpoint_override: Option<&str>,
) -> Result<()> {
    let paths = key_paths()?;
    let did = load_did(&paths)?;
    let key = load_signing_key(&paths.signing_key)
        .with_context(|| format!("loading signing key from {}", paths.signing_key.display()))?;

    let endpoint = match endpoint_override {
        Some(s) => s.to_string(),
        None => first_registered_relay(&paths.relay_state)?,
    };

    // Build optional org context.
    let org_ctx = match (org, role, channels) {
        (None, None, None) => None,
        (Some(alias_str), Some(role_str), Some(channels_str)) => {
            let alias = OrgAlias::parse(alias_str)
                .map_err(|e| anyhow!("invalid org alias `{alias_str}`: {e}"))?;
            let org_entry = secretariat_core::application::show_org(&paths.orgs_root, &alias)
                .context("looking up org")?
                .ok_or_else(|| anyhow!("org `{}` not found locally", alias.as_str()))?;
            let org_did = org_entry.did.ok_or_else(|| {
                anyhow!(
                    "org `{}` has no DID — invite would carry no `org_did` field",
                    alias.as_str()
                )
            })?;
            let (scope_intent, handles) = parse_channels_spec(channels_str)?;
            Some(OrgInviteContext {
                org_did,
                org_alias: alias.as_str().to_string(),
                role: role_str.to_string(),
                channel_handles: handles,
                channel_relay_endpoint: Some(
                    channel_relay_endpoint_override
                        .unwrap_or(&endpoint)
                        .to_string(),
                ),
                scope_intent,
            })
        }
        _ => {
            return Err(anyhow!(
                "--org / --role / --channels must be passed together for org invites"
            ));
        }
    };

    let invite =
        create_invite(&endpoint, &did, &key, purpose, ttl_hours, org_ctx.as_ref())
            .context("creating invite at relay")?;

    eprintln!("[sec] invite created");
    eprintln!("[sec]   token:      {}", invite.token);
    eprintln!("[sec]   expires at: {}", invite.expires_at.to_rfc3339());
    eprintln!("[sec]   purpose:    {}", purpose.unwrap_or("(none)"));
    eprintln!();
    println!("{}", invite.claim_url);
    eprintln!();
    eprintln!("Share that URL with the peer you're inviting. They'll run:");
    eprintln!("    sec invite claim {}", invite.claim_url);
    eprintln!();
    eprintln!(
        "(Default TTL: {} hours. Override with --ttl-hours.)",
        ttl_hours.unwrap_or(DEFAULT_INVITE_TTL_HOURS)
    );
    Ok(())
}

fn run_claim(url: &str, _name_override: Option<&str>) -> Result<()> {
    let paths = key_paths()?;
    let did = load_did(&paths)?;
    let key = load_signing_key(&paths.signing_key)
        .with_context(|| format!("loading signing key from {}", paths.signing_key.display()))?;

    // Preview first so the operator can see who's inviting before they sign.
    let preview = view_invite(url).context("fetching invite preview")?;
    if let Some(claimed_by) = &preview.claimed_by {
        return Err(anyhow!(
            "invite has already been claimed (by {claimed_by}); ask the inviter for a new one"
        ));
    }
    eprintln!("[sec] invite preview");
    eprintln!("[sec]   inviter:    {}", preview.inviter_did);
    eprintln!("[sec]   expires at: {}", preview.expires_at.to_rfc3339());
    if let Some(p) = &preview.purpose {
        eprintln!("[sec]   purpose:    {p}");
    }

    // Actually claim.
    let claimed = claim_invite(url, &did, &key).context("claiming invite at relay")?;
    eprintln!();
    eprintln!("[sec] claim accepted by relay");
    eprintln!("[sec]   inviter:        {}", claimed.inviter_did);
    eprintln!("[sec]   you (claimant): {}", claimed.claimant_did);
    eprintln!(
        "[sec]   claimed at:     {}",
        claimed.claimed_at.to_rfc3339()
    );
    if claimed.registered {
        eprintln!("[sec]   relay also registered your DID (single-shot setup).");
    }

    // Persist the relay endpoint in relay-state if not already present
    // (so `sec daemon serve` will poll it). Contact-book auto-add was
    // removed in the substrate-for-themia slice (Move 3b).
    let endpoint_url = relay_origin_from_claim_url(url)?;
    if let Ok(mut state) = RelayState::load(&paths.relay_state) {
        let entry = state.entry_mut(&endpoint_url);
        entry.registered = true;
        let _ = state.save(&paths.relay_state);
    }

    // Slice A': org-flavored invite — persist the org membership so the
    // daemon's poll loop starts pulling the org's `_meta` queue. The
    // first poll cycle picks up `_meta` history → channelDef ingest
    // hydrates the channel tree (the "eager bootstrap" the pitch
    // promises is the *first* sync_now tick after this).
    if let (Some(org_did), Some(org_alias), Some(role)) = (
        claimed.org_did.as_ref(),
        claimed.org_alias.as_ref(),
        claimed.role.as_ref(),
    ) {
        let relay_endpoint = claimed
            .channel_relay_endpoint
            .clone()
            .unwrap_or_else(|| endpoint_url.clone());
        let request = AcceptMembershipRequest {
            org_did: org_did.clone(),
            org_alias: org_alias.clone(),
            role: role.clone(),
            relay_endpoint,
            inviter_did: Some(claimed.inviter_did.clone()),
            joined_at: Utc::now(),
        };
        let outcome = persist_org_membership(&paths.orgs_root, Some(&paths.contract_stub), request)
            .context("persisting org membership locally")?;
        eprintln!();
        eprintln!("[sec] org membership recorded");
        eprintln!("[sec]   alias:        {}", outcome.alias.as_str());
        eprintln!("[sec]   role:         {}", outcome.membership.role);
        eprintln!(
            "[sec]   relay:        {}",
            outcome.membership.relay_endpoint.as_str()
        );
        if let Some(scope) = &claimed.scope_intent {
            eprintln!("[sec]   scope_intent: {}", scope.to_wire_string());
        }
        if outcome.org_created {
            eprintln!("[sec]   org skeleton created at orgs/{}/", outcome.alias.as_str());
        }
        if !outcome.membership_created {
            eprintln!("[sec]   membership already up to date");
        }
        // Persist scope_intent on the org-level contract for the daemon
        // ingest to interpret later (private channels, subtree filters,
        // …). Currently informational — public-only ingest doesn't
        // gate on it.
        let _ = stash_scope_intent(&paths, &outcome.alias, claimed.scope_intent.as_ref());

        // Slice A' eager bootstrap: kick a one-shot sync_now tick so
        // the `<alias>:_meta` queue history is pulled NOW, not on the
        // next 15-min poll cycle. Each historical channelDef envelope
        // streams through the ingest hook → local channel.md materialises
        // → sidebar populates "on first connect" per the pitch.
        eprintln!();
        eprintln!("[sec] eager bootstrap: pulling `_meta` queue history…");
        match eager_bootstrap_sync(&paths, &did, &key) {
            Ok(SyncSummary { channels, warnings }) => {
                eprintln!(
                    "[sec]   bootstrap done — {channels} channel(s) materialised, {warnings} warning(s)"
                );
            }
            Err(e) => {
                eprintln!(
                    "[sec]   bootstrap failed: {e} \
                     (the channels will appear on the next regular poll cycle)"
                );
            }
        }
    }

    Ok(())
}

/// Best-effort: write the claimed `scope_intent` next to the org
/// membership so future ingest passes can consult it. Failure here is
/// non-fatal — the slice's public-channel scope works without it; the
/// hook is for follow-up slices implementing subtree filters and
/// `private` visibility.
fn stash_scope_intent(
    paths: &secretariat_core::infrastructure::keys::KeyPaths,
    alias: &OrgAlias,
    scope_intent: Option<&ScopeIntent>,
) -> std::io::Result<()> {
    use std::io::Write;
    let Some(scope) = scope_intent else {
        return Ok(());
    };
    let path = paths
        .orgs_root
        .join(alias.as_str())
        .join("scope_intent.local");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::File::create(&path)?;
    writeln!(f, "{}", scope.to_wire_string())?;
    Ok(())
}

/// Bootstrap sync summary surfaced to the principal after claim.
struct SyncSummary {
    channels: usize,
    warnings: usize,
}

/// Run one `sync_now` cycle in a temporary current-thread tokio runtime
/// so the CLI's sync `run_claim` can drive the inbound poll right after
/// persisting org membership. Idempotent with the daemon's poll loop —
/// the global `tick_lock` (held inside `sync_now`) serialises against
/// any running daemon's concurrent tick.
fn eager_bootstrap_sync(
    paths: &secretariat_core::infrastructure::keys::KeyPaths,
    did: &secretariat_core::Did,
    key: &ed25519_dalek::SigningKey,
) -> Result<SyncSummary> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime for eager bootstrap")?;
    let outcome = runtime
        .block_on(secretariat_core::application::sync_now(paths, did, key))
        .context("sync_now during eager bootstrap")?;
    // Count derived channels: walk the channels tree for every org that
    // now has a membership. The pre-claim baseline is "0 channels" for
    // a freshly-claimed org, so listing post-tick gives us the count.
    let mut channels = 0usize;
    if let Ok(orgs) = secretariat_core::application::list_orgs(&paths.orgs_root) {
        for org in orgs {
            let root = secretariat_core::infrastructure::org_store::org_channels_root(
                &paths.orgs_root,
                &org.alias,
            );
            if let Ok(list) = secretariat_core::application::list_channels(&root) {
                channels += list.len();
            }
        }
    }
    let warnings: usize = outcome.per_relay.iter().map(|r| r.warnings.len()).sum();
    Ok(SyncSummary { channels, warnings })
}

/// Parse a `--channels` argument into `(ScopeIntent, channel_handles)`.
/// - `*`            → `ScopeIntent::Org`, empty list
/// - bare handle    → `ScopeIntent::Subtree(handle)`, empty list
/// - `h1,h2,h3`     → `ScopeIntent::Channels`, list
fn parse_channels_spec(spec: &str) -> Result<(ScopeIntent, Vec<String>)> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("--channels cannot be empty"));
    }
    if trimmed == "*" {
        return Ok((ScopeIntent::Org, Vec::new()));
    }
    if trimmed.contains(',') {
        let mut handles = Vec::new();
        for raw in trimmed.split(',') {
            let h = raw.trim();
            if h.is_empty() {
                continue;
            }
            let _ = QueueHandle::parse(h)
                .map_err(|e| anyhow!("invalid channel handle `{h}`: {e}"))?;
            handles.push(h.to_string());
        }
        if handles.is_empty() {
            return Err(anyhow!("--channels comma-separated list parsed empty"));
        }
        return Ok((ScopeIntent::Channels, handles));
    }
    let handle = QueueHandle::parse(trimmed)
        .map_err(|e| anyhow!("invalid subtree handle `{trimmed}`: {e}"))?;
    Ok((ScopeIntent::Subtree(handle), Vec::new()))
}

fn first_registered_relay(path: &std::path::Path) -> Result<String> {
    let state = RelayState::load(path).context("loading relay-state.json")?;
    let endpoint = state
        .iter()
        .find(|r| r.registered)
        .map(|r| r.endpoint.clone());
    endpoint.ok_or_else(|| {
        anyhow!(
            "no registered relay yet. Run `sec daemon register --endpoint <url>` first, \
             or pass --endpoint here."
        )
    })
}

fn relay_origin_from_claim_url(claim_url: &str) -> Result<String> {
    let idx = claim_url
        .find("/v0/invite/")
        .ok_or_else(|| anyhow!("claim URL does not contain `/v0/invite/`"))?;
    Ok(claim_url[..idx].to_string())
}
