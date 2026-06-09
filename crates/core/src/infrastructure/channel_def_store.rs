//! Per-channel manifest storage at `<channel_dir>/channel.md`.
//!
//! YAML frontmatter + optional markdown body, mirroring `contract.local.md`.
//! Frontmatter carries the lexicon-shaped record; the body is freeform
//! prose the principal can use to annotate the channel.
//!
//! ```markdown
//! ---
//! $type: tech.equanimi.secretariat.channelDef
//! handle: channel:secretariat:dev
//! name: Secretariat — Dev
//! created_at: 2026-05-12T14:16:08Z
//! ---
//!
//! # Secretariat — Dev
//!
//! Dev workspace for the Secretariat client.
//! ```
//!
//! Legacy: pre-rename channels carried a `.channelDef` JSON sidecar. The
//! loader reads either form transparently; saves always write the new
//! shape. A best-effort one-shot migration renames legacy files in
//! place the first time we encounter them.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{ChannelDef, QueueHandle, QueueHandleError};

const CHANNEL_DEF_TYPE: &str = "tech.equanimi.secretariat.channelDef";
pub const CHANNEL_DEF_FILENAME: &str = "channel.md";
/// Legacy filename; loaded for back-compat, never written.
pub const LEGACY_CHANNEL_DEF_FILENAME: &str = ".channelDef";

const DEFAULT_STUB_BODY: &str = "\n# {NAME}\n\n{DESCRIPTION}\n";

#[derive(Debug, Error)]
pub enum ChannelDefStoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed frontmatter at {path}: {message}")]
    MalformedFrontmatter { path: PathBuf, message: String },
    #[error("malformed json at {path}: {source}")]
    MalformedJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid handle `{handle}`: {source}")]
    InvalidHandle {
        handle: String,
        #[source]
        source: QueueHandleError,
    },
    #[error("invalid created_at `{value}` at {path}")]
    InvalidTimestamp { value: String, path: PathBuf },
    #[error("channel def already present at {0} — refuse to overwrite")]
    AlreadyExists(PathBuf),
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ChannelDefFrontmatter {
    #[serde(rename = "$type", default, skip_serializing_if = "String::is_empty")]
    ty: String,
    #[serde(default)]
    handle: String,
    #[serde(default)]
    name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(default)]
    created_at: String,
    /// Channel-governance policy: receivers MUST treat unstamped envelopes
    /// as ambient on stamp-required channels. Default `false` (omitted from
    /// frontmatter when at default).
    #[serde(default, skip_serializing_if = "is_false")]
    requires_stamp: bool,
    /// Tombstone marker. When `true`, this envelope's channelDef announces
    /// the channel's removal. Receivers drop the local `channel.md` but
    /// preserve `envelopes/` history. Default `false` (omitted).
    #[serde(default, skip_serializing_if = "is_false")]
    tombstoned: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Legacy v1 JSON shape, kept for back-compat reads only.
#[derive(Debug, Serialize, Deserialize)]
struct LegacyChannelDefFile {
    #[serde(default)]
    version: u32,
    handle: String,
    name: String,
    #[serde(default)]
    description: String,
    created_at: String,
}

/// Resolve the on-disk directory for a channel under a given channels root.
/// `foo:bar` → `<channels_root>/foo/bar/`. Walks every segment — v0.5
/// handles no longer carry a leading `channel:` / `inbox:` / `area:`
/// namespace token.
pub fn channel_dir(channels_root: &Path, handle: &QueueHandle) -> PathBuf {
    let mut dir = channels_root.to_path_buf();
    for seg in handle.segments() {
        dir.push(seg);
    }
    dir
}

/// Path to the canonical `channel.md` manifest for a given handle. Note:
/// the existence gate used by capture / launch should check this path
/// OR the legacy `.channelDef` path via [`channel_def_exists`].
pub fn channel_def_path(channels_root: &Path, handle: &QueueHandle) -> PathBuf {
    channel_dir(channels_root, handle).join(CHANNEL_DEF_FILENAME)
}

/// Returns true when either the canonical `channel.md` or the legacy
/// `.channelDef` exists for the channel. Use this for existence gates
/// (capture, launch) so legacy channels stay discoverable until
/// migration runs.
pub fn channel_def_exists(channels_root: &Path, handle: &QueueHandle) -> bool {
    channel_def_exists_in_dir(&channel_dir(channels_root, handle))
}

