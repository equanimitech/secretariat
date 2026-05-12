//! `sec orgs` — CRUD over the local organization primitive.
//!
//! Orgs live under `~/.secretariat/orgs/<alias>/` with a `.org` metadata
//! file at the root and `channels/` underneath. See
//! `docs/decisions/2026-05-12-substrate-layout-v03.md`.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};

use secretariat_core::application::{
    create_org, delete_org, get_org_contract, list_orgs, set_org_contract, show_org,
};
use secretariat_core::domain::{Did, OrgAlias};

use super::channels::build_patch;

use super::paths::key_paths;

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

    /// Read or edit this principal's private consumption contract for
    /// an org (`<org-dir>/contract.local.md`). Org-root overrides
    /// accumulate down the channel tree per
    /// [[project-consumption-vs-governance]].
    #[command(subcommand)]
    Contract(ContractCmd),
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
        Cmd::Create(c) => run_create(&paths.orgs_root, c),
        Cmd::List => run_list(&paths.orgs_root),
        Cmd::Show(s) => run_show(&paths.orgs_root, s),
        Cmd::Delete(d) => run_delete(&paths.orgs_root, d),
        Cmd::Contract(ContractCmd::Get(g)) => run_contract_get(&paths.orgs_root, g),
        Cmd::Contract(ContractCmd::Set(s)) => run_contract_set(&paths.orgs_root, s),
    }
}

fn run_contract_get(orgs_root: &std::path::Path, args: ContractGetArgs) -> Result<()> {
    let alias = OrgAlias::parse(&args.alias)
        .map_err(|e| anyhow!("invalid alias `{}`: {e}", args.alias))?;
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
    let alias = OrgAlias::parse(&args.alias)
        .map_err(|e| anyhow!("invalid alias `{}`: {e}", args.alias))?;
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

fn run_create(orgs_root: &std::path::Path, args: CreateArgs) -> Result<()> {
    let alias = OrgAlias::parse(&args.alias)
        .map_err(|e| anyhow!("invalid alias `{}`: {e}", args.alias))?;
    let did = match args.did.as_deref() {
        None => None,
        Some(s) => Some(Did::parse(s).map_err(|e| anyhow!("invalid did `{s}`: {e}"))?),
    };
    let name = args.name.unwrap_or_else(|| alias.as_str().to_string());
    let org = create_org(orgs_root, alias, did, name, args.description, Utc::now())
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
        println!("{alias}  {did}  {name}", alias = o.alias, did = did, name = o.name);
    }
    Ok(())
}

fn run_show(orgs_root: &std::path::Path, args: ShowArgs) -> Result<()> {
    let alias = OrgAlias::parse(&args.alias)
        .map_err(|e| anyhow!("invalid alias `{}`: {e}", args.alias))?;
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
    let alias = OrgAlias::parse(&args.alias)
        .map_err(|e| anyhow!("invalid alias `{}`: {e}", args.alias))?;
    if !args.yes {
        return Err(anyhow!(
            "refusing to delete `{}` without --yes (destructive)",
            alias.as_str()
        ));
    }
    if show_org(orgs_root, &alias).context("checking org")?.is_none() {
        return Err(anyhow!("org `{}` not found", alias.as_str()));
    }
    delete_org(orgs_root, &alias).context("deleting org")?;
    println!("deleted org: {}", alias);
    Ok(())
}
