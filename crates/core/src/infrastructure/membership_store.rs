//! Org membership record at `<root>/orgs/<alias>/membership.local.md`.
//!
//! Declares the principal's membership in an org: the org's DID, the role
//! they were granted, the relay endpoint where the org's channels live,
//! and when they joined. Written by `accept_org_invite` (next slice);
//! read by the daemon's `enumerate_subscribed_queues` to discover which
//! org channels to poll.
//!
//! **`.local.md` suffix is load-bearing.** Per AGENTS.md rule #6, the
//! `.local` infix marks this file as **private to the subscriber** —
//! receiver-side state, never sent on wire, ignored by future backup
//! systems. The org's roster lives on the relay's authoritative state;
//! this file is the principal's local record of "I joined this org as X."
//!
//! ## Shape
//!
//! ```markdown
//! ---
//! $type: tech.equanimi.secretariat.orgMembership
//! org_did: did:web:equanimi.tech
//! role: publish
//! relay_endpoint: https://relay.equanimi.tech
//! joined_at: 2026-05-19T18:30:00Z
//! inviter_did: did:key:z6MkjB...   # optional
//! ---
//! # Membership in equanimi.tech
//!
//! Free-form principal-editable prose. Notes about why joined, who's
//! who in the org, anything restoring this vault should know.
//! ```
//!
//! ## What's NOT in this file
//!
//! - **The list of channels.** Channels are discovered by walking
//!   `<root>/orgs/<alias>/channels/<handle-path>/` for directories with a
//!   `channel.md` marker — filesystem-authoritative per
//!   [[project_filesystem_authoritative]]. The membership file declares
//!   org-level facts; the filesystem declares "which channels do I
//!   have locally."
//! - **The org's full roster** (who else is a member). That lives on the
//!   owner's relay as authoritative state. Subscribers don't need to
//!   know — they post + poll their own queues.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{Did, RelayEndpoint, RelayEndpointParseError};

const MEMBERSHIP_TYPE: &str = "tech.equanimi.secretariat.orgMembership";
pub const MEMBERSHIP_FILENAME: &str = "membership.local.md";

const BUILTIN_BODY: &str = "\n# membership\n\n\
Local record of this principal's membership in the org. Free-form prose; \
the load-bearing facts are in the frontmatter.\n";

