//! `sec agent` — manage authorized agents (scribes + future roles).
//!
//! Substrate-for-themia slice. Grants a non-principal DID-keyed identity
//! signing authority on the principal's behalf, recorded in
//! `<root>/identity.md` under `authorized_agents`.
//!
//! Subcommands:
//! - `sec agent add <name> --role scribe --substrate claude-code`
//! - `sec agent list [--role scribe]`
//! - `sec agent remove <name>`
//! - `sec agent rotate <name>` (fresh keypair, same name + role + substrate)

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};

use secretariat_core::application::agent_ops;
use secretariat_core::domain::{AgentName, AgentRole, AgentSubstrate};

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Grant a new agent signing authority. Mints a fresh did:key keypair,
    /// stores the key at <root>/identity/agents/<name>/key (mode 0600),
    /// appends an entry to `authorized_agents`, re-signs identity record.
    Add {
        /// Principal-chosen nickname for this agent. Conventionally matches
        /// the substrate identifier (e.g. `claude` for `--substrate claude-code`).
        name: String,
        /// Agent role. Today only `scribe`.
        #[arg(long, default_value = "scribe")]
        role: String,
        /// Cognition provider the agent runs under. Today only `claude-code`;
        /// future adapters extend additively.
        #[arg(long, default_value = "claude-code")]
        substrate: String,
    },
    /// List authorized agents.
    List {
        /// Filter by role (e.g. `--role scribe`). Omit for all.
        #[arg(long)]
        role: Option<String>,
    },
    /// Remove an agent. The agent's key file is archived (renamed with
    /// timestamp suffix), not deleted, for audit trail.
    Remove {
        /// Agent name to remove.
        name: String,
    },
    /// Rotate an agent's keypair. Preserves name + role + substrate;
    /// generates new keypair, archives prior key, updates DID.
    Rotate {
        /// Agent name to rotate.
        name: String,
    },
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;
    match args.cmd {
        Cmd::Add {
            name,
            role,
            substrate,
        } => add(&paths, name, role, substrate),
        Cmd::List { role } => list(&paths, role),
        Cmd::Remove { name } => remove(&paths, name),
        Cmd::Rotate { name } => rotate(&paths, name),
    }
}

fn add(
    paths: &secretariat_core::infrastructure::keys::KeyPaths,
    name: String,
    role: String,
    substrate: String,
) -> Result<()> {
    let name = AgentName::parse(name).map_err(|e| anyhow!("invalid agent name: {e}"))?;
    let role = AgentRole::parse(&role).map_err(|e| anyhow!("invalid role: {e}"))?;
    let substrate =
        AgentSubstrate::parse(substrate).map_err(|e| anyhow!("invalid substrate: {e}"))?;

    let agent =
        agent_ops::add_agent(paths, name, role, substrate, Utc::now()).context("adding agent")?;
    eprintln!(
        "[sec] agent added: {} ({}, {}) → {}",
        agent.name, agent.role, agent.substrate, agent.did
    );
    eprintln!(
        "[sec]   key path → {}",
        paths.agent_signing_key_path(agent.name.as_str()).display()
    );
    Ok(())
}

fn list(
    paths: &secretariat_core::infrastructure::keys::KeyPaths,
    role_filter: Option<String>,
) -> Result<()> {
    let role_filter = role_filter
        .map(|r| AgentRole::parse(&r).map_err(|e| anyhow!("invalid role filter: {e}")))
        .transpose()?;
    let agents = agent_ops::list_agents(paths).context("listing agents")?;
    let filtered: Vec<_> = agents
        .into_iter()
        .filter(|a| role_filter.map(|r| a.role == r).unwrap_or(true))
        .collect();
    if filtered.is_empty() {
        eprintln!("[sec] no agents yet — `sec agent add <name>` to grant signing authority");
        return Ok(());
    }
    for a in &filtered {
        println!(
            "{name}\t{role}\t{substrate}\t{did}\t{added_at}",
            name = a.name,
            role = a.role,
            substrate = a.substrate,
            did = a.did,
            added_at = a.added_at.to_rfc3339()
        );
    }
    Ok(())
}

fn remove(paths: &secretariat_core::infrastructure::keys::KeyPaths, name: String) -> Result<()> {
    let name = AgentName::parse(name).map_err(|e| anyhow!("invalid agent name: {e}"))?;
    let removed = agent_ops::remove_agent(paths, &name, Utc::now()).context("removing agent")?;
    eprintln!(
        "[sec] agent removed: {} (key archived; identity re-signed)",
        removed.name
    );
    Ok(())
}

fn rotate(paths: &secretariat_core::infrastructure::keys::KeyPaths, name: String) -> Result<()> {
    let name = AgentName::parse(name).map_err(|e| anyhow!("invalid agent name: {e}"))?;
    let rotated = agent_ops::rotate_agent(paths, &name, Utc::now()).context("rotating agent")?;
    eprintln!(
        "[sec] agent rotated: {} → new DID {}",
        rotated.name, rotated.did
    );
    Ok(())
}
