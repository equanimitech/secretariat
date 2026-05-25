//! `AgentManifest` — on-wire dual of the principal-private `authorized_agents`
//! field, signed by the principal themselves.
//!
//! See `lexicons/tech.equanimi.secretariat.agentManifest.json` for the
//! authoritative record shape. Distinct from `rosterUpdate`:
//!   - rosterUpdate is **admin-signed** ("who is a member of this channel")
//!   - agentManifest is **self-signed** ("which agent keys have I authorized
//!     to act on my behalf")
//!
//! Emitted by the principal on:
//!   - `sec invite accept` — one manifest into every channel they joined
//!   - `sec agent add` / `sec agent rotate` / `sec agent remove` — refresh
//!     into every channel where they are already a member
//!
//! Receivers cache the latest manifest per `(signer, target)` and consult it
//! during the substrate-for-themia verifier chain (P2 hop 3).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

use crate::domain::{Agent, Did, EnvelopeSignature, Signature};

/// Canonical-preimage version tag. Bump if the canonicalization changes.
const CANONICAL_PREIMAGE_TAG: &[u8] = b"agentManifest:v0:";

// ---------------------------------------------------------------------------
// ManifestTarget
// ---------------------------------------------------------------------------

/// Scope the manifest applies to. Three forms:
///   - `Org(did)` → `org:<org-did>` — every channel in the org
///   - `Channel { owner, handle }` → `channel:<owner-did>#<handle>` — one specific channel
///   - `Global` → `*` — global broadcast
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestTarget {
    Org(Did),
    Channel { owner: Did, handle: String },
    Global,
}

#[derive(Debug, Error)]
pub enum ManifestTargetParseError {
    #[error("target must start with `org:`, `channel:`, or be `*`; got `{0}`")]
    UnknownForm(String),
    #[error("target `channel:` form requires `<did>#<handle>`; got `{0}`")]
    MalformedChannel(String),
    #[error("target carries invalid DID: {0}")]
    InvalidDid(#[from] crate::domain::DidParseError),
}

impl ManifestTarget {
    pub fn parse(s: &str) -> Result<Self, ManifestTargetParseError> {
        if s == "*" {
            return Ok(Self::Global);
        }
        if let Some(rest) = s.strip_prefix("org:") {
            return Ok(Self::Org(Did::parse(rest)?));
        }
        if let Some(rest) = s.strip_prefix("channel:") {
            let (did_part, handle) = rest
                .split_once('#')
                .ok_or_else(|| ManifestTargetParseError::MalformedChannel(s.to_string()))?;
            return Ok(Self::Channel {
                owner: Did::parse(did_part)?,
                handle: handle.to_string(),
            });
        }
        Err(ManifestTargetParseError::UnknownForm(s.to_string()))
    }

    pub fn as_string(&self) -> String {
        match self {
            Self::Global => "*".to_string(),
            Self::Org(did) => format!("org:{}", did.as_str()),
            Self::Channel { owner, handle } => format!("channel:{}#{handle}", owner.as_str()),
        }
    }
}

impl std::fmt::Display for ManifestTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.as_string())
    }
}

// ---------------------------------------------------------------------------
// AgentManifest
// ---------------------------------------------------------------------------

/// One emitted manifest. Receivers index by `(signer, target)`, keeping the
/// latest by `declared_at` (then by envelope TID for tie-breaks). Includes
/// the detached signature so the manifest can be verified independent of
/// the envelope transport metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentManifest {
    pub signer: Did,
    pub target: ManifestTarget,
    pub authorized_agents: Vec<Agent>,
    pub declared_at: DateTime<Utc>,
    pub signature: Signature,
}

impl AgentManifest {
    /// Compute the canonical preimage for the manifest's signature. Mirror
    /// of the identity record's preimage shape: version tag + sorted-key
    /// JSON of every field except the signature itself.
    pub fn canonical_preimage(
        signer: &Did,
        target: &ManifestTarget,
        authorized_agents: &[Agent],
        declared_at: &DateTime<Utc>,
    ) -> Vec<u8> {
        use serde_json::Value as JsonValue;

        let mut map: BTreeMap<&str, JsonValue> = BTreeMap::new();
        map.insert("signer", JsonValue::String(signer.as_str().to_string()));
        map.insert("target", JsonValue::String(target.as_string()));
        map.insert(
            "authorized_agents",
            serde_json::to_value(authorized_agents).unwrap_or(JsonValue::Array(vec![])),
        );
        map.insert(
            "declared_at",
            JsonValue::String(declared_at.to_rfc3339()),
        );

        let mut out = CANONICAL_PREIMAGE_TAG.to_vec();
        out.extend(serde_json::to_vec(&map).unwrap_or_default());
        out
    }

