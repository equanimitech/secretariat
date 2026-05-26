//! `sec orgs` — CRUD over the local organization primitive.
//!
//! Orgs live under `~/.secretariat/orgs/<alias>/` with a `.org` metadata
//! file at the root and `channels/` underneath. See
//! `docs/decisions/2026-05-12-substrate-layout-v03.md`.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};

use secretariat_core::application::{
    create_org, delete_org, emit_channel_def_envelope, get_org_contract, list_channels, list_orgs,
    set_org_contract, show_org,
};
use secretariat_core::domain::{Did, OrgAlias, QueueHandle, SignerRole};
use secretariat_core::infrastructure::keys::load_signing_key;
use secretariat_core::infrastructure::org_store::org_channels_root;

use super::channels::build_patch;

use super::paths::{key_paths, load_did};

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Create a new org.
    Create(CreateArgs),

    /// List every local org.
    List,

    /// Show metadata for a single org.
    Show(ShowArgs),

    /// Hard-delete an org's directory tree (destructive — requires --yes).
    Delete(DeleteArgs),

    /// One-shot: emit a `channelDef` envelope to `<alias>:_meta` for every
    /// channel currently present in the org's local tree, parents first.
    /// Idempotent on the receiver — ingest deduplicates. Slice A' backfill
    /// primitive; replaces the removed `sec migrate` step.
    BackfillChannelDefs(BackfillArgs),

    /// Read or edit this principal's private consumption contract for
    /// an org (`<org-dir>/contract.local.md`). Org-root overrides
    /// accumulate down the channel tree per
    /// [[project-consumption-vs-governance]].
    #[command(subcommand)]
    Contract(ContractCmd),
}

#[derive(Parser, Debug)]
pub struct BackfillArgs {
    /// Org alias whose channel tree should be backfilled.
    alias: String,
}

#[derive(Subcommand, Debug)]
enum ContractCmd {
    Get(ContractGetArgs),
    Set(ContractSetArgs),
}

#[derive(Parser, Debug)]
pub struct ContractGetArgs {
    alias: String,
}

#[derive(Parser, Debug)]
pub struct ContractSetArgs {
    alias: String,
    #[arg(long, conflicts_with = "clear_cadence")]
    cadence_floor_minutes: Option<u32>,
    #[arg(long)]
    clear_cadence: bool,
    #[arg(long, value_parser = ["signed-only", "stamp-required"], conflicts_with = "clear_min_trust")]
    min_trust: Option<String>,
    #[arg(long)]
    clear_min_trust: bool,
}

#[derive(Parser, Debug)]
pub struct CreateArgs {
    /// Friendly alias (e.g. `themia.pro`, `equanimi.tech`).
    alias: String,
    /// Optional DID (e.g. `did:web:themia.pro`).
    #[arg(long)]
    did: Option<String>,
    /// Human-readable name. Defaults to the alias.
    #[arg(long)]
    name: Option<String>,
    /// Free-form description.
    #[arg(long, default_value_t = String::new())]
    description: String,
}

#[derive(Parser, Debug)]
pub struct ShowArgs {
    alias: String,
}

#[derive(Parser, Debug)]
pub struct DeleteArgs {
    alias: String,
    /// Required to actually delete (defense against typos).
    #[arg(long, default_value_t = false)]
    yes: bool,
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;
    match args.cmd {
        Cmd::Create(c) => run_create(&paths.orgs_root, c, Some(&paths.contract_stub)),
        Cmd::List => run_list(&paths.orgs_root),
        Cmd::Show(s) => run_show(&paths.orgs_root, s),
        Cmd::Delete(d) => run_delete(&paths.orgs_root, d),
        Cmd::BackfillChannelDefs(b) => run_backfill_channeldefs(&paths, b),
        Cmd::Contract(ContractCmd::Get(g)) => run_contract_get(&paths.orgs_root, g),
        Cmd::Contract(ContractCmd::Set(s)) => run_contract_set(&paths.orgs_root, s),
    }
}

