//! Per-level **consumption** contract storage at
//! `<channel-dir>/contract.local.md` (and the org-root variant at
//! `<org-dir>/contract.local.md`).
//!
//! These files are **private to the subscriber**: how *they* approach
//! the channel/org, not what the channel demands of its members. Never
//! sent on wire. Never committed to a shared substrate. The `.local`
//! suffix mirrors the Claude Code convention (`CLAUDE.md` shared vs
//! `CLAUDE.local.md` private) so visibility is explicit in the
//! filename.
//!
//! See [`crate::domain::ChannelContract`] for the consumption-vs-
//! governance split. Bare `contract.md` (without `.local`) is reserved
//! for the future **governance** artifact — roster, channel-wide
//! artifact policy, shared with all subscribers; eventually a signed
//! envelope.
//!
//! On-disk shape: markdown file with YAML frontmatter:
//!
//! ```text
//! ---
//! $type: tech.equanimi.secretariat.channelContract
//! cadence_floor_minutes: 15
//! min_trust: signed-only
//! ---
//!
//! # Free-form prose explaining my preferences.
//! ```
//!
//! v0.3 slice 1a: read/write a single contract file. The accumulate
//! resolver (walking my org-root → ancestor channels → leaf and merging
//! within my own chain) lives in a later slice per
//! `docs/pitches/2026-05-12-channel-contracts-mcp.md`.
//!
//! An empty-frontmatter file (`---\n---\n` + prose) round-trips as
//! `ChannelContract::empty()` — the auto-scaffold stub.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{ChannelBinding, ChannelContract, QueueHandle, TrustGate};

use super::channel_def_store::channel_dir;

/// On-disk filename for the **private consumption** contract.
///
/// Suffix `.local` mirrors the Claude Code convention
/// (`CLAUDE.md` shared vs `CLAUDE.local.md` private). The bare
/// `contract.md` filename is reserved for the future **governance**
/// artifact — shared with the roster, eventually a signed envelope.
pub const CONTRACT_FILENAME: &str = "contract.local.md";

/// On-disk `$type` discriminator. Mirrors the lexicon name even though
/// v0.3 does not yet drive runtime validation against the schema.
const CONTRACT_TYPE: &str = "tech.equanimi.secretariat.channelContract";

/// Built-in fallback when `~/.secretariat/_self/contract-stub.md`
/// is absent. The principal can override this by writing their own
/// stub there; `save_stub_if_absent` reads it.
///
/// Three soft dimensions the cognition agent reads at review time.
/// Section bodies are free-form prose — defaults below the headers
/// stand in until the principal rewrites them.
const BUILTIN_STUB_BODY: &str = "\n# detail\n\ndefault: lede.\n\n\
# importance\n\ndefault: normal.\n\n\
# notes\n";

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
    #[error("unknown min_trust `{value}` at {path} (want `signed-only` or `stamp-required`)")]
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

/// Write a `contract.local.md`. The parent directory must already exist.
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
    let tmp = path.with_extension("local.md.tmp");
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

/// Write the stub `contract.local.md` that ships with a freshly-created
/// channel: empty frontmatter (no overrides) + a short body. Body is
/// the principal's own `contract-stub.md` (typically at
/// `<self_root>/contract-stub.md`) when `override_stub` points at an
/// existing file; otherwise falls back to [`BUILTIN_STUB_BODY`].
///
/// No-op if the contract file already exists — auto-scaffold is
/// idempotent across repeated `create_channel` calls so we never
/// silently overwrite hand-edited contracts.
pub fn save_stub_if_absent(
    path: &Path,
    override_stub: Option<&Path>,
) -> Result<bool, ContractStoreError> {
    if path.exists() {
        return Ok(false);
    }
    let body = resolve_stub_body(override_stub)?;
    save_contract(path, &ChannelContract::empty(), &body, false)?;
    Ok(true)
}