    /// Mint + sign a manifest. The signer's `principal_key` is the same
    /// key that signs `identity.md`.
    pub fn sign(
        signer: Did,
        target: ManifestTarget,
        authorized_agents: Vec<Agent>,
        declared_at: DateTime<Utc>,
        principal_key: &ed25519_dalek::SigningKey,
    ) -> Self {
        use ed25519_dalek::Signer as _;
        let preimage =
            Self::canonical_preimage(&signer, &target, &authorized_agents, &declared_at);
        let dalek_sig = principal_key.sign(&preimage);
        Self {
            signer,
            target,
            authorized_agents,
            declared_at,
            signature: Signature::from_bytes(dalek_sig.to_bytes()),
        }
    }

    /// Verify the manifest's signature against the principal's verifying
    /// key. Returns `true` iff the signature is valid over the canonical
    /// preimage derived from the manifest's current field values.
    pub fn verify(&self, principal_pubkey: &ed25519_dalek::VerifyingKey) -> bool {
        use ed25519_dalek::Verifier as _;
        let preimage = Self::canonical_preimage(
            &self.signer,
            &self.target,
            &self.authorized_agents,
            &self.declared_at,
        );
        let dalek_sig = ed25519_dalek::Signature::from_bytes(self.signature.as_bytes());
        principal_pubkey.verify(&preimage, &dalek_sig).is_ok()
    }
}

// ---------------------------------------------------------------------------
// On-wire frontmatter shape (serde mirror)
// ---------------------------------------------------------------------------

/// Frontmatter shape for a manifest envelope. The manifest fields ride
/// at the top level (with `$type: tech.equanimi.secretariat.agentManifest`
/// as discriminator). The `$signature` block — envelope-level author
/// signature mandated by hard rule #4 — sits alongside as a sibling
/// block, keeping the manifest envelope uniform with every other
/// `$signature`-bearing envelope on the wire.
///
/// Two cryptographic layers, both signed by the principal's key:
///   - **Inner** (`signature: ed25519:...`) — over the manifest's
///     canonical preimage. Lets the manifest verify standalone when
///     extracted to a cache, independent of any envelope wrapper.
///   - **Outer** (`$signature: { ... }`) — over the body
///     (always empty for manifests; the doc_hash is therefore
///     canonical_body_hash("")). Enforces the "every envelope carries
///     `$signature`" invariant uniformly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifestFrontmatter {
    #[serde(rename = "$type")]
    pub ty: String,
    pub signer: String,
    pub target: String,
    pub authorized_agents: Vec<Agent>,
    pub declared_at: String,
    pub signature: String,
    /// Envelope-level author signature. Optional in the parser for
    /// pre-Move-1C-revision back-compat; emitters MUST populate it.
    #[serde(
        rename = "$signature",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub envelope_signature: Option<EnvelopeSignature>,
}

pub const AGENT_MANIFEST_TYPE: &str = "tech.equanimi.secretariat.agentManifest";