fn run_backfill_channeldefs(
    paths: &secretariat_core::infrastructure::keys::KeyPaths,
    args: BackfillArgs,
) -> Result<()> {
    let alias = OrgAlias::parse(&args.alias)
        .map_err(|e| anyhow!("invalid org alias `{}`: {e}", args.alias))?;
    let org = show_org(&paths.orgs_root, &alias)
        .with_context(|| format!("looking up org `{}`", alias.as_str()))?
        .ok_or_else(|| anyhow!("org `{}` not found", alias.as_str()))?;
    let org_did = org.did.clone().ok_or_else(|| {
        anyhow!(
            "org `{}` has no DID — backfill needs a DID to address the `_meta` queue",
            alias.as_str()
        )
    })?;
    let owner_did = load_did(paths)?;
    let key = load_signing_key(&paths.signing_key)
        .with_context(|| format!("loading signing key from {}", paths.signing_key.display()))?;

    let channels_root = org_channels_root(&paths.orgs_root, &alias);
    let mut summaries = list_channels(&channels_root)
        .with_context(|| format!("walking channels of org `{}`", alias.as_str()))?;
    // Topological order: parents (fewer colons) first so subscribers
    // can reason about parent-before-child if they ever care. Stable
    // sort by handle within the same depth keeps the output predictable.
    summaries.sort_by(|a, b| {
        let da = a.handle.matches(':').count();
        let db = b.handle.matches(':').count();
        da.cmp(&db).then_with(|| a.handle.cmp(&b.handle))
    });

    if summaries.is_empty() {
        println!(
            "(org `{}` has no channels yet — nothing to backfill)",
            alias.as_str()
        );
        return Ok(());
    }

    let now = Utc::now();
    let mut emitted = 0usize;
    for s in &summaries {
        let handle = QueueHandle::parse(&s.handle)
            .with_context(|| format!("malformed handle `{}`", s.handle))?;
        let envelope_path = emit_channel_def_envelope(
            &paths.orgs_root,
            &alias,
            &org_did,
            &owner_did,
            SignerRole::Principal,
            &key,
            &handle,
            &s.name,
            &s.description,
            false,
            now,
        )
        .with_context(|| format!("emitting channelDef envelope for `{}`", s.handle))?;
        emitted += 1;
        println!("emitted {} → {}", s.handle, envelope_path.display());
    }
    println!(
        "backfill complete: {emitted} channelDef envelope(s) queued on `{}:_meta`",
        alias.as_str()
    );
    Ok(())
}

fn run_contract_get(orgs_root: &std::path::Path, args: ContractGetArgs) -> Result<()> {
    let alias =
        OrgAlias::parse(&args.alias).map_err(|e| anyhow!("invalid alias `{}`: {e}", args.alias))?;
    let view = get_org_contract(orgs_root, &alias)
        .with_context(|| format!("reading contract for org `{}`", alias.as_str()))?;
    println!("org: {}", alias.as_str());
    println!("path: {}", view.path.display());
    println!(
        "cadence_floor_minutes: {}",
        view.contract
            .cadence_floor_minutes
            .map(|n| n.to_string())
            .unwrap_or_else(|| "(inherit)".to_string())
    );
    println!(
        "min_trust: {}",
        view.contract
            .min_trust
            .map(|g| g.as_str().to_string())
            .unwrap_or_else(|| "(inherit)".to_string())
    );
    Ok(())
}