/// Read the principal's custom stub body, or fall back to the built-in.
fn resolve_stub_body(override_stub: Option<&Path>) -> Result<String, ContractStoreError> {
    match override_stub {
        Some(p) if p.is_file() => std::fs::read_to_string(p).map_err(|e| ContractStoreError::Io {
            path: p.to_path_buf(),
            source: e,
        }),
        _ => Ok(BUILTIN_STUB_BODY.to_string()),
    }
}

/// Load a contract. Returns `Ok(None)` if the file doesn't exist.
pub fn load_contract(
    path: &Path,
) -> Result<Option<(ChannelContract, String)>, ContractStoreError> {
    Ok(load_contract_with_binding(path)?.map(|(c, _b, body)| (c, body)))
}

/// Load a contract along with its [`ChannelBinding`]. The binding is
/// receiver-private metadata living in the same frontmatter — only the
/// parser separates the two value objects.
pub fn load_contract_with_binding(
    path: &Path,
) -> Result<Option<(ChannelContract, ChannelBinding, String)>, ContractStoreError> {
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
    let (contract, binding) = parse_frontmatter_full(yaml_block, path)?;
    Ok(Some((contract, binding, body.to_string())))
}

// -- on-disk shape ------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
struct ContractFile {
    #[serde(rename = "$type", default, skip_serializing_if = "String::is_empty")]
    ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cadence_floor_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min_trust: Option<String>,
    /// Receiver-private binding fields. Leaf-only — never inherited via
    /// the accumulate merge. Sibling to `ChannelContract` in the on-disk
    /// shape; lifted into [`ChannelBinding`] by the parser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    root_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    launch_command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    launch_args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    launch_env: BTreeMap<String, String>,
}

fn emit_frontmatter(contract: &ChannelContract) -> Result<String, String> {
    emit_frontmatter_full(contract, &ChannelBinding::empty())
}

fn emit_frontmatter_full(
    contract: &ChannelContract,
    binding: &ChannelBinding,
) -> Result<String, String> {
    let file = ContractFile {
        ty: CONTRACT_TYPE.to_string(),
        cadence_floor_minutes: contract.cadence_floor_minutes,
        min_trust: contract.min_trust.map(|g| g.as_str().to_string()),
        root_path: binding.root_path.clone(),
        launch_command: binding.launch_command.clone(),
        launch_args: binding.launch_args.clone(),
        launch_env: binding.launch_env.clone(),
    };
    serde_yaml::to_string(&file).map_err(|e| e.to_string())
}

