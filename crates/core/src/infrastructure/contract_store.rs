//! Per-level contract storage at `<channel-dir>/contract.md` (and the
//! org-root variant at `<org-dir>/contract.md`).
//!
//! On-disk shape: markdown file with YAML frontmatter:
//!
//! ```text
//! ---
//! $type: tech.equanimi.secretariat.channelContract
//! cadence_floor_minutes: 15
//! trust_gate: signed-only
//! roster:
//!   - did:web:rafa.equanimi.tech
//! preferred_transports:
//!   - relay:themia.pro
//! ---
//!
//! # Free-form prose explaining the contract.
//! ```
//!
//! v0.3 slice 1a: read/write a single contract file. The accumulate
//! resolver (walking org-root → leaf and merging) lives in a later slice
//! per `docs/pitches/2026-05-12-channel-contracts-mcp.md`.
//!
//! An empty-frontmatter file (`---\n---\n` + prose) round-trips as
//! `ChannelContract::empty()` — the auto-scaffold stub.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{ChannelContract, Did, DidParseError, QueueHandle, TrustGate};

use super::channel_def_store::channel_dir;

pub const CONTRACT_FILENAME: &str = "contract.md";

/// On-disk `$type` discriminator. Mirrors the lexicon name even though
/// v0.3 does not yet drive runtime validation against the schema.
const CONTRACT_TYPE: &str = "tech.equanimi.secretariat.channelContract";

const DEFAULT_STUB_BODY: &str = "\n# Channel contract\n\n\
This file declares this channel's governance overrides. Empty frontmatter \
above means \"contribute nothing to the accumulated contract\" — inherited \
fields from org-root and ancestor channels apply as-is.\n\n\
Edit via `sec channels contract set` (CLI) or `set_channel_contract` (MCP) \
once those verbs ship; in the meantime, edit by hand and re-resolve.\n";

#[derive(Debug, Error)]
pub enum ContractStoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("contract already present at {0} — refuse to overwrite")]
    AlreadyExists(PathBuf),
    #[error("malformed frontmatter at {path}: {message}")]
    MalformedFrontmatter { path: PathBuf, message: String },
    #[error("invalid did `{value}` in roster at {path}: {source}")]
    InvalidDid {
        value: String,
        path: PathBuf,
        #[source]
        source: DidParseError,
    },
    #[error("unknown trust_gate `{value}` at {path} (want `signed-only` or `stamp-required`)")]
    UnknownTrustGate { value: String, path: PathBuf },
}

/// Resolve the on-disk path for a channel's contract file.
pub fn channel_contract_path(channels_root: &Path, handle: &QueueHandle) -> PathBuf {
    channel_dir(channels_root, handle).join(CONTRACT_FILENAME)
}

/// Resolve the on-disk path for an org-root contract file. `org_dir`
/// is `<.secretariat>/orgs/<alias>/`.
pub fn org_contract_path(org_dir: &Path) -> PathBuf {
    org_dir.join(CONTRACT_FILENAME)
}

/// Write a `contract.md`. The parent directory must already exist.
/// When `overwrite` is false and the file is present, returns
/// `AlreadyExists` — the create-channel auto-scaffold relies on this
/// being non-destructive.
pub fn save_contract(
    path: &Path,
    contract: &ChannelContract,
    body: &str,
    overwrite: bool,
) -> Result<(), ContractStoreError> {
    if path.exists() && !overwrite {
        return Err(ContractStoreError::AlreadyExists(path.to_path_buf()));
    }
    let yaml = emit_frontmatter(contract).map_err(|e| ContractStoreError::MalformedFrontmatter {
        path: path.to_path_buf(),
        message: e,
    })?;
    let rendered = format!("---\n{yaml}---\n{body}");
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, rendered.as_bytes()).map_err(|e| ContractStoreError::Io {
        path: tmp.clone(),
        source: e,
    })?;
    std::fs::rename(&tmp, path).map_err(|e| ContractStoreError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// Write the stub `contract.md` that ships with a freshly-created
/// channel: empty frontmatter (no overrides) + a short explanatory
/// body. No-op if the file already exists — auto-scaffold is
/// idempotent across repeated `create_channel` calls so we never
/// silently overwrite hand-edited contracts.
pub fn save_stub_if_absent(path: &Path) -> Result<bool, ContractStoreError> {
    if path.exists() {
        return Ok(false);
    }
    save_contract(path, &ChannelContract::empty(), DEFAULT_STUB_BODY, false)?;
    Ok(true)
}

/// Load a contract. Returns `Ok(None)` if the file doesn't exist.
pub fn load_contract(
    path: &Path,
) -> Result<Option<(ChannelContract, String)>, ContractStoreError> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path).map_err(|e| ContractStoreError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let (yaml_block, body) = split_frontmatter(&raw).ok_or_else(|| {
        ContractStoreError::MalformedFrontmatter {
            path: path.to_path_buf(),
            message: "missing `---` frontmatter delimiters".into(),
        }
    })?;
    let contract = parse_frontmatter(yaml_block, path)?;
    Ok(Some((contract, body.to_string())))
}

// -- on-disk shape ------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
struct ContractFile {
    #[serde(rename = "$type", default, skip_serializing_if = "String::is_empty")]
    ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cadence_floor_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    trust_gate: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    roster: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    preferred_transports: Vec<String>,
}

