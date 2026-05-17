//! Principal identity record at `<self_root>/identity.md`.
//!
//! Consolidates what slice 2 + earlier shipped as four separate files at
//! the vault root (`did`, `key`, `did.json`, `profile.json`) into one
//! frontmatter+body markdown file inside `_self/`. The key itself
//! stays raw binary at `<self_root>/identity/key` (referenced by
//! `key_path` in the frontmatter); the DID document stays JSON at
//! `<self_root>/identity/did.json` (the file principals upload to their
//! `did:web` host).
//!
//! Shape (per `tech.equanimi.secretariat.identity` lexicon):
//!
//! ```markdown
//! ---
//! $type: tech.equanimi.secretariat.identity
//! did: did:web:rafa.equanimi.tech
//! did_method: did:web
//! display_name: Rafa
//! full_name: Rafael T. Ballestiero
//! key_path: identity/key
//! key_type: ed25519
//! key_created_at: 2026-05-12T05:55:00Z
//! key_rotations: []
//! created_at: 2026-05-12T05:55:00Z
//! ---
//!
//! # Identity
//!
//! Free-form principal-editable prose.
//! ```
//!
//! No backward-compat reads. Pre-v0.7 vaults migrate via
//! `scripts/migrate-vault-v0.7.0.sh` BEFORE upgrading the binary; the
//! Rust code only knows the new shape.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{Did, DisplayName, DisplayNameParseError};

const IDENTITY_TYPE: &str = "tech.equanimi.secretariat.identity";
const DEFAULT_KEY_PATH: &str = "identity/key";
const DEFAULT_KEY_TYPE: &str = "ed25519";

const BUILTIN_BODY: &str = "\n# identity\n\n\
Principal-editable prose. Free-form notes about the identity — preferred \
name, signature line, anything someone restoring this vault should know.\n";

#[derive(Debug, Error)]
pub enum IdentityStoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed frontmatter at {path}: {message}")]
    MalformedFrontmatter { path: PathBuf, message: String },
    #[error("invalid did `{did}`: {reason}")]
    InvalidDid { did: String, reason: String },
    #[error("invalid display_name: {0}")]
    InvalidName(DisplayNameParseError),
    #[error("invalid created_at `{value}` at {path}")]
    InvalidTimestamp { value: String, path: PathBuf },
}

/// One past key rotation. Appended to `key_rotations[]` when a rotation
/// ships (future wedge); active key always lives at `key_path`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRotation {
    /// Path to the archived key, relative to `identity.md`.
    pub archived_path: String,
    /// When the rotation happened.
    pub rotated_at: DateTime<Utc>,
    /// Optional principal-supplied reason ("device-lost", "scheduled").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrincipalIdentity {
    pub did: Did,
    /// Method of the DID — `did:key` or `did:web`. Redundant with the
    /// DID itself but useful for tooling that wants to branch without
    /// reparsing.
    pub did_method: String,
    pub display_name: DisplayName,
    /// Formal name (envelope signatures, legal artifacts). Optional;
    /// defaults to display_name when missing.
    pub full_name: Option<String>,
    /// Path to the active key, relative to `identity.md`. Conventionally
    /// `identity/key`.
    pub key_path: String,
    /// Cryptographic algorithm — `ed25519` today.
    pub key_type: String,
    /// When this active key was generated.
    pub key_created_at: DateTime<Utc>,
    /// Rotation history. Append on rotation; never delete.
    pub key_rotations: Vec<KeyRotation>,
    /// When this identity record was first written.
    pub created_at: DateTime<Utc>,
    /// Free-form principal-editable prose body.
    pub body: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct IdentityFrontmatter {
    #[serde(rename = "$type", default, skip_serializing_if = "String::is_empty")]
    ty: String,
    did: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    did_method: String,
    display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    full_name: Option<String>,
    #[serde(default = "default_key_path")]
    key_path: String,
    #[serde(default = "default_key_type")]
    key_type: String,
    key_created_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    key_rotations: Vec<KeyRotation>,
    created_at: String,
}

fn default_key_path() -> String {
    DEFAULT_KEY_PATH.to_string()
}

fn default_key_type() -> String {
    DEFAULT_KEY_TYPE.to_string()
}