fn parse_frontmatter_full(
    yaml: &str,
    path: &Path,
) -> Result<(ChannelContract, ChannelBinding), ContractStoreError> {
    if yaml.trim().is_empty() {
        return Ok((ChannelContract::empty(), ChannelBinding::empty()));
    }
    let file: ContractFile = serde_yaml::from_str(yaml).map_err(|e| {
        ContractStoreError::MalformedFrontmatter {
            path: path.to_path_buf(),
            message: e.to_string(),
        }
    })?;
    let min_trust = match file.min_trust.as_deref() {
        None | Some("") => None,
        Some(s) => Some(TrustGate::parse(s).ok_or_else(|| {
            ContractStoreError::UnknownTrustGate {
                value: s.to_string(),
                path: path.to_path_buf(),
            }
        })?),
    };
    let contract = ChannelContract {
        cadence_floor_minutes: file.cadence_floor_minutes,
        min_trust,
    };
    let binding = ChannelBinding {
        root_path: file.root_path,
        launch_command: file.launch_command,
        launch_args: file.launch_args,
        launch_env: file.launch_env,
    };
    Ok((contract, binding))
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

    #[test]
    fn stub_round_trips_as_empty_contract() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONTRACT_FILENAME);
        let written = save_stub_if_absent(&path, None).unwrap();
        assert!(written);
        let (loaded, body) = load_contract(&path).unwrap().unwrap();
        assert!(loaded.is_empty());
        assert!(body.contains("# importance"));
    }

    #[test]
    fn stub_is_idempotent_no_overwrite() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONTRACT_FILENAME);
        assert!(save_stub_if_absent(&path, None).unwrap());
        // Hand-edit between calls — second save_stub must not clobber.
        std::fs::write(&path, "---\ncadence_floor_minutes: 30\n---\nhand-edited\n").unwrap();
        let written_again = save_stub_if_absent(&path, None).unwrap();
        assert!(!written_again);
        let (loaded, body) = load_contract(&path).unwrap().unwrap();
        assert_eq!(loaded.cadence_floor_minutes, Some(30));
        assert!(body.contains("hand-edited"));
    }

    #[test]
    fn round_trips_full_contract() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONTRACT_FILENAME);
        let c = ChannelContract {
            cadence_floor_minutes: Some(15),
            min_trust: Some(TrustGate::StampRequired),
        };
        save_contract(&path, &c, "\n# My overrides\n", false).unwrap();
        let (loaded, body) = load_contract(&path).unwrap().unwrap();
        assert_eq!(loaded, c);
        assert!(body.contains("My overrides"));
    }

    #[test]
    fn refuses_to_overwrite_unless_explicit() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONTRACT_FILENAME);
        save_contract(&path, &ChannelContract::empty(), "body\n", false).unwrap();
        let err = save_contract(&path, &ChannelContract::empty(), "body\n", false);
        assert!(matches!(err, Err(ContractStoreError::AlreadyExists(_))));
        // Explicit overwrite succeeds.
        save_contract(&path, &ChannelContract::empty(), "body\n", true).unwrap();
    }

    #[test]
    fn unknown_min_trust_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONTRACT_FILENAME);
        std::fs::write(&path, "---\nmin_trust: wide-open\n---\n").unwrap();
        let r = load_contract(&path);
        assert!(matches!(r, Err(ContractStoreError::UnknownTrustGate { .. })));
    }

    #[test]
    fn unknown_frontmatter_fields_ignored() {
        // Hand-scaffolded contracts may carry fields not in the v1
        // consumption shape (e.g. legacy `roster`, `preferred_transports`,
        // `inherit_from_parent`). These belong to governance, not
        // consumption — we just ignore them rather than failing the load.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONTRACT_FILENAME);
        std::fs::write(
            &path,
            "---\nroster:\n  - did:web:rafa.equanimi.tech\ninherit_from_parent: true\ncadence_floor_minutes: 30\n---\n",
        )
        .unwrap();
        let (loaded, _) = load_contract(&path).unwrap().unwrap();
        assert_eq!(loaded.cadence_floor_minutes, Some(30));
        assert!(loaded.min_trust.is_none());
    }

    #[test]
    fn missing_file_loads_as_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONTRACT_FILENAME);
        assert!(load_contract(&path).unwrap().is_none());
    }

    #[test]
    fn missing_frontmatter_delimiters_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONTRACT_FILENAME);
        std::fs::write(&path, "no frontmatter at all\n").unwrap();
        let r = load_contract(&path);
        assert!(matches!(r, Err(ContractStoreError::MalformedFrontmatter { .. })));
    }

    #[test]
    fn parses_root_path_into_binding() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONTRACT_FILENAME);
        std::fs::write(
            &path,
            "---\ncadence_floor_minutes: 15\nroot_path: /Users/rafa/Developer/secretariat\n---\nbody\n",
        )
        .unwrap();
        let (_, binding, body) = load_contract_with_binding(&path).unwrap().unwrap();
        assert_eq!(
            binding.root_path.as_deref(),
            Some(std::path::Path::new("/Users/rafa/Developer/secretariat"))
        );
        assert!(body.contains("body"));
    }

    #[test]
    fn binding_empty_when_root_path_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONTRACT_FILENAME);
        std::fs::write(&path, "---\ncadence_floor_minutes: 15\n---\n").unwrap();
        let (_, binding, _) = load_contract_with_binding(&path).unwrap().unwrap();
        assert!(binding.is_empty());
    }
}
