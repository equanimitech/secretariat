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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature as DalekSignature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;

use crate::domain::{Agent, Did, DisplayName, DisplayNameParseError, Signature};

const IDENTITY_TYPE: &str = "tech.equanimi.secretariat.identity";
const DEFAULT_KEY_PATH: &str = "identity/key";
const DEFAULT_KEY_TYPE: &str = "ed25519";
/// Canonical preimage version tag. Bump if the canonicalization shape changes.
const CANONICAL_PREIMAGE_TAG: &[u8] = b"identity:v0:";

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
    #[error("identity record at {path} has an invalid signature \
            (tampered or wrong key)")]
    SignatureInvalid { path: PathBuf },
    #[error("did `{0}` is not a did:key — cannot derive verifying key without DID document")]
    NotDidKey(String),
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
    /// Agents the principal has delegated signing authority to (scribes
    /// today; future roles reuse the shape). Empty for fresh installs;
    /// populated via `sec agent add`. See `docs/pitches/2026-05-21-substrate-for-themia.md`.
    pub authorized_agents: Vec<Agent>,
    /// When this identity record was first written.
    pub created_at: DateTime<Utc>,
    /// Detached ed25519 signature over the canonical preimage of this record
    /// (sans the signature field itself), signed by the principal's active
    /// key. `None` for legacy records written before identity signing
    /// landed; daemon migration re-signs on first tick post-upgrade.
    pub signature: Option<Signature>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    authorized_agents: Vec<Agent>,
    created_at: String,
    /// Detached signature over canonical preimage. Skipped when absent
    /// (legacy records).
    #[serde(
        rename = "$signature",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    signature: Option<Signature>,
}

fn default_key_path() -> String {
    DEFAULT_KEY_PATH.to_string()
}

fn default_key_type() -> String {
    DEFAULT_KEY_TYPE.to_string()
}

/// Load the principal's identity from `identity.md`. Returns `Ok(None)`
/// if the file doesn't exist.
///
/// **Does NOT verify the embedded signature.** Use [`load_identity_verified`]
/// when the caller needs cryptographic assurance that the record hasn't been
/// tampered with; callers that just want the parsed shape (e.g. for display)
/// can use this fn directly.
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

/// Load + verify the principal's identity. If the record carries a signature,
/// verify it against the verifying key derived from `did:key` (for did:key
/// principals) or supplied by the caller (for did:web — `expected_pubkey`).
/// Records without a signature (legacy, pre-Move-1) load unchanged but the
/// caller is responsible for re-signing on the next save.
pub fn load_identity_verified(
    path: &Path,
    expected_pubkey_for_did_web: Option<&VerifyingKey>,
) -> Result<Option<PrincipalIdentity>, IdentityStoreError> {
    let Some(identity) = load_identity(path)? else {
        return Ok(None);
    };
    if let Some(sig) = &identity.signature {
        let vk = match expected_pubkey_for_did_web {
            Some(k) => *k,
            None => {
                let pk = identity
                    .did
                    .embedded_ed25519_key()
                    .ok_or_else(|| IdentityStoreError::NotDidKey(identity.did.to_string()))?;
                VerifyingKey::from_bytes(&pk)
                    .map_err(|_| IdentityStoreError::SignatureInvalid {
                        path: path.to_path_buf(),
                    })?
            }
        };
        let preimage = canonical_preimage(&identity);
        let dalek_sig = DalekSignature::from_bytes(sig.as_bytes());
        vk.verify(&preimage, &dalek_sig)
            .map_err(|_| IdentityStoreError::SignatureInvalid {
                path: path.to_path_buf(),
            })?;
    }
    Ok(Some(identity))
}

