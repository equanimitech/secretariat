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
use clap::{Parser, Subcommand};

use secretariat_core::application::{
    claim_invite, create_invite, view_invite, DEFAULT_INVITE_TTL_HOURS,
};
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
        } => run_create(purpose.as_deref(), ttl_hours, endpoint.as_deref()),
        Cmd::Claim { url, name } => run_claim(&url, name.as_deref()),
    }
}

fn run_create(
    purpose: Option<&str>,
    ttl_hours: Option<i64>,
    endpoint_override: Option<&str>,
) -> Result<()> {
    let paths = key_paths()?;
    let did = load_did(&paths)?;
    let key = load_signing_key(&paths.signing_key)
        .with_context(|| format!("loading signing key from {}", paths.signing_key.display()))?;

    let endpoint = match endpoint_override {
        Some(s) => s.to_string(),
        None => first_registered_relay(&paths.relay_state)?,
    };

    let invite = create_invite(&endpoint, &did, &key, purpose, ttl_hours, None)
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

    Ok(())
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