fn emit_frontmatter(contract: &ChannelContract) -> Result<String, String> {
    let file = ContractFile {
        ty: CONTRACT_TYPE.to_string(),
        cadence_floor_minutes: contract.cadence_floor_minutes,
        trust_gate: contract.trust_gate.map(|g| g.as_str().to_string()),
        roster: contract
            .roster
            .iter()
            .map(|d| d.as_str().to_string())
            .collect(),
        preferred_transports: contract.preferred_transports.clone(),
    };
    serde_yaml::to_string(&file).map_err(|e| e.to_string())
}

fn parse_frontmatter(yaml: &str, path: &Path) -> Result<ChannelContract, ContractStoreError> {
    if yaml.trim().is_empty() {
        return Ok(ChannelContract::empty());
    }
    let file: ContractFile = serde_yaml::from_str(yaml).map_err(|e| {
        ContractStoreError::MalformedFrontmatter {
            path: path.to_path_buf(),
            message: e.to_string(),
        }
    })?;
    let trust_gate = match file.trust_gate.as_deref() {
        None | Some("") => None,
        Some(s) => Some(TrustGate::parse(s).ok_or_else(|| {
            ContractStoreError::UnknownTrustGate {
                value: s.to_string(),
                path: path.to_path_buf(),
            }
        })?),
    };
    let mut roster = Vec::with_capacity(file.roster.len());
    for raw in file.roster {
        let did = Did::parse(&raw).map_err(|source| ContractStoreError::InvalidDid {
            value: raw,
            path: path.to_path_buf(),
            source,
        })?;
        roster.push(did);
    }
    Ok(ChannelContract {
        cadence_floor_minutes: file.cadence_floor_minutes,
        trust_gate,
        roster,
        preferred_transports: file.preferred_transports,
    })
}

/// Split a `---\n...\n---\n<body>` document. Returns `None` if the file
/// doesn't start with a frontmatter delimiter or has no closing one.
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
    use tempfile::TempDir;

    fn did_a() -> Did {
        Did::from_ed25519_public_key(&[0xa1; 32])
    }

    fn did_b() -> Did {
        Did::from_ed25519_public_key(&[0xb2; 32])
    }

    #[test]
    fn stub_round_trips_as_empty_contract() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contract.md");
        let written = save_stub_if_absent(&path).unwrap();
        assert!(written);
        let (loaded, body) = load_contract(&path).unwrap().unwrap();
        assert!(loaded.is_empty());
        assert!(body.contains("Channel contract"));
    }

    #[test]
    fn stub_is_idempotent_no_overwrite() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contract.md");
        assert!(save_stub_if_absent(&path).unwrap());
        // Hand-edit between calls — second save_stub must not clobber.
        std::fs::write(&path, "---\ncadence_floor_minutes: 30\n---\nhand-edited\n").unwrap();
        let written_again = save_stub_if_absent(&path).unwrap();
        assert!(!written_again);
        let (loaded, body) = load_contract(&path).unwrap().unwrap();
        assert_eq!(loaded.cadence_floor_minutes, Some(30));
        assert!(body.contains("hand-edited"));
    }

    #[test]
    fn round_trips_full_contract() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contract.md");
        let c = ChannelContract {
            cadence_floor_minutes: Some(15),
            trust_gate: Some(TrustGate::StampRequired),
            roster: vec![did_a(), did_b()],
            preferred_transports: vec!["relay:themia.pro".into()],
        };
        save_contract(&path, &c, "\n# Org-root\n", false).unwrap();
        let (loaded, body) = load_contract(&path).unwrap().unwrap();
        assert_eq!(loaded, c);
        assert!(body.contains("Org-root"));
    }

    #[test]
    fn refuses_to_overwrite_unless_explicit() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contract.md");
        save_contract(&path, &ChannelContract::empty(), "body\n", false).unwrap();
        let err = save_contract(&path, &ChannelContract::empty(), "body\n", false);
        assert!(matches!(err, Err(ContractStoreError::AlreadyExists(_))));
        // Explicit overwrite succeeds.
        save_contract(&path, &ChannelContract::empty(), "body\n", true).unwrap();
    }

    #[test]
    fn unknown_trust_gate_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contract.md");
        std::fs::write(&path, "---\ntrust_gate: wide-open\n---\n").unwrap();
        let r = load_contract(&path);
        assert!(matches!(r, Err(ContractStoreError::UnknownTrustGate { .. })));
    }

    #[test]
    fn malformed_did_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contract.md");
        std::fs::write(
            &path,
            "---\nroster:\n  - not-a-did\n---\n",
        )
        .unwrap();
        let r = load_contract(&path);
        assert!(matches!(r, Err(ContractStoreError::InvalidDid { .. })));
    }

    #[test]
    fn missing_file_loads_as_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contract.md");
        assert!(load_contract(&path).unwrap().is_none());
    }

    #[test]
    fn missing_frontmatter_delimiters_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contract.md");
        std::fs::write(&path, "no frontmatter at all\n").unwrap();
        let r = load_contract(&path);
        assert!(matches!(r, Err(ContractStoreError::MalformedFrontmatter { .. })));
    }
}
