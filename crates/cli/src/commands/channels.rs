//! `sec channels` — CRUD over the channel tree.
//!
//! Channels are colon-pathed handles (`channel:secretariat:dev`,
//! `channel:dommage-corporel:paris-cohort`). When `--org <alias>` is
//! passed, the channel lives inside that org's tree at
//! `~/.secretariat/orgs/<alias>/channels/<segs>/`. Without `--org` it
//! lives in the principal's personal tree at `~/.secretariat/channels/`.
//! See `docs/decisions/2026-05-12-substrate-layout-v03.md`.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};

use secretariat_core::application::{
    create_channel, delete_channel, get_channel_contract, list_channels, read_channel,
    set_channel_contract, show_org, ContractPatch, PatchField,
};
use secretariat_core::domain::{OrgAlias, QueueHandle, TrustGate};
use secretariat_core::infrastructure::org_store::org_channels_root;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Create a channel (writes `.channelDef` + pre-creates envelopes/).
    Create(CreateArgs),

    /// List channels in a tree, grouped by top segment.
    List(ListArgs),

    /// Print the most-recent envelopes from a channel.
    Read(ReadArgs),

    /// Hard-delete a channel's directory tree (destructive — requires --yes).
    Delete(DeleteArgs),

    /// Read or edit this principal's private consumption contract for
    /// a channel (`<channel-dir>/contract.local.md`).
    #[command(subcommand)]
    Contract(ContractCmd),
}

#[derive(Subcommand, Debug)]
enum ContractCmd {
    /// Print the consumption contract for a channel.
    Get(ContractGetArgs),
    /// Apply a partial update to a channel's consumption contract.
    /// Fields not mentioned are left untouched; pass `--clear-*` to
    /// revert a field to inheriting from ancestors.
    Set(ContractSetArgs),
}

#[derive(Parser, Debug)]
pub struct ContractGetArgs {
    handle: String,
    #[arg(long)]
    org: Option<String>,
}

#[derive(Parser, Debug)]
pub struct ContractSetArgs {
    handle: String,
    #[arg(long)]
    org: Option<String>,
    /// Set my poll-floor for this channel (minutes).
    #[arg(long, conflicts_with = "clear_cadence")]
    cadence_floor_minutes: Option<u32>,
    /// Clear my poll-floor — fall back to inheriting from ancestors.
    #[arg(long)]
    clear_cadence: bool,
    /// Set my receiver-side trust filter for this channel.
    #[arg(long, value_parser = ["signed-only", "stamp-required"], conflicts_with = "clear_min_trust")]
    min_trust: Option<String>,
    /// Clear my trust filter — fall back to inheriting from ancestors.
    #[arg(long)]
    clear_min_trust: bool,
}

#[derive(Parser, Debug)]
pub struct CreateArgs {
    /// Channel handle (`channel:foo:bar`).
    handle: String,
    /// Org alias the channel lives in. Omit for personal channels.
    #[arg(long)]
    org: Option<String>,
    /// Human-readable name. Defaults to the handle's last segment.
    #[arg(long)]
    name: Option<String>,
    /// Free-form description.
    #[arg(long, default_value_t = String::new())]
    description: String,
}

#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Org alias to scope the listing to. Omit for personal channels.
    #[arg(long)]
    org: Option<String>,
    /// Only show channels whose handle starts with this prefix
    /// (e.g. `channel:product`).
    #[arg(long)]
    prefix: Option<String>,
    /// Flat output instead of the default grouped/tree render.
    #[arg(long, default_value_t = false)]
    flat: bool,
}

#[derive(Parser, Debug)]
pub struct ReadArgs {
    handle: String,
    #[arg(long)]
    org: Option<String>,
    #[arg(short = 'n', long, default_value_t = 10)]
    limit: usize,
}

#[derive(Parser, Debug)]
pub struct DeleteArgs {
    handle: String,
    #[arg(long)]
    org: Option<String>,
    #[arg(long, default_value_t = false)]
    yes: bool,
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;
    match args.cmd {
        Cmd::Create(c) => run_create(&paths, c),
        Cmd::List(l) => run_list(&paths, l),
        Cmd::Read(r) => run_read(&paths, r),
        Cmd::Delete(d) => run_delete(&paths, d),
        Cmd::Contract(ContractCmd::Get(g)) => run_contract_get(&paths, g),
        Cmd::Contract(ContractCmd::Set(s)) => run_contract_set(&paths, s),
    }
}