/// Atomic save (temp + rename). Creates parent dirs on demand.
///
/// If `signing_key` is `Some`, signs the canonical preimage and embeds the
/// signature in the record's `$signature` frontmatter field. `None` writes
/// the record unsigned (legacy path; only used by migration scripts).
pub fn save_identity(
    path: &Path,
    identity: &PrincipalIdentity,
    signing_key: Option<&SigningKey>,
) -> Result<(), IdentityStoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| IdentityStoreError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    // Build identity with the freshly-computed signature (if signing_key
    // provided) so the on-disk record always reflects what was signed.
    let mut to_write = identity.clone();
    if let Some(key) = signing_key {
        let preimage = canonical_preimage(&to_write);
        let sig = key.sign(&preimage);
        to_write.signature = Some(Signature::from_bytes(sig.to_bytes()));
    }
    let fm = IdentityFrontmatter {
        ty: IDENTITY_TYPE.to_string(),
        did: to_write.did.as_str().to_string(),
        did_method: to_write.did_method.clone(),
        display_name: to_write.display_name.to_string(),
        full_name: to_write.full_name.clone(),
        key_path: to_write.key_path.clone(),
        key_type: to_write.key_type.clone(),
        key_created_at: to_write.key_created_at.to_rfc3339(),
        key_rotations: to_write.key_rotations.clone(),
        authorized_agents: to_write.authorized_agents.clone(),
        created_at: to_write.created_at.to_rfc3339(),
        signature: to_write.signature.clone(),
    };
    let yaml = serde_yaml::to_string(&fm).map_err(|e| IdentityStoreError::MalformedFrontmatter {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    let body = if to_write.body.is_empty() {
        BUILTIN_BODY.to_string()
    } else {
        to_write.body.clone()
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

/// Canonical preimage for identity-record signature. Serializes every field
/// except `signature` itself as a sorted-key JSON object, prefixed with a
/// version tag, suffixed with U+001F + body bytes.
///
/// Determinism comes from `BTreeMap`'s key ordering; nested structs serialize
/// in their declaration order (stable as long as the struct definitions
/// don't change). If the canonical shape changes, bump `CANONICAL_PREIMAGE_TAG`.
fn canonical_preimage(identity: &PrincipalIdentity) -> Vec<u8> {
    let mut map: BTreeMap<&str, JsonValue> = BTreeMap::new();
    map.insert("did", JsonValue::String(identity.did.as_str().to_string()));
    map.insert(
        "did_method",
        JsonValue::String(identity.did_method.clone()),
    );
    map.insert(
        "display_name",
        JsonValue::String(identity.display_name.to_string()),
    );
    if let Some(fn_) = &identity.full_name {
        map.insert("full_name", JsonValue::String(fn_.clone()));
    }
    map.insert("key_path", JsonValue::String(identity.key_path.clone()));
    map.insert("key_type", JsonValue::String(identity.key_type.clone()));
    map.insert(
        "key_created_at",
        JsonValue::String(identity.key_created_at.to_rfc3339()),
    );
    map.insert(
        "key_rotations",
        serde_json::to_value(&identity.key_rotations).unwrap_or(JsonValue::Array(vec![])),
    );
    map.insert(
        "authorized_agents",
        serde_json::to_value(&identity.authorized_agents).unwrap_or(JsonValue::Array(vec![])),
    );
    map.insert(
        "created_at",
        JsonValue::String(identity.created_at.to_rfc3339()),
    );

    let mut out = CANONICAL_PREIMAGE_TAG.to_vec();
    let json = serde_json::to_vec(&map).unwrap_or_default();
    out.extend(json);
    out.push(0x1F);
    out.extend(identity.body.as_bytes());
    out
}

/// Helper: sign an identity record without persisting. Useful for emitting
/// the snapshot into a MembershipClaim envelope (where the signature is
/// embedded in the envelope, not the on-disk record).
pub fn sign_identity(identity: &PrincipalIdentity, key: &SigningKey) -> Signature {
    let preimage = canonical_preimage(identity);
    let sig = key.sign(&preimage);
    Signature::from_bytes(sig.to_bytes())
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
        authorized_agents: fm.authorized_agents,
        created_at,
        signature: fm.signature,
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
            authorized_agents: Vec::new(),
            created_at: when,
            signature: None,
            body: BUILTIN_BODY.to_string(),
        }
    }

    fn didkey_sample(when: DateTime<Utc>) -> (PrincipalIdentity, SigningKey) {
        use crate::domain::{Agent, AgentName, AgentRole, AgentSubstrate};

        let key = SigningKey::from_bytes(&[0x42; 32]);
        let pk = key.verifying_key().to_bytes();
        let did = Did::from_ed25519_public_key(&pk);
        let id = PrincipalIdentity {
            did,
            did_method: "did:key".to_string(),
            display_name: DisplayName::parse("Rafa").unwrap(),
            full_name: None,
            key_path: DEFAULT_KEY_PATH.to_string(),
            key_type: DEFAULT_KEY_TYPE.to_string(),
            key_created_at: when,
            key_rotations: Vec::new(),
            authorized_agents: vec![Agent::new(
                Did::from_ed25519_public_key(&[0x99; 32]),
                AgentRole::Scribe,
                AgentName::parse("claude").unwrap(),
                AgentSubstrate::parse("claude-code").unwrap(),
                when,
            )],
            created_at: when,
            signature: None,
            body: BUILTIN_BODY.to_string(),
        };
        (id, key)
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("identity.md");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let id = sample(when);
        save_identity(&path, &id, None).unwrap();
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
        save_identity(&path, &id, None).unwrap();
        let loaded = load_identity(&path).unwrap().unwrap();
        assert_eq!(loaded.body, id.body);
    }

    #[test]
    fn signed_record_verifies_against_didkey() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("identity.md");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let (id, key) = didkey_sample(when);
        save_identity(&path, &id, Some(&key)).unwrap();

        // Load directly: signature field is present.
        let loaded = load_identity(&path).unwrap().unwrap();
        assert!(loaded.signature.is_some(), "signature should be embedded");

        // Verified load succeeds.
        let verified = load_identity_verified(&path, None).unwrap().unwrap();
        assert_eq!(verified.signature, loaded.signature);
    }

    #[test]
    fn verified_load_rejects_tampered_body() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("identity.md");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let (id, key) = didkey_sample(when);
        save_identity(&path, &id, Some(&key)).unwrap();

        // Append a line to the body — signature should now fail.
        let mut current = std::fs::read_to_string(&path).unwrap();
        current.push_str("malicious tail\n");
        std::fs::write(&path, current).unwrap();

        let result = load_identity_verified(&path, None);
        assert!(matches!(
            result,
            Err(IdentityStoreError::SignatureInvalid { .. })
        ));
    }

    #[test]
    fn legacy_unsigned_record_loads_clean() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("identity.md");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let id = sample(when);
        // Save with None — emits an unsigned legacy record.
        save_identity(&path, &id, None).unwrap();

        // load_identity_verified accepts it because signature is None.
        let loaded = load_identity_verified(&path, None).unwrap().unwrap();
        assert!(loaded.signature.is_none());
    }

    #[test]
    fn authorized_agents_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("identity.md");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let (id, key) = didkey_sample(when);
        save_identity(&path, &id, Some(&key)).unwrap();

        let loaded = load_identity(&path).unwrap().unwrap();
        assert_eq!(loaded.authorized_agents.len(), 1);
        assert_eq!(loaded.authorized_agents[0].name.as_str(), "claude");
        assert_eq!(loaded.authorized_agents[0].substrate.as_str(), "claude-code");
    }
}