#[derive(Debug, Error)]
pub enum MembershipStoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed frontmatter at {path}: {message}")]
    MalformedFrontmatter { path: PathBuf, message: String },
    #[error("invalid org_did `{did}`: {reason}")]
    InvalidOrgDid { did: String, reason: String },
    #[error("invalid inviter_did `{did}`: {reason}")]
    InvalidInviterDid { did: String, reason: String },
    #[error("invalid relay_endpoint `{value}`: {source}")]
    InvalidRelayEndpoint {
        value: String,
        #[source]
        source: RelayEndpointParseError,
    },
    #[error("invalid joined_at `{value}` at {path}")]
    InvalidTimestamp { value: String, path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrgMembership {
    pub org_did: Did,
    /// Role granted to this principal on the org's channels. Conventionally
    /// `subscribe` | `publish` | `admin` per the rosterUpdate lexicon, but
    /// stored as a free-form string — validation of known values lives
    /// upstream (where the role is applied), not here.
    pub role: String,
    pub relay_endpoint: RelayEndpoint,
    pub joined_at: DateTime<Utc>,
    /// Optional — the inviter's DID, when this membership came from an
    /// invite. None when membership was bootstrapped some other way.
    pub inviter_did: Option<Did>,
    /// Free-form principal-editable prose body. Includes everything after
    /// the closing `---` frontmatter delimiter.
    pub body: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct MembershipFrontmatter {
    #[serde(rename = "$type", default, skip_serializing_if = "String::is_empty")]
    ty: String,
    org_did: String,
    role: String,
    relay_endpoint: String,
    joined_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    inviter_did: Option<String>,
}

/// Load `membership.local.md`. Returns `Ok(None)` if the file doesn't
/// exist — caller treats absence as "no membership recorded for this org."
pub fn load_membership(path: &Path) -> Result<Option<OrgMembership>, MembershipStoreError> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path).map_err(|e| MembershipStoreError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let (yaml, body) = split_frontmatter(&raw).ok_or_else(|| {
        MembershipStoreError::MalformedFrontmatter {
            path: path.to_path_buf(),
            message: "missing `---` frontmatter delimiters".into(),
        }
    })?;
    let fm: MembershipFrontmatter =
        serde_yaml::from_str(yaml).map_err(|e| MembershipStoreError::MalformedFrontmatter {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
    let m = finalize(fm, body.to_string(), path)?;
    Ok(Some(m))
}

/// Atomic save (temp + rename). Creates parent dirs on demand.
pub fn save_membership(
    path: &Path,
    membership: &OrgMembership,
) -> Result<(), MembershipStoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| MembershipStoreError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let fm = MembershipFrontmatter {
        ty: MEMBERSHIP_TYPE.to_string(),
        org_did: membership.org_did.as_str().to_string(),
        role: membership.role.clone(),
        relay_endpoint: membership.relay_endpoint.as_str().to_string(),
        joined_at: membership.joined_at.to_rfc3339(),
        inviter_did: membership.inviter_did.as_ref().map(|d| d.as_str().to_string()),
    };
    let yaml = serde_yaml::to_string(&fm).map_err(|e| MembershipStoreError::MalformedFrontmatter {
        path: path.to_path_buf(),
        message: format!("could not serialize frontmatter: {e}"),
    })?;
    let body = if membership.body.is_empty() {
        BUILTIN_BODY.to_string()
    } else {
        membership.body.clone()
    };
    let content = format!("---\n{yaml}---\n{body}");

    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, content).map_err(|e| MembershipStoreError::Io {
        path: tmp.clone(),
        source: e,
    })?;
    std::fs::rename(&tmp, path).map_err(|e| MembershipStoreError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

fn finalize(
    fm: MembershipFrontmatter,
    body: String,
    path: &Path,
) -> Result<OrgMembership, MembershipStoreError> {
    let org_did = Did::parse(&fm.org_did).map_err(|e| MembershipStoreError::InvalidOrgDid {
        did: fm.org_did.clone(),
        reason: e.to_string(),
    })?;
    let relay_endpoint = RelayEndpoint::parse(&fm.relay_endpoint).map_err(|e| {
        MembershipStoreError::InvalidRelayEndpoint {
            value: fm.relay_endpoint.clone(),
            source: e,
        }
    })?;
    let joined_at = DateTime::parse_from_rfc3339(&fm.joined_at)
        .map_err(|_| MembershipStoreError::InvalidTimestamp {
            value: fm.joined_at.clone(),
            path: path.to_path_buf(),
        })?
        .with_timezone(&Utc);
    let inviter_did = match fm.inviter_did {
        Some(s) => Some(Did::parse(&s).map_err(|e| MembershipStoreError::InvalidInviterDid {
            did: s,
            reason: e.to_string(),
        })?),
        None => None,
    };
    Ok(OrgMembership {
        org_did,
        role: fm.role,
        relay_endpoint,
        joined_at,
        inviter_did,
        body,
    })
}

fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let yaml = &rest[..end];
    let after = &rest[end + 4..];
    let body = after.strip_prefix('\n').unwrap_or(after);
    Some((yaml, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fixture() -> OrgMembership {
        OrgMembership {
            org_did: Did::from_ed25519_public_key(&[7u8; 32]),
            role: "publish".to_string(),
            relay_endpoint: RelayEndpoint::parse("https://relay.equanimi.tech").unwrap(),
            joined_at: DateTime::parse_from_rfc3339("2026-05-19T18:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
            inviter_did: Some(Did::from_ed25519_public_key(&[1u8; 32])),
            body: String::new(),
        }
    }

    #[test]
    fn load_returns_none_when_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(MEMBERSHIP_FILENAME);
        let r = load_membership(&path).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(MEMBERSHIP_FILENAME);
        let original = fixture();
        save_membership(&path, &original).unwrap();
        let loaded = load_membership(&path).unwrap().unwrap();
        assert_eq!(loaded.org_did, original.org_did);
        assert_eq!(loaded.role, original.role);
        assert_eq!(loaded.relay_endpoint, original.relay_endpoint);
        assert_eq!(loaded.joined_at, original.joined_at);
        assert_eq!(loaded.inviter_did, original.inviter_did);
    }

    #[test]
    fn loads_without_inviter() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(MEMBERSHIP_FILENAME);
        let mut m = fixture();
        m.inviter_did = None;
        save_membership(&path, &m).unwrap();
        let loaded = load_membership(&path).unwrap().unwrap();
        assert!(loaded.inviter_did.is_none());
    }

    #[test]
    fn rejects_malformed_frontmatter() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(MEMBERSHIP_FILENAME);
        std::fs::write(&path, "no frontmatter here").unwrap();
        let r = load_membership(&path);
        assert!(matches!(
            r,
            Err(MembershipStoreError::MalformedFrontmatter { .. })
        ));
    }

    #[test]
    fn rejects_bad_org_did() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(MEMBERSHIP_FILENAME);
        std::fs::write(
            &path,
            "---\n\
$type: tech.equanimi.secretariat.orgMembership\n\
org_did: not-a-did\n\
role: publish\n\
relay_endpoint: https://relay.example\n\
joined_at: 2026-05-19T18:30:00Z\n\
---\n\
body\n",
        )
        .unwrap();
        let r = load_membership(&path);
        assert!(matches!(r, Err(MembershipStoreError::InvalidOrgDid { .. })));
    }

    #[test]
    fn rejects_bad_relay_endpoint() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(MEMBERSHIP_FILENAME);
        std::fs::write(
            &path,
            "---\n\
$type: tech.equanimi.secretariat.orgMembership\n\
org_did: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak\n\
role: publish\n\
relay_endpoint: ftp://wrong-scheme.example\n\
joined_at: 2026-05-19T18:30:00Z\n\
---\n\
body\n",
        )
        .unwrap();
        let r = load_membership(&path);
        assert!(matches!(
            r,
            Err(MembershipStoreError::InvalidRelayEndpoint { .. })
        ));
    }

    #[test]
    fn writes_builtin_body_when_body_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(MEMBERSHIP_FILENAME);
        let m = fixture();
        save_membership(&path, &m).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("# membership"));
    }
}