fn run_contract_set(orgs_root: &std::path::Path, args: ContractSetArgs) -> Result<()> {
    let alias =
        OrgAlias::parse(&args.alias).map_err(|e| anyhow!("invalid alias `{}`: {e}", args.alias))?;
    let patch = build_patch(
        args.cadence_floor_minutes,
        args.clear_cadence,
        args.min_trust.as_deref(),
        args.clear_min_trust,
    )?;
    if patch.is_noop() {
        return Err(anyhow!(
            "no fields to set — pass at least one of --cadence-floor-minutes/--clear-cadence/--min-trust/--clear-min-trust"
        ));
    }
    let view = set_org_contract(orgs_root, &alias, patch)
        .with_context(|| format!("updating contract for org `{}`", alias.as_str()))?;
    println!("updated: {}", view.path.display());
    println!(
        "cadence_floor_minutes: {}",
        view.contract
            .cadence_floor_minutes
            .map(|n| n.to_string())
            .unwrap_or_else(|| "(inherit)".to_string())
    );
    println!(
        "min_trust: {}",
        view.contract
            .min_trust
            .map(|g| g.as_str().to_string())
            .unwrap_or_else(|| "(inherit)".to_string())
    );
    Ok(())
}

fn run_create(
    orgs_root: &std::path::Path,
    args: CreateArgs,
    stub_override: Option<&std::path::Path>,
) -> Result<()> {
    let alias =
        OrgAlias::parse(&args.alias).map_err(|e| anyhow!("invalid alias `{}`: {e}", args.alias))?;
    let did = match args.did.as_deref() {
        None => None,
        Some(s) => Some(Did::parse(s).map_err(|e| anyhow!("invalid did `{s}`: {e}"))?),
    };
    let name = args.name.unwrap_or_else(|| alias.as_str().to_string());
    let org = create_org(
        orgs_root,
        alias,
        did,
        name,
        args.description,
        Utc::now(),
        stub_override,
    )
    .context("creating org")?;
    println!("created org: {}", org.alias);
    if let Some(d) = &org.did {
        println!("  did: {}", d);
    }
    println!("  name: {}", org.name);
    if !org.description.is_empty() {
        println!("  description: {}", org.description);
    }
    println!("  root: {}", orgs_root.join(org.alias.as_str()).display());
    Ok(())
}

fn run_list(orgs_root: &std::path::Path) -> Result<()> {
    let orgs = list_orgs(orgs_root).context("listing orgs")?;
    if orgs.is_empty() {
        println!("(no orgs yet — create one with `sec orgs create <alias>`)");
        return Ok(());
    }
    for o in &orgs {
        let did = o
            .did
            .as_ref()
            .map(|d| d.as_str().to_string())
            .unwrap_or_else(|| "(no did)".to_string());
        println!(
            "{alias}  {did}  {name}",
            alias = o.alias,
            did = did,
            name = o.name
        );
    }
    Ok(())
}

fn run_show(orgs_root: &std::path::Path, args: ShowArgs) -> Result<()> {
    let alias =
        OrgAlias::parse(&args.alias).map_err(|e| anyhow!("invalid alias `{}`: {e}", args.alias))?;
    match show_org(orgs_root, &alias).context("loading org")? {
        Some(o) => {
            println!("alias: {}", o.alias);
            if let Some(d) = &o.did {
                println!("did: {}", d);
            }
            println!("name: {}", o.name);
            if !o.description.is_empty() {
                println!("description: {}", o.description);
            }
            println!("created_at: {}", o.created_at.to_rfc3339());
            Ok(())
        }
        None => Err(anyhow!("org `{}` not found", alias.as_str())),
    }
}

fn run_delete(orgs_root: &std::path::Path, args: DeleteArgs) -> Result<()> {
    let alias =
        OrgAlias::parse(&args.alias).map_err(|e| anyhow!("invalid alias `{}`: {e}", args.alias))?;
    if !args.yes {
        return Err(anyhow!(
            "refusing to delete `{}` without --yes (destructive)",
            alias.as_str()
        ));
    }
    if show_org(orgs_root, &alias)
        .context("checking org")?
        .is_none()
    {
        return Err(anyhow!("org `{}` not found", alias.as_str()));
    }
    delete_org(orgs_root, &alias).context("deleting org")?;
    println!("deleted org: {}", alias);
    Ok(())
}