fn run_contract_get(
    paths: &secretariat_core::infrastructure::KeyPaths,
    args: ContractGetArgs,
) -> Result<()> {
    let handle = QueueHandle::parse(&args.handle)
        .map_err(|e| anyhow!("invalid handle `{}`: {e}", args.handle))?;
    let root = resolve_channels_root(paths, args.org.as_deref())?;
    let view = get_channel_contract(&root, &handle)
        .with_context(|| format!("reading contract for `{}`", handle.as_str()))?;
    println!("handle: {}", handle.as_str());
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

fn run_contract_set(
    paths: &secretariat_core::infrastructure::KeyPaths,
    args: ContractSetArgs,
) -> Result<()> {
    let handle = QueueHandle::parse(&args.handle)
        .map_err(|e| anyhow!("invalid handle `{}`: {e}", args.handle))?;
    let root = resolve_channels_root(paths, args.org.as_deref())?;
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
    let view = set_channel_contract(&root, &handle, patch)
        .with_context(|| format!("updating contract for `{}`", handle.as_str()))?;
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

pub(crate) fn build_patch(
    cadence: Option<u32>,
    clear_cadence: bool,
    min_trust: Option<&str>,
    clear_min_trust: bool,
) -> Result<ContractPatch> {
    let cadence_floor_minutes = match (cadence, clear_cadence) {
        (Some(n), false) => PatchField::Set(n),
        (None, true) => PatchField::Clear,
        (None, false) => PatchField::Leave,
        (Some(_), true) => unreachable!("clap conflicts_with"),
    };
    let min_trust = match (min_trust, clear_min_trust) {
        (Some(s), false) => PatchField::Set(
            TrustGate::parse(s).ok_or_else(|| anyhow!("invalid min_trust `{s}`"))?,
        ),
        (None, true) => PatchField::Clear,
        (None, false) => PatchField::Leave,
        (Some(_), true) => unreachable!("clap conflicts_with"),
    };
    Ok(ContractPatch {
        cadence_floor_minutes,
        min_trust,
    })
}

fn resolve_channels_root(
    paths: &secretariat_core::infrastructure::KeyPaths,
    org: Option<&str>,
) -> Result<PathBuf> {
    match org {
        None => Ok(paths.channels.clone()),
        Some(s) => {
            let alias = OrgAlias::parse(s)
                .map_err(|e| anyhow!("invalid org alias `{s}`: {e}"))?;
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
            Ok(org_channels_root(&paths.orgs_root, &alias))
        }
    }
}

fn run_create(
    paths: &secretariat_core::infrastructure::KeyPaths,
    args: CreateArgs,
) -> Result<()> {
    let handle = QueueHandle::parse(&args.handle)
        .map_err(|e| anyhow!("invalid handle `{}`: {e}", args.handle))?;
    let root = resolve_channels_root(paths, args.org.as_deref())?;
    let name = args
        .name
        .unwrap_or_else(|| handle.slug().to_string());
    let def = create_channel(&root, handle, name, args.description, Utc::now())
        .context("creating channel")?;
    println!("created channel: {}", def.handle);
    if !def.name.is_empty() {
        println!("  name: {}", def.name);
    }
    if !def.description.is_empty() {
        println!("  description: {}", def.description);
    }
    if let Some(org_alias) = &args.org {
        println!("  org: {}", org_alias);
    }
    Ok(())
}

fn run_list(
    paths: &secretariat_core::infrastructure::KeyPaths,
    args: ListArgs,
) -> Result<()> {
    let root = resolve_channels_root(paths, args.org.as_deref())?;
    let mut summaries =
        list_channels(&root).context("walking channels tree")?;
    if let Some(prefix) = args.prefix.as_deref() {
        summaries.retain(|s| s.handle.starts_with(prefix));
    }
    if summaries.is_empty() {
        let scope = args
            .org
            .as_deref()
            .map(|o| format!("`{o}`"))
            .unwrap_or_else(|| "(personal)".to_string());
        println!("(no channels in {scope} — create one with `sec channels create channel:<name>`)");
        return Ok(());
    }
    if args.flat {
        for s in &summaries {
            print_flat_row(s);
        }
    } else {
        print_grouped(&summaries);
    }
    Ok(())
}

fn print_flat_row(s: &secretariat_core::application::ChannelSummary) {
    let latest = s
        .latest_at
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(|| "-".to_string());
    let display = if s.name.is_empty() {
        String::new()
    } else {
        format!("  ({})", s.name)
    };
    println!(
        "{handle}{display}  {count} envelope{plural}  latest {latest}",
        handle = s.handle,
        display = display,
        count = s.envelope_count,
        plural = if s.envelope_count == 1 { "" } else { "s" },
        latest = latest,
    );
}

/// Group channels by their top segment after `channel:` and render as a
/// shallow tree. Channels with a single segment after `channel:` (e.g.
/// `channel:general`) land in an `(unprefixed)` bucket.
fn print_grouped(summaries: &[secretariat_core::application::ChannelSummary]) {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<String, Vec<&secretariat_core::application::ChannelSummary>> =
        BTreeMap::new();
    for s in summaries {
        let segs: Vec<&str> = s.handle.split(':').collect();
        // `channel:<top>:<rest>...`. Top group key:
        let group = if segs.len() <= 2 {
            "(unprefixed)".to_string()
        } else {
            segs[1].to_string()
        };
        groups.entry(group).or_default().push(s);
    }
    println!("{} channels", summaries.len());
    for (group, items) in &groups {
        println!("├── {group} ({})", items.len());
        for s in items {
            let display = if s.name.is_empty() {
                String::new()
            } else {
                format!(" — {}", s.name)
            };
            let envelopes = if s.envelope_count == 0 {
                String::new()
            } else {
                format!("  [{} envelope{}]", s.envelope_count, if s.envelope_count == 1 { "" } else { "s" })
            };
            println!("│     {}{display}{envelopes}", s.handle);
        }
    }
}

fn run_read(
    paths: &secretariat_core::infrastructure::KeyPaths,
    args: ReadArgs,
) -> Result<()> {
    let handle = QueueHandle::parse(&args.handle)
        .map_err(|e| anyhow!("invalid handle `{}`: {e}", args.handle))?;
    let root = resolve_channels_root(paths, args.org.as_deref())?;
    let envelopes = read_channel(&root, &handle, args.limit)
        .with_context(|| format!("reading channel `{}`", handle.as_str()))?;
    if envelopes.is_empty() {
        println!("(channel `{}` exists but has no envelopes)", handle.as_str());
        return Ok(());
    }
    for (i, e) in envelopes.iter().enumerate() {
        if i > 0 {
            println!();
            println!("---");
            println!();
        }
        let captured = e
            .captured_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "?".to_string());
        let from = e.from.as_deref().unwrap_or("(unknown)");
        println!("# {captured}");
        println!("from: {from}");
        if !e.source.is_empty() {
            println!("source: {}", e.source);
        }
        if e.stamped {
            println!("stamped: true");
        }
        if e.encrypted {
            println!("encrypted: true");
        }
        println!();
        println!("{}", e.body.trim_end());
    }
    Ok(())
}

fn run_delete(
    paths: &secretariat_core::infrastructure::KeyPaths,
    args: DeleteArgs,
) -> Result<()> {
    if !args.yes {
        return Err(anyhow!(
            "refusing to delete `{}` without --yes (destructive)",
            args.handle
        ));
    }
    let handle = QueueHandle::parse(&args.handle)
        .map_err(|e| anyhow!("invalid handle `{}`: {e}", args.handle))?;
    let root = resolve_channels_root(paths, args.org.as_deref())?;
    delete_channel(&root, &handle).context("deleting channel")?;
    println!("deleted channel: {}", handle.as_str());
    Ok(())
}

// Allow tests to use the helper.
#[allow(dead_code)]
pub(crate) fn channels_root_for(
    paths: &secretariat_core::infrastructure::KeyPaths,
    org: Option<&str>,
) -> Result<PathBuf> {
    resolve_channels_root(paths, org)
}

#[allow(dead_code)]
pub(crate) fn channels_root_path(paths: &secretariat_core::infrastructure::KeyPaths) -> &Path {
    &paths.channels
}
