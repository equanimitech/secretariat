//! Per-channel metadata storage at `<channel_dir>/.channelDef`.
//!
//! JSON shape (v1):
//!
//! ```json
//! {
//!   "version": 1,
//!   "handle": "channel:product:data:baux-commerciaux",
//!   "name": "Baux commerciaux — produit data",
//!   "description": "Suivi du module BC et de ses cohortes.",
//!   "created_at": "2026-05-12T03:00:00Z"
//! }
//! ```

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{ChannelDef, QueueHandle, QueueHandleError};

const CURRENT_VERSION: u32 = 1;
pub const CHANNEL_DEF_FILENAME: &str = ".channelDef";

#[derive(Debug, Error)]
pub enum ChannelDefStoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed json at {path}: {source}")]
    MalformedJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("unsupported .channelDef version {version} at {path}")]
    UnsupportedVersion { version: u32, path: PathBuf },
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

#[derive(Debug, Serialize, Deserialize)]
struct ChannelDefFile {
    version: u32,
    handle: String,
    name: String,
    description: String,
    created_at: String,
}

/// Resolve the on-disk directory for a channel under a given channels root.
/// `channel:foo:bar` → `<channels_root>/foo/bar/`.
pub fn channel_dir(channels_root: &Path, handle: &QueueHandle) -> PathBuf {
    let mut dir = channels_root.to_path_buf();
    for seg in handle.segments().iter().skip(1) {
        dir.push(seg);
    }
    dir
}

pub fn channel_def_path(channels_root: &Path, handle: &QueueHandle) -> PathBuf {
    channel_dir(channels_root, handle).join(CHANNEL_DEF_FILENAME)
}

pub fn load_channel_def(
    channels_root: &Path,
    handle: &QueueHandle,
) -> Result<Option<ChannelDef>, ChannelDefStoreError> {
    let path = channel_def_path(channels_root, handle);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| ChannelDefStoreError::Io {
        path: path.clone(),
        source: e,
    })?;
    let file: ChannelDefFile =
        serde_json::from_str(&raw).map_err(|e| ChannelDefStoreError::MalformedJson {
            path: path.clone(),
            source: e,
        })?;
    if file.version != CURRENT_VERSION {
        return Err(ChannelDefStoreError::UnsupportedVersion {
            version: file.version,
            path,
        });
    }
    let parsed_handle =
        QueueHandle::parse(&file.handle).map_err(|e| ChannelDefStoreError::InvalidHandle {
            handle: file.handle.clone(),
            source: e,
        })?;
    let created_at = DateTime::parse_from_rfc3339(&file.created_at)
        .map_err(|_| ChannelDefStoreError::InvalidTimestamp {
            value: file.created_at.clone(),
            path,
        })?
        .with_timezone(&Utc);
    Ok(Some(ChannelDef::new(
        parsed_handle,
        file.name,
        file.description,
        created_at,
    )))
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
    if path.exists() && !overwrite {
        return Err(ChannelDefStoreError::AlreadyExists(path));
    }

    let file = ChannelDefFile {
        version: CURRENT_VERSION,
        handle: def.handle.as_str().to_string(),
        name: def.name.clone(),
        description: def.description.clone(),
        created_at: def.created_at.to_rfc3339(),
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| {
        ChannelDefStoreError::MalformedJson {
            path: path.clone(),
            source: e,
        }
    })?;
    let tmp = path.with_extension("channelDef.tmp");
    std::fs::write(&tmp, json).map_err(|e| ChannelDefStoreError::Io {
        path: tmp.clone(),
        source: e,
    })?;
    std::fs::rename(&tmp, &path).map_err(|e| ChannelDefStoreError::Io {
        path: path.clone(),
        source: e,
    })?;
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
        // Idempotent — nothing to remove.
        return Ok(());
    }
    std::fs::remove_dir_all(&dir).map_err(|e| ChannelDefStoreError::Io {
        path: dir,
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn def(when: DateTime<Utc>) -> ChannelDef {
        ChannelDef::new(
            QueueHandle::parse("channel:product:data:baux-commerciaux").unwrap(),
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
        // Envelopes dir created.
        let envelopes = channel_dir(&root, &d.handle).join("envelopes");
        assert!(envelopes.is_dir());
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
        let h = QueueHandle::parse("channel:com:landing-page").unwrap();
        let d = channel_dir(root, &h);
        assert!(d.ends_with("com/landing-page"));
    }

    #[test]
    fn delete_is_idempotent_for_missing_channel() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("channels");
        let h = QueueHandle::parse("channel:does:not:exist").unwrap();
        delete_channel(&root, &h).unwrap();
    }
}
