//! Use cases for agent (scribe + future roles) CRUD.
//!
//! Substrate-for-themia slice — explicit `sec agent add <name> --role <role>
//! --substrate <substrate>` ceremony grants an agent (today: only `scribe`)
//! signing authority on the principal's behalf. The agent's DID + role +
//! substrate + nickname are recorded in the principal's `identity.md` under
//! `authorized_agents`; the agent's signing key lives at
//! `<self_root>/identity/agents/<name>/key` (filesystem-based, mode `0600` —
//! mirror of the principal-key pattern; platform Keychain Services migration
//! is a separate later slice).
//!
//! Verbs shipped this slice:
//! - [`add_agent`] — mint a fresh DID+keypair, append to `authorized_agents`,
//!   re-sign + persist identity.
//! - [`list_agents`] — enumerate the principal's authorized agents.
//! - [`remove_agent`] — filter out one entry by name; archive key file (move
//!   to `<agents_root>/<name>/key.<removed-ts>`) for audit trail.
//! - [`rotate_agent`] — mint a fresh keypair for an existing agent entry,
//!   archive prior key, update the DID in `authorized_agents`.
//!
//! All four return the updated [`Agent`] (or list); CLI / MCP wrap for
//! presentation.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;
use thiserror::Error;

use crate::domain::{Agent, AgentName, AgentRole, AgentSubstrate, Did};
use crate::infrastructure::identity_store::{
    load_identity, save_identity, IdentityStoreError,
};
#[cfg(test)]
use crate::infrastructure::identity_store::PrincipalIdentity;
use crate::infrastructure::keys::{
    generate_keypair, load_signing_key, save_signing_key, KeyError, KeyPaths,
};