/// Path-based variant for callers that already have the channel-dir
/// resolved (e.g. tree walks where parsing the handle would be wasted
/// work).
pub fn channel_def_exists_in_dir(dir: &Path) -> bool {
    dir.join(CHANNEL_DEF_FILENAME).is_file() || dir.join(LEGACY_CHANNEL_DEF_FILENAME).is_file()
}

/// Lightweight (name, description) lookup, transparent to the new
/// markdown shape and the legacy JSON. Returns empties when no manifest
/// is present or the file is malformed — listing paths treat that as a
/// soft failure, not an error.
pub fn read_channel_meta_in_dir(dir: &Path) -> (String, String) {
    let primary = dir.join(CHANNEL_DEF_FILENAME);
    if primary.is_file() {
        if let Ok(raw) = std::fs::read_to_string(&primary) {
            if let Some((yaml, _)) = split_frontmatter(&raw) {
                if let Ok(fm) = serde_yaml::from_str::<ChannelDefFrontmatter>(yaml) {
                    return (fm.name, fm.description);
                }
            }
        }
    }
    let legacy = dir.join(LEGACY_CHANNEL_DEF_FILENAME);
    if legacy.is_file() {
        if let Ok(raw) = std::fs::read_to_string(&legacy) {
            if let Ok(f) = serde_json::from_str::<LegacyChannelDefFile>(&raw) {
                return (f.name, f.description);
            }
        }
    }
    (String::new(), String::new())
}

pub fn load_channel_def(
    channels_root: &Path,
    handle: &QueueHandle,
) -> Result<Option<ChannelDef>, ChannelDefStoreError> {
    let dir = channel_dir(channels_root, handle);
    let primary = dir.join(CHANNEL_DEF_FILENAME);
    if primary.exists() {
        return load_from_markdown(&primary).map(Some);
    }
    let legacy = dir.join(LEGACY_CHANNEL_DEF_FILENAME);
    if legacy.exists() {
        return load_from_legacy_json(&legacy).map(Some);
    }
    Ok(None)
}