impl From<&AgentManifest> for AgentManifestFrontmatter {
    fn from(m: &AgentManifest) -> Self {
        Self {
            ty: AGENT_MANIFEST_TYPE.to_string(),
            signer: m.signer.as_str().to_string(),
            target: m.target.as_string(),
            authorized_agents: m.authorized_agents.clone(),
            declared_at: m.declared_at.to_rfc3339(),
            signature: m.signature.to_string(),
            // Outer signature is added by `emit_manifest_into_channel`,
            // not by the domain `From` impl — the domain has no signing
            // key handle.
            envelope_signature: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum AgentManifestParseError {
    #[error("invalid signer DID: {0}")]
    InvalidSigner(crate::domain::DidParseError),
    #[error("invalid target: {0}")]
    InvalidTarget(#[from] ManifestTargetParseError),
    #[error("invalid signature: {0}")]
    InvalidSignature(#[from] crate::domain::SignatureParseError),
    #[error("invalid declared_at `{0}`")]
    InvalidTimestamp(String),
    #[error("wrong $type: expected `{AGENT_MANIFEST_TYPE}`, got `{0}`")]
    WrongType(String),
}

impl TryFrom<AgentManifestFrontmatter> for AgentManifest {
    type Error = AgentManifestParseError;
    fn try_from(fm: AgentManifestFrontmatter) -> Result<Self, Self::Error> {
        if fm.ty != AGENT_MANIFEST_TYPE {
            return Err(AgentManifestParseError::WrongType(fm.ty));
        }
        let signer =
            Did::parse(&fm.signer).map_err(AgentManifestParseError::InvalidSigner)?;
        let target = ManifestTarget::parse(&fm.target)?;
        let signature = Signature::parse(&fm.signature)?;
        let declared_at = DateTime::parse_from_rfc3339(&fm.declared_at)
            .map_err(|_| AgentManifestParseError::InvalidTimestamp(fm.declared_at.clone()))?
            .with_timezone(&Utc);
        Ok(AgentManifest {
            signer,
            target,
            authorized_agents: fm.authorized_agents,
            declared_at,
            signature,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AgentName, AgentRole, AgentSubstrate};

    fn sample_agent() -> Agent {
        Agent::new(
            Did::from_ed25519_public_key(&[0x99; 32]),
            AgentRole::Scribe,
            AgentName::parse("claude").unwrap(),
            AgentSubstrate::parse("claude-code").unwrap(),
            Utc::now(),
        )
    }

    #[test]
    fn target_org_roundtrip() {
        let did = Did::from_ed25519_public_key(&[0x11; 32]);
        let t = ManifestTarget::Org(did.clone());
        let s = t.as_string();
        assert!(s.starts_with("org:did:key:"));
        let back = ManifestTarget::parse(&s).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn target_channel_roundtrip() {
        let did = Did::from_ed25519_public_key(&[0x22; 32]);
        let t = ManifestTarget::Channel {
            owner: did.clone(),
            handle: "assemblee_generale".to_string(),
        };
        let s = t.as_string();
        let back = ManifestTarget::parse(&s).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn target_global_roundtrip() {
        let t = ManifestTarget::Global;
        assert_eq!(t.as_string(), "*");
        let back = ManifestTarget::parse("*").unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn target_rejects_unknown_form() {
        assert!(matches!(
            ManifestTarget::parse("garbage"),
            Err(ManifestTargetParseError::UnknownForm(_))
        ));
    }

    #[test]
    fn sign_then_verify_roundtrip() {
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x42; 32]);
        let signer = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        let target = ManifestTarget::Org(Did::from_ed25519_public_key(&[0x11; 32]));
        let agents = vec![sample_agent()];
        let when = Utc::now();
        let manifest = AgentManifest::sign(signer.clone(), target.clone(), agents, when, &key);
        assert!(manifest.verify(&key.verifying_key()));
    }

    #[test]
    fn tampered_manifest_fails_verify() {
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x42; 32]);
        let signer = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        let target = ManifestTarget::Org(Did::from_ed25519_public_key(&[0x11; 32]));
        let agents = vec![sample_agent()];
        let when = Utc::now();
        let mut manifest =
            AgentManifest::sign(signer, target, agents, when, &key);
        // Mutate the authorized_agents list without re-signing.
        manifest.authorized_agents.push(sample_agent());
        assert!(!manifest.verify(&key.verifying_key()));
    }

    #[test]
    fn frontmatter_roundtrip() {
        let key = ed25519_dalek::SigningKey::from_bytes(&[0x42; 32]);
        let signer = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        let target = ManifestTarget::Channel {
            owner: Did::from_ed25519_public_key(&[0x33; 32]),
            handle: "finance".to_string(),
        };
        let agents = vec![sample_agent()];
        let when = Utc::now();
        let original =
            AgentManifest::sign(signer.clone(), target.clone(), agents.clone(), when, &key);

        let fm: AgentManifestFrontmatter = (&original).into();
        let reparsed: AgentManifest = fm.try_into().unwrap();
        assert!(reparsed.verify(&key.verifying_key()));
        // Times are RFC3339-rounded — compare via canonical equivalence
        // (signer/target/authorized_agents must match exactly; declared_at
        // round-trips through rfc3339 so equality holds at second granularity).
        assert_eq!(reparsed.signer, original.signer);
        assert_eq!(reparsed.target, original.target);
        assert_eq!(reparsed.authorized_agents, original.authorized_agents);
    }
}
