//! On-disk persistence for relay state.
//!
//! One file: `<data_dir>/state.json`. Atomic write via tempfile rename.
//! Mode `0600` on Unix — the file holds tenant pubkeys (not secret, but
//! we keep precedent with the rest of `~/.secretariat`).
//!
//! v0 design choices (subject to revisit when scale / write rate justifies):
//!
//! - Single JSON file (vs. per-tenant directory or sqlite). At v0 scale —
//!   handful of tenants, dozens of operations per day — fsync of one file
//!   is trivial. SQLite is the upgrade path when contention or write
//!   amplification matters.
//! - Synchronous save after each mutation. Saves are fast (the whole file
//!   is small). No background flushing; correctness is local.
//! - Schema versioned (`version: 1`); a future bump triggers a clear
//!   error rather than silent data loss.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use secretariat_core::domain::QueueHandle;
use secretariat_core::Did;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;
use tracing::info;

use crate::queue::TenantQueue;
use crate::state::Invite;

/// v3 (2026-05-19): single queue index axis keyed by `(owner_did, handle)`.
/// Supersedes the legacy per-DID `queues:` field (v1) and the transient v2
/// `channels:` naming. DMs ride as `(peer, "inbox:default")` — same primitive.
/// No migration code — nothing was in production.
pub const STATE_VERSION: u32 = 3;
pub const STATE_FILENAME: &str = "state.json";

#[derive(Debug, Error)]
pub enum PersistError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("state file has unsupported version {0} (this build understands {STATE_VERSION})")]
    UnsupportedVersion(u32),
    #[error("state file references invalid tenant: {0}")]
    InvalidTenant(String),
}

/// On-disk wire shape. `tenants` uses the [`PersistedTenant`] form
/// (registered in `state.rs`) so the in-memory `VerifyingKey` doesn't need
/// to be serializable directly.
///
/// `queues` is the single queue index axis — keyed by `(owner_did, handle)`
/// per the queues-as-primitive substrate model. Serialized as a list rather
/// than a map because JSON map keys must be strings; flattening to a list
/// avoids ad-hoc tuple-key encoding.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct StateFile {
    pub version: u32,
    pub tenants: Vec<super::state::PersistedTenant>,
    pub invites: HashMap<String, Invite>,
    pub queues: Vec<PersistedQueue>,
}

/// One entry in the queue index — a per-`(owner, handle)` queue.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PersistedQueue {
    pub owner: Did,
    pub handle: QueueHandle,
    pub queue: TenantQueue,
}

impl Default for StateFile {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            tenants: Vec::new(),
            invites: HashMap::new(),
            queues: Vec::new(),
        }
    }
}

/// Load `state.json` from `data_dir`. Missing file = empty state (fresh
/// deploy).
pub(crate) fn load(data_dir: &Path) -> Result<StateFile, PersistError> {
    let path = data_dir.join(STATE_FILENAME);
    if !path.exists() {
        return Ok(StateFile::default());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| PersistError::Io {
        path: path.clone(),
        source: e,
    })?;
    let parsed: StateFile = serde_json::from_str(&raw)?;
    if parsed.version != STATE_VERSION {
        return Err(PersistError::UnsupportedVersion(parsed.version));
    }
    info!(
        path = %path.display(),
        tenants = parsed.tenants.len(),
        invites = parsed.invites.len(),
        queues = parsed.queues.len(),
        "loaded relay state"
    );
    Ok(parsed)
}

/// Atomic write of `state.json` into `data_dir`.
pub(crate) fn save(data_dir: &Path, file: &StateFile) -> Result<(), PersistError> {
    std::fs::create_dir_all(data_dir).map_err(|e| PersistError::Io {
        path: data_dir.to_path_buf(),
        source: e,
    })?;
    let path = data_dir.join(STATE_FILENAME);

    let pretty = serde_json::to_string_pretty(file)?;

    let mut tmp = NamedTempFile::new_in(data_dir).map_err(|e| PersistError::Io {
        path: data_dir.to_path_buf(),
        source: e,
    })?;
    tmp.write_all(pretty.as_bytes())
        .and_then(|_| tmp.write_all(b"\n"))
        .map_err(|e| PersistError::Io {
            path: tmp.path().to_path_buf(),
            source: e,
        })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(tmp.path(), perms).map_err(|e| PersistError::Io {
            path: tmp.path().to_path_buf(),
            source: e,
        })?;
    }

    tmp.persist(&path).map_err(|e| PersistError::Io {
        path: path.clone(),
        source: e.error,
    })?;
    Ok(())
}