fn load_from_markdown(path: &Path) -> Result<ChannelDef, ChannelDefStoreError> {
    let raw = std::fs::read_to_string(path).map_err(|e| ChannelDefStoreError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let (yaml, _body) =
        split_frontmatter(&raw).ok_or_else(|| ChannelDefStoreError::MalformedFrontmatter {
            path: path.to_path_buf(),
            message: "missing `---` frontmatter delimiters".into(),
        })?;
    let fm: ChannelDefFrontmatter =
        serde_yaml::from_str(yaml).map_err(|e| ChannelDefStoreError::MalformedFrontmatter {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
    finalize(
        fm.handle,
        fm.name,
        fm.description,
        fm.created_at,
        fm.requires_stamp,
        fm.tombstoned,
        path,
    )
}

fn load_from_legacy_json(path: &Path) -> Result<ChannelDef, ChannelDefStoreError> {
    let raw = std::fs::read_to_string(path).map_err(|e| ChannelDefStoreError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let file: LegacyChannelDefFile =
        serde_json::from_str(&raw).map_err(|e| ChannelDefStoreError::MalformedJson {
            path: path.to_path_buf(),
            source: e,
        })?;
    finalize(
        file.handle,
        file.name,
        file.description,
        file.created_at,
        false,
        false,
        path,
    )
}

#[allow(clippy::too_many_arguments)]
fn finalize(
    handle: String,
    name: String,
    description: String,
    created_at: String,
    requires_stamp: bool,
    tombstoned: bool,
    path: &Path,
) -> Result<ChannelDef, ChannelDefStoreError> {
    let parsed_handle =
        QueueHandle::parse(&handle).map_err(|e| ChannelDefStoreError::InvalidHandle {
            handle: handle.clone(),
            source: e,
        })?;
    let created_at = DateTime::parse_from_rfc3339(&created_at)
        .map_err(|_| ChannelDefStoreError::InvalidTimestamp {
            value: created_at.clone(),
            path: path.to_path_buf(),
        })?
        .with_timezone(&Utc);
    Ok(
        ChannelDef::new(parsed_handle, name, description, created_at)
            .with_requires_stamp(requires_stamp)
            .with_tombstoned(tombstoned),
    )
}

pub fn save_channel_def(
    channels_root: &Path,
    def: &ChannelDef,
    overwrite: bool,
) -> Result<(), ChannelDefStoreError> {
    let dir = channel_dir(channels_root, &def.handle);
    std::fs::create_dir_all(&dir).map_err(|e| ChannelDefStoreError::Io {
        path: dir.clone(),
        source: e,
    })?;
    // Pre-create envelopes/ so capture into this channel finds the dir.
    let envelopes = dir.join("envelopes");
    std::fs::create_dir_all(&envelopes).map_err(|e| ChannelDefStoreError::Io {
        path: envelopes,
        source: e,
    })?;

    let path = channel_def_path(channels_root, &def.handle);
    let legacy_path = dir.join(LEGACY_CHANNEL_DEF_FILENAME);
    if (path.exists() || legacy_path.exists()) && !overwrite {
        return Err(ChannelDefStoreError::AlreadyExists(path));
    }

    let fm = ChannelDefFrontmatter {
        ty: CHANNEL_DEF_TYPE.to_string(),
        handle: def.handle.as_str().to_string(),
        name: def.name.clone(),
        description: def.description.clone(),
        created_at: def.created_at.to_rfc3339(),
        requires_stamp: def.requires_stamp,
        tombstoned: def.tombstoned,
    };
    let yaml =
        serde_yaml::to_string(&fm).map_err(|e| ChannelDefStoreError::MalformedFrontmatter {
            path: path.clone(),
            message: e.to_string(),
        })?;
    let body = DEFAULT_STUB_BODY
        .replace(
            "{NAME}",
            if def.name.is_empty() {
                def.handle.as_str()
            } else {
                &def.name
            },
        )
        .replace("{DESCRIPTION}", &def.description);
    let rendered = format!("---\n{yaml}---\n{body}");

    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, rendered).map_err(|e| ChannelDefStoreError::Io {
        path: tmp.clone(),
        source: e,
    })?;
    std::fs::rename(&tmp, &path).map_err(|e| ChannelDefStoreError::Io {
        path: path.clone(),
        source: e,
    })?;
    // Remove the legacy sidecar once the new shape is on disk — opportunistic
    // one-shot migration so old + new don't coexist after a save.
    if legacy_path.exists() {
        let _ = std::fs::remove_file(&legacy_path);
    }
    Ok(())
}

/// Remove a channel's entire directory tree (envelopes + def + nested
/// subchannels). Caller handles confirmation UX. NOT recursive into
/// substrate-private dirs — they're inside the channel and go with it.
pub fn delete_channel(
    channels_root: &Path,
    handle: &QueueHandle,
) -> Result<(), ChannelDefStoreError> {
    let dir = channel_dir(channels_root, handle);
    if !dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&dir).map_err(|e| ChannelDefStoreError::Io {
        path: dir,
        source: e,
    })?;
    Ok(())
}

/// Split a `---\n...\n---\n<body>` document. Returns `None` if the file
/// doesn't start with a frontmatter delimiter or has no closing one.
/// Duplicated from `contract_store` to keep modules decoupled; both
/// readers share the same wire shape.
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

    fn def(when: DateTime<Utc>) -> ChannelDef {
        ChannelDef::new(
            QueueHandle::parse("product:data:baux-commerciaux").unwrap(),
            "Baux commerciaux",
            "Cohort tracking",
            when,
        )
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("channels");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let d = def(when);
        save_channel_def(&root, &d, false).unwrap();
        let loaded = load_channel_def(&root, &d.handle).unwrap().unwrap();
        assert_eq!(loaded, d);
        let envelopes = channel_dir(&root, &d.handle).join("envelopes");
        assert!(envelopes.is_dir());
        // New shape on disk.
        assert!(channel_def_path(&root, &d.handle).is_file());
        assert_eq!(
            channel_def_path(&root, &d.handle).extension().unwrap(),
            "md"
        );
    }

    #[test]
    fn requires_stamp_roundtrips() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("channels");
        let when = Utc.with_ymd_and_hms(2026, 5, 21, 0, 0, 0).unwrap();
        let d = ChannelDef::new(
            QueueHandle::parse("assemblee_generale").unwrap(),
            "AG",
            "Assemblée générale",
            when,
        )
        .with_requires_stamp(true);
        save_channel_def(&root, &d, false).unwrap();
        let loaded = load_channel_def(&root, &d.handle).unwrap().unwrap();
        assert!(loaded.requires_stamp);
        assert_eq!(loaded, d);

        // Verify it actually appears in the rendered frontmatter (regression
        // guard against `skip_serializing_if` swallowing it).
        let raw = std::fs::read_to_string(channel_def_path(&root, &d.handle)).unwrap();
        assert!(raw.contains("requires_stamp: true"));
    }

    #[test]
    fn requires_stamp_default_false_when_absent_in_yaml() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("channels");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        // Save a default channel (requires_stamp = false) — should NOT
        // emit the field (skip_serializing_if = is_false).
        let d = def(when);
        save_channel_def(&root, &d, false).unwrap();
        let raw = std::fs::read_to_string(channel_def_path(&root, &d.handle)).unwrap();
        assert!(
            !raw.contains("requires_stamp"),
            "default-false field should be omitted from frontmatter"
        );
        // And it loads back as false.
        let loaded = load_channel_def(&root, &d.handle).unwrap().unwrap();
        assert!(!loaded.requires_stamp);
    }

    #[test]
    fn save_refuses_to_overwrite_unless_explicit() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("channels");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let d = def(when);
        save_channel_def(&root, &d, false).unwrap();
        let r = save_channel_def(&root, &d, false);
        assert!(matches!(r, Err(ChannelDefStoreError::AlreadyExists(_))));
        save_channel_def(&root, &d, true).unwrap();
    }

    #[test]
    fn channel_dir_maps_handle_segments() {
        let root = Path::new("/foo");
        let h = QueueHandle::parse("com:landing-page").unwrap();
        let d = channel_dir(root, &h);
        assert!(d.ends_with("com/landing-page"));
    }

    #[test]
    fn delete_is_idempotent_for_missing_channel() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("channels");
        let h = QueueHandle::parse("does:not:exist").unwrap();
        delete_channel(&root, &h).unwrap();
    }

    #[test]
    fn loads_legacy_dotchanneldef_json() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("channels");
        let h = QueueHandle::parse("legacy:one").unwrap();
        let chdir = channel_dir(&root, &h);
        std::fs::create_dir_all(&chdir).unwrap();
        let legacy_json = r#"{
  "version": 1,
  "handle": "legacy:one",
  "name": "Legacy",
  "description": "",
  "created_at": "2026-05-12T00:00:00+00:00"
}"#;
        std::fs::write(chdir.join(LEGACY_CHANNEL_DEF_FILENAME), legacy_json).unwrap();
        let loaded = load_channel_def(&root, &h).unwrap().unwrap();
        assert_eq!(loaded.handle.as_str(), "legacy:one");
        assert_eq!(loaded.name, "Legacy");
    }

    #[test]
    fn save_removes_legacy_sidecar() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("channels");
        let h = QueueHandle::parse("mig:rate").unwrap();
        let chdir = channel_dir(&root, &h);
        std::fs::create_dir_all(&chdir).unwrap();
        let legacy_json = r#"{"version":1,"handle":"mig:rate","name":"Old","description":"","created_at":"2026-05-12T00:00:00+00:00"}"#;
        std::fs::write(chdir.join(LEGACY_CHANNEL_DEF_FILENAME), legacy_json).unwrap();
        let when = Utc.with_ymd_and_hms(2026, 5, 13, 0, 0, 0).unwrap();
        let d = ChannelDef::new(h.clone(), "New", "", when);
        save_channel_def(&root, &d, true).unwrap();
        assert!(channel_def_path(&root, &h).is_file());
        assert!(!chdir.join(LEGACY_CHANNEL_DEF_FILENAME).exists());
    }

    #[test]
    fn channel_def_exists_sees_both_shapes() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("channels");
        let h = QueueHandle::parse("exists:check").unwrap();
        assert!(!channel_def_exists(&root, &h));
        let chdir = channel_dir(&root, &h);
        std::fs::create_dir_all(&chdir).unwrap();
        std::fs::write(chdir.join(LEGACY_CHANNEL_DEF_FILENAME), "{}").unwrap();
        assert!(channel_def_exists(&root, &h));
    }
}
