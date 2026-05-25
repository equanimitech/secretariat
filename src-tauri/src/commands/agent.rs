//! Tauri commands for the agent (scribe + future roles) onboarding surface.
//!
//! Substrate-for-themia slice — exposes `sec agent` operations to the
//! Tauri frontend so the cognition-provider selection screen on first
//! launch can provision a scribe without subprocess'ing the CLI.

use chrono::Utc;
use secretariat_core::application::{add_agent, list_agents, AgentOpsError};
use secretariat_core::domain::{AgentName, AgentRole, AgentSubstrate};
use secretariat_core::infrastructure::keys::KeyPaths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct AgentDto {
    pub did: String,
    pub role: String,
    pub name: String,
    pub substrate: String,
    pub added_at: String,
}

impl From<secretariat_core::domain::Agent> for AgentDto {
    fn from(a: secretariat_core::domain::Agent) -> Self {
        Self {
            did: a.did.as_str().to_string(),
            role: a.role.as_str().to_string(),
            name: a.name.as_str().to_string(),
            substrate: a.substrate.as_str().to_string(),
            added_at: a.added_at.to_rfc3339(),
        }
    }
}

/// Provision a scribe (cognition-provider selection on Tauri onboarding).
///
/// Mirrors `sec agent add <name> --role scribe --substrate <substrate>`.
/// For `claude-code` substrate, the MCP wiring is already done at app
/// launch (per the bundled `sec mcp install` on boot — see AGENTS.md
/// "Tauri shell"); this command just provisions the agent identity.
#[tauri::command]
#[specta::specta]
pub async fn provision_scribe(name: String, substrate: String) -> Result<AgentDto, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    paths
        .ensure_dirs()
        .map_err(|e| format!("creating directories: {e}"))?;

    let agent_name =
        AgentName::parse(name).map_err(|e| format!("invalid agent name: {e}"))?;
    let substrate_vo =
        AgentSubstrate::parse(substrate).map_err(|e| format!("invalid substrate: {e}"))?;

    match add_agent(&paths, agent_name, AgentRole::Scribe, substrate_vo, Utc::now()) {
        Ok(agent) => Ok(agent.into()),
        Err(AgentOpsError::NameTaken(n)) => {
            Err(format!("a scribe named `{n}` is already configured"))
        }
        Err(AgentOpsError::NoIdentity) => {
            Err("no principal identity — initialize first".to_string())
        }
        Err(e) => Err(format!("provisioning scribe failed: {e}")),
    }
}

/// List the principal's authorized scribes (and future agent roles).
/// Used by the onboarding screen to detect whether a scribe has already
/// been provisioned (returning to onboarding after a partial run should
/// show the existing scribe rather than offer to add another).
#[tauri::command]
#[specta::specta]
pub async fn list_scribes() -> Result<Vec<AgentDto>, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    match list_agents(&paths) {
        Ok(agents) => Ok(agents.into_iter().map(Into::into).collect()),
        Err(AgentOpsError::NoIdentity) => Ok(Vec::new()),
        Err(e) => Err(format!("list_scribes failed: {e}")),
    }
}