/// Load the principal's identity from `identity.md`. Returns `Ok(None)`
/// if the file doesn't exist.
pub fn load_identity(path: &Path) -> Result<Option<PrincipalIdentity>, IdentityStoreError> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path).map_err(|e| IdentityStoreError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let (yaml, body) = split_frontmatter(&raw).ok_or_else(|| {
        IdentityStoreError::MalformedFrontmatter {
            path: path.to_path_buf(),
            message: "missing `---` frontmatter delimiters".into(),
        }
    })?;
    let fm: IdentityFrontmatter =
        serde_yaml::from_str(yaml).map_err(|e| IdentityStoreError::MalformedFrontmatter {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
    let identity = finalize(fm, body.to_string(), path)?;
    Ok(Some(identity))
}

/// Atomic save (temp + rename). Creates parent dirs on demand.
pub fn save_identity(
    path: &Path,
    identity: &PrincipalIdentity,
) -> Result<(), IdentityStoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| IdentityStoreError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let fm = IdentityFrontmatter {
        ty: IDENTITY_TYPE.to_string(),
        did: identity.did.as_str().to_string(),
        did_method: identity.did_method.clone(),
        display_name: identity.display_name.to_string(),
        full_name: identity.full_name.clone(),
        key_path: identity.key_path.clone(),
        key_type: identity.key_type.clone(),
        key_created_at: identity.key_created_at.to_rfc3339(),
        key_rotations: identity.key_rotations.clone(),
        created_at: identity.created_at.to_rfc3339(),
    };
    let yaml = serde_yaml::to_string(&fm).map_err(|e| IdentityStoreError::MalformedFrontmatter {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    let body = if identity.body.is_empty() {
        BUILTIN_BODY.to_string()
    } else {
        identity.body.clone()
    };
    let rendered = format!("---\n{yaml}---\n{body}");
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, rendered).map_err(|e| IdentityStoreError::Io {
        path: tmp.clone(),
        source: e,
    })?;
    std::fs::rename(&tmp, path).map_err(|e| IdentityStoreError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

fn finalize(
    fm: IdentityFrontmatter,
    body: String,
    path: &Path,
) -> Result<PrincipalIdentity, IdentityStoreError> {
    let did = Did::parse(&fm.did).map_err(|e| IdentityStoreError::InvalidDid {
        did: fm.did.clone(),
        reason: e.to_string(),
    })?;
    let did_method = if fm.did_method.is_empty() {
        did_method_of(&did)
    } else {
        fm.did_method
    };
    let display_name =
        DisplayName::parse(&fm.display_name).map_err(IdentityStoreError::InvalidName)?;
    let key_created_at = parse_ts(&fm.key_created_at, path)?;
    let created_at = parse_ts(&fm.created_at, path)?;
    Ok(PrincipalIdentity {
        did,
        did_method,
        display_name,
        full_name: fm.full_name,
        key_path: fm.key_path,
        key_type: fm.key_type,
        key_created_at,
        key_rotations: fm.key_rotations,
        created_at,
        body,
    })
}

fn parse_ts(value: &str, path: &Path) -> Result<DateTime<Utc>, IdentityStoreError> {
    DateTime::parse_from_rfc3339(value)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| IdentityStoreError::InvalidTimestamp {
            value: value.to_string(),
            path: path.to_path_buf(),
        })
}

fn did_method_of(did: &Did) -> String {
    let s = did.as_str();
    if let Some(rest) = s.strip_prefix("did:") {
        if let Some((method, _)) = rest.split_once(':') {
            return format!("did:{method}");
        }
    }
    String::new()
}

/// Split a `---\n...\n---\n<body>` document. Same shape as the parser in
/// `contract_store`; duplicated locally to keep modules decoupled.
fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let stripped = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    let after_open = stripped
        .strip_prefix("---\r\n")
        .or_else(|| stripped.strip_prefix("---\n"))?;
    let mut search_start = 0usize;
    while let Some(rel) = after_open[search_start..].find("\n---") {
        let abs = search_start + rel;
        let after_dashes = abs + 4;
        let tail = &after_open[after_dashes..];
        if let Some(after_lf) = tail.strip_prefix('\n') {
            return Some((&after_open[..abs], after_lf));
        }
        if let Some(after_crlf) = tail.strip_prefix("\r\n") {
            return Some((&after_open[..abs], after_crlf));
        }
        if tail.is_empty() {
            return Some((&after_open[..abs], ""));
        }
        search_start = abs + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn rafa_did() -> Did {
        Did::parse("did:web:rafa.equanimi.tech").unwrap()
    }

    fn sample(when: DateTime<Utc>) -> PrincipalIdentity {
        PrincipalIdentity {
            did: rafa_did(),
            did_method: "did:web".to_string(),
            display_name: DisplayName::parse("Rafa").unwrap(),
            full_name: Some("Rafael T. Ballestiero".to_string()),
            key_path: DEFAULT_KEY_PATH.to_string(),
            key_type: DEFAULT_KEY_TYPE.to_string(),
            key_created_at: when,
            key_rotations: Vec::new(),
            created_at: when,
            body: BUILTIN_BODY.to_string(),
        }
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("identity.md");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let id = sample(when);
        save_identity(&path, &id).unwrap();
        let loaded = load_identity(&path).unwrap().unwrap();
        assert_eq!(loaded, id);
    }

    #[test]
    fn missing_file_returns_none() {
        let dir = TempDir::new().unwrap();
        assert!(load_identity(&dir.path().join("identity.md"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn did_method_extraction() {
        assert_eq!(
            did_method_of(&Did::parse("did:web:example.com").unwrap()),
            "did:web"
        );
        assert_eq!(
            did_method_of(&Did::from_ed25519_public_key(&[0xa1; 32])),
            "did:key"
        );
    }

    #[test]
    fn body_preserved_across_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("identity.md");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let mut id = sample(when);
        id.body = "\n# custom\n\nMy hand-written body.\n".to_string();
        save_identity(&path, &id).unwrap();
        let loaded = load_identity(&path).unwrap().unwrap();
        assert_eq!(loaded.body, id.body);
    }
}
