//! Value objects + aggregate for an authorized agent (substrate-for-themia slice).
//!
//! An *agent* is a non-principal DID-keyed identity that signs envelopes on a
//! principal's behalf. The principal grants signing authority via an explicit
//! `sec agent add <name> --role <role> --substrate <substrate>` ceremony; the
//! grant is recorded in the principal's `identity.md` under `authorized_agents`.
//!
//! Today's only role is `scribe`. Future roles (`auditor`, `scheduler`,
//! `reader`) reuse the same record shape with different `role` values.
//!
//! Per AGENTS.md two-layer naming:
//!   - protocol/cryptographic layer → `agent` (this module's vocabulary)
//!   - substrate/UX layer → `scribe` (the only currently-shipped role)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::Did;

// ---------------------------------------------------------------------------
// AgentRole
// ---------------------------------------------------------------------------

/// Role of an agent. Today only `Scribe`; future roles reuse the same
/// record shape additively (`auditor`, `scheduler`, `reader`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Scribe,
}

#[derive(Debug, Error)]
pub enum AgentRoleParseError {
    #[error("unknown agent role `{0}` (known: scribe)")]
    Unknown(String),
}

impl AgentRole {
    pub fn parse(s: &str) -> Result<Self, AgentRoleParseError> {
        match s {
            "scribe" => Ok(Self::Scribe),
            other => Err(AgentRoleParseError::Unknown(other.to_string())),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Scribe => "scribe",
        }
    }
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// AgentSubstrate
// ---------------------------------------------------------------------------

/// Cognition provider the agent runs under (per architectural invariant #5
/// — cognition is pluggable). Today only `claude-code`; future values land
/// additively (`opencode`, `anthropic-api`, `ollama-<model>`, `bedrock`, ...).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AgentSubstrate(String);

#[derive(Debug, Error)]
pub enum AgentSubstrateParseError {
    #[error("substrate must be non-empty")]
    Empty,
    #[error("substrate must match [a-z0-9_.-]+, got `{0}`")]
    InvalidChars(String),
}

impl AgentSubstrate {
    /// Parse a substrate identifier. We accept any well-formed string (not just
    /// the lexicon's `knownValues`) so future adapters land without VO
    /// churn — lexicon `knownValues` are advisory; conformance is by
    /// downstream use.
    pub fn parse(s: impl Into<String>) -> Result<Self, AgentSubstrateParseError> {
        let s = s.into();
        if s.is_empty() {
            return Err(AgentSubstrateParseError::Empty);
        }
        if !s.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.'
        }) {
            return Err(AgentSubstrateParseError::InvalidChars(s));
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<AgentSubstrate> for String {
    fn from(s: AgentSubstrate) -> String {
        s.0
    }
}

impl TryFrom<String> for AgentSubstrate {
    type Error = AgentSubstrateParseError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl std::fmt::Display for AgentSubstrate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// AgentName
// ---------------------------------------------------------------------------

/// Principal-chosen nickname for an agent. Used in CLI references
/// (`sec agent rotate <name>`) and UI surfaces. Conventionally matches the
/// cognition substrate identifier when not customized (e.g. `claude` for
/// `claude-code`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AgentName(String);

#[derive(Debug, Error)]
pub enum AgentNameParseError {
    #[error("agent name must be non-empty")]
    Empty,
    #[error("agent name must not exceed 64 chars")]
    TooLong,
    #[error("agent name must match [a-z0-9_-]+, got `{0}`")]
    InvalidChars(String),
}

impl AgentName {
    pub fn parse(s: impl Into<String>) -> Result<Self, AgentNameParseError> {
        let s = s.into();
        if s.is_empty() {
            return Err(AgentNameParseError::Empty);
        }
        if s.len() > 64 {
            return Err(AgentNameParseError::TooLong);
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(AgentNameParseError::InvalidChars(s));
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<AgentName> for String {
    fn from(n: AgentName) -> String {
        n.0
    }
}

impl TryFrom<String> for AgentName {
    type Error = AgentNameParseError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl std::fmt::Display for AgentName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Agent
// ---------------------------------------------------------------------------

/// One entry in a principal's `authorized_agents` list. Binds a DID-keyed
/// identity to a role + cognition substrate + principal-chosen nickname.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    pub did: Did,
    pub role: AgentRole,
    pub name: AgentName,
    pub substrate: AgentSubstrate,
    pub added_at: DateTime<Utc>,
}

impl Agent {
    pub fn new(
        did: Did,
        role: AgentRole,
        name: AgentName,
        substrate: AgentSubstrate,
        added_at: DateTime<Utc>,
    ) -> Self {
        Self {
            did,
            role,
            name,
            substrate,
            added_at,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_role_parse_roundtrip() {
        let r = AgentRole::parse("scribe").unwrap();
        assert_eq!(r.as_str(), "scribe");
    }

    #[test]
    fn agent_role_rejects_unknown() {
        assert!(matches!(
            AgentRole::parse("auditor"),
            Err(AgentRoleParseError::Unknown(_))
        ));
    }

    #[test]
    fn agent_substrate_accepts_known() {
        assert_eq!(
            AgentSubstrate::parse("claude-code").unwrap().as_str(),
            "claude-code"
        );
    }

    #[test]
    fn agent_substrate_accepts_future_values() {
        // Lexicon knownValues are advisory; the VO accepts any well-formed
        // string so adapters land additively.
        assert!(AgentSubstrate::parse("opencode").is_ok());
        assert!(AgentSubstrate::parse("ollama-llama3.2").is_ok());
    }

    #[test]
    fn agent_substrate_rejects_empty_and_bad_chars() {
        assert!(matches!(
            AgentSubstrate::parse(""),
            Err(AgentSubstrateParseError::Empty)
        ));
        assert!(matches!(
            AgentSubstrate::parse("Has Spaces"),
            Err(AgentSubstrateParseError::InvalidChars(_))
        ));
    }

    #[test]
    fn agent_name_roundtrip() {
        let n = AgentName::parse("claude").unwrap();
        assert_eq!(n.as_str(), "claude");
    }

    #[test]
    fn agent_name_rejects_uppercase_and_spaces() {
        assert!(AgentName::parse("Claude").is_err());
        assert!(AgentName::parse("my claude").is_err());
    }

    #[test]
    fn agent_name_rejects_too_long() {
        assert!(matches!(
            AgentName::parse("a".repeat(65)),
            Err(AgentNameParseError::TooLong)
        ));
    }

    #[test]
    fn agent_construct() {
        let did = Did::from_ed25519_public_key(&[0x11; 32]);
        let role = AgentRole::Scribe;
        let name = AgentName::parse("claude").unwrap();
        let sub = AgentSubstrate::parse("claude-code").unwrap();
        let when = Utc::now();
        let a = Agent::new(did.clone(), role, name.clone(), sub.clone(), when);
        assert_eq!(a.did, did);
        assert_eq!(a.role, role);
        assert_eq!(a.name, name);
        assert_eq!(a.substrate, sub);
        assert_eq!(a.added_at, when);
    }
}