#[derive(Debug, Error)]
pub enum AgentOpsError {
    #[error("identity store: {0}")]
    IdentityStore(#[from] IdentityStoreError),
    #[error("key: {0}")]
    Key(#[from] KeyError),
    #[error("no identity yet — run `sec init` first")]
    NoIdentity,
    #[error("agent named `{0}` already exists; pick a different name or use `sec agent rotate`")]
    NameTaken(String),
    #[error("no agent named `{0}` — check `sec agent list`")]
    NotFound(String),
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Add a new agent. Generates a fresh ed25519 keypair, derives a `did:key`
/// DID, stores the key at `<agents_root>/<name>/key` (mode 0600), appends
/// the entry to `authorized_agents`, re-signs the principal's identity
/// record with the principal's active key, persists.
///
/// Errors if no principal identity exists, if an agent with the same name
/// is already present, or on filesystem IO.
pub fn add_agent(
    paths: &KeyPaths,
    name: AgentName,
    role: AgentRole,
    substrate: AgentSubstrate,
    added_at: DateTime<Utc>,
) -> Result<Agent, AgentOpsError> {
    let mut identity = load_identity(&paths.identity_md)?.ok_or(AgentOpsError::NoIdentity)?;

    if identity
        .authorized_agents
        .iter()
        .any(|a| a.name == name)
    {
        return Err(AgentOpsError::NameTaken(name.to_string()));
    }

    let key = generate_keypair();
    let pubkey = key.verifying_key().to_bytes();
    let agent_did = Did::from_ed25519_public_key(&pubkey);

    let key_path = paths.agent_signing_key_path(name.as_str());
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent).map_err(|e| AgentOpsError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    save_signing_key(&key_path, &key)?;

    let agent = Agent::new(agent_did, role, name, substrate, added_at);
    identity.authorized_agents.push(agent.clone());

    let signing_key = load_signing_key(&paths.signing_key)?;
    save_identity(&paths.identity_md, &identity, Some(&signing_key))?;

    Ok(agent)
}

/// List the principal's authorized agents. Returns empty `Vec` for a
/// fresh install with no agents granted.
pub fn list_agents(paths: &KeyPaths) -> Result<Vec<Agent>, AgentOpsError> {
    let identity = load_identity(&paths.identity_md)?.ok_or(AgentOpsError::NoIdentity)?;
    Ok(identity.authorized_agents)
}

/// Remove an agent by name. Archives the agent's key (move to
/// `<agents_root>/<name>/key.<ts>` for audit) rather than deleting outright.
/// Removes the entry from `authorized_agents`, re-signs + persists identity.
pub fn remove_agent(
    paths: &KeyPaths,
    name: &AgentName,
    removed_at: DateTime<Utc>,
) -> Result<Agent, AgentOpsError> {
    let mut identity = load_identity(&paths.identity_md)?.ok_or(AgentOpsError::NoIdentity)?;

    let idx = identity
        .authorized_agents
        .iter()
        .position(|a| &a.name == name)
        .ok_or_else(|| AgentOpsError::NotFound(name.to_string()))?;

    let removed = identity.authorized_agents.remove(idx);

    let key_path = paths.agent_signing_key_path(name.as_str());
    if key_path.exists() {
        let archived = archive_path(&key_path, removed_at, "removed");
        fs::rename(&key_path, &archived).map_err(|e| AgentOpsError::Io {
            path: key_path.clone(),
            source: e,
        })?;
    }

    let signing_key = load_signing_key(&paths.signing_key)?;
    save_identity(&paths.identity_md, &identity, Some(&signing_key))?;

    Ok(removed)
}

/// Rotate an agent's key. Generates a fresh keypair, archives the prior
/// key (move to `<agents_root>/<name>/key.<ts>`), updates the agent's DID
/// in `authorized_agents`, re-signs + persists identity. Preserves the
/// agent's name + role + substrate.
pub fn rotate_agent(
    paths: &KeyPaths,
    name: &AgentName,
    rotated_at: DateTime<Utc>,
) -> Result<Agent, AgentOpsError> {
    let mut identity = load_identity(&paths.identity_md)?.ok_or(AgentOpsError::NoIdentity)?;

    let idx = identity
        .authorized_agents
        .iter()
        .position(|a| &a.name == name)
        .ok_or_else(|| AgentOpsError::NotFound(name.to_string()))?;

    let key_path = paths.agent_signing_key_path(name.as_str());
    if key_path.exists() {
        let archived = archive_path(&key_path, rotated_at, "rotated");
        fs::rename(&key_path, &archived).map_err(|e| AgentOpsError::Io {
            path: key_path.clone(),
            source: e,
        })?;
    }

    let new_key = generate_keypair();
    let pubkey = new_key.verifying_key().to_bytes();
    let new_did = Did::from_ed25519_public_key(&pubkey);
    save_signing_key(&key_path, &new_key)?;

    identity.authorized_agents[idx].did = new_did;

    let signing_key = load_signing_key(&paths.signing_key)?;
    save_identity(&paths.identity_md, &identity, Some(&signing_key))?;

    Ok(identity.authorized_agents[idx].clone())
}

fn archive_path(active: &Path, when: DateTime<Utc>, suffix: &str) -> PathBuf {
    let ts = when.format("%Y%m%dT%H%M%SZ").to_string();
    active.with_file_name(format!(
        "{}.{ts}.{suffix}",
        active.file_name().and_then(|s| s.to_str()).unwrap_or("key")
    ))
}

/// Borrow-style overload: convenience builder that loads + threads the
/// principal's signing key directly (mostly useful from tests that want a
/// terser invocation when they already have a key in hand).
#[doc(hidden)]
pub fn add_agent_with_key(
    paths: &KeyPaths,
    name: AgentName,
    role: AgentRole,
    substrate: AgentSubstrate,
    added_at: DateTime<Utc>,
    principal_key: &SigningKey,
) -> Result<Agent, AgentOpsError> {
    let mut identity = load_identity(&paths.identity_md)?.ok_or(AgentOpsError::NoIdentity)?;
    if identity.authorized_agents.iter().any(|a| a.name == name) {
        return Err(AgentOpsError::NameTaken(name.to_string()));
    }
    let key = generate_keypair();
    let pubkey = key.verifying_key().to_bytes();
    let agent_did = Did::from_ed25519_public_key(&pubkey);

    let key_path = paths.agent_signing_key_path(name.as_str());
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent).map_err(|e| AgentOpsError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    save_signing_key(&key_path, &key)?;

    let agent = Agent::new(agent_did, role, name, substrate, added_at);
    identity.authorized_agents.push(agent.clone());
    save_identity(&paths.identity_md, &identity, Some(principal_key))?;
    Ok(agent)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DisplayName;
    use tempfile::TempDir;

    fn fresh_principal(tmp: &TempDir) -> (KeyPaths, SigningKey) {
        let root = tmp.path().to_path_buf();
        let paths = KeyPaths::under(root);
        paths.ensure_dirs().unwrap();
        let key = generate_keypair();
        let pk = key.verifying_key().to_bytes();
        let did = Did::from_ed25519_public_key(&pk);
        save_signing_key(&paths.signing_key, &key).unwrap();
        let when = Utc::now();
        let id = PrincipalIdentity {
            did,
            did_method: "did:key".to_string(),
            display_name: DisplayName::parse("Rafa").unwrap(),
            full_name: None,
            key_path: "identity/key".to_string(),
            key_type: "ed25519".to_string(),
            key_created_at: when,
            key_rotations: vec![],
            authorized_agents: vec![],
            created_at: when,
            signature: None,
            body: String::new(),
        };
        save_identity(&paths.identity_md, &id, Some(&key)).unwrap();
        (paths, key)
    }

    #[test]
    fn add_then_list_returns_agent() {
        let tmp = TempDir::new().unwrap();
        let (paths, _key) = fresh_principal(&tmp);
        let name = AgentName::parse("claude").unwrap();
        let agent = add_agent(
            &paths,
            name.clone(),
            AgentRole::Scribe,
            AgentSubstrate::parse("claude-code").unwrap(),
            Utc::now(),
        )
        .unwrap();
        assert_eq!(agent.name, name);

        let listed = list_agents(&paths).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, name);
        assert_eq!(listed[0].substrate.as_str(), "claude-code");

        // Agent key file exists at expected path
        assert!(paths.agent_signing_key_path(name.as_str()).exists());
    }

    #[test]
    fn add_twice_same_name_errors() {
        let tmp = TempDir::new().unwrap();
        let (paths, _key) = fresh_principal(&tmp);
        let name = AgentName::parse("claude").unwrap();
        let _ = add_agent(
            &paths,
            name.clone(),
            AgentRole::Scribe,
            AgentSubstrate::parse("claude-code").unwrap(),
            Utc::now(),
        )
        .unwrap();
        let err = add_agent(
            &paths,
            name.clone(),
            AgentRole::Scribe,
            AgentSubstrate::parse("claude-code").unwrap(),
            Utc::now(),
        );
        assert!(matches!(err, Err(AgentOpsError::NameTaken(_))));
    }

    #[test]
    fn remove_archives_key_and_drops_entry() {
        let tmp = TempDir::new().unwrap();
        let (paths, _key) = fresh_principal(&tmp);
        let name = AgentName::parse("claude").unwrap();
        add_agent(
            &paths,
            name.clone(),
            AgentRole::Scribe,
            AgentSubstrate::parse("claude-code").unwrap(),
            Utc::now(),
        )
        .unwrap();
        let when = Utc::now();
        let removed = remove_agent(&paths, &name, when).unwrap();
        assert_eq!(removed.name, name);
        assert_eq!(list_agents(&paths).unwrap().len(), 0);
        // Original key file is gone, but an archived sibling exists.
        let key_path = paths.agent_signing_key_path(name.as_str());
        assert!(!key_path.exists());
        let parent = key_path.parent().unwrap();
        let archived_count = fs::read_dir(parent)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".removed"))
            .count();
        assert_eq!(archived_count, 1);
    }

    #[test]
    fn rotate_replaces_did_keeps_name() {
        let tmp = TempDir::new().unwrap();
        let (paths, _key) = fresh_principal(&tmp);
        let name = AgentName::parse("claude").unwrap();
        let original = add_agent(
            &paths,
            name.clone(),
            AgentRole::Scribe,
            AgentSubstrate::parse("claude-code").unwrap(),
            Utc::now(),
        )
        .unwrap();
        let rotated = rotate_agent(&paths, &name, Utc::now()).unwrap();
        assert_eq!(rotated.name, original.name);
        assert_eq!(rotated.role, original.role);
        assert_eq!(rotated.substrate, original.substrate);
        assert_ne!(rotated.did, original.did, "DID must change on rotate");
    }

    #[test]
    fn remove_unknown_errors() {
        let tmp = TempDir::new().unwrap();
        let (paths, _key) = fresh_principal(&tmp);
        let ghost = AgentName::parse("ghost").unwrap();
        let r = remove_agent(&paths, &ghost, Utc::now());
        assert!(matches!(r, Err(AgentOpsError::NotFound(_))));
    }
}
