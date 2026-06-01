//! Persistent relay state (`~/.secretariat/relay-state.json`).
//!
//! The federation HTTP client that polled and pushed envelopes over a
//! self-hosted relay was removed in the git-native teardown (cut A). What
//! remains is the on-disk state model that other flows still read:
//!
//! - `endpoint` — the relay base URL a contact / org channel is reachable at
//! - `registered` — whether we've completed the one-time registration
//!   (used by the invite flow to pick a default endpoint)
//! - `token` + `token_expires_at` — kept on the record for forward
//!   compatibility; nothing writes them today
//! - `queue_cursors` — per-`(owner, handle)` ingest cursors
//!
//! State is persisted at `~/.secretariat/relay-state.json` (atomic write,
//! mode 0600 on Unix). Invite creation/claim and the Settings → Relay pane
//! are the remaining readers/writers.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::domain::{Did, QueueHandle};

/// v2 (2026-05-19): per-`(owner, handle)` cursors replace the single
/// per-endpoint cursor. Channels (org-scoped or self-owned) all subscribe
/// uniformly via `(owner, handle)`. No migration code — nothing was in
/// production.
const STATE_VERSION: u32 = 2;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum RelayStateError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("relay state json malformed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("relay state has unsupported version {0} (this build understands {STATE_VERSION})")]
    UnsupportedVersion(u32),
}

// ---------------------------------------------------------------------------
// Persistent state
// ---------------------------------------------------------------------------

/// Per-relay session state. One entry per relay we talk to. Cursors live
/// on `queue_cursors` (per `(owner, handle)`) — see [`QueueCursor`]. The
/// legacy single-`cursor` field is gone; every channel subscription
/// (org-scoped or self-owned) uses the per-`(owner, handle)` axis.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RelayEntry {
    pub endpoint: String,
    #[serde(default)]
    pub registered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_expires_at: Option<DateTime<Utc>>,
    /// One cursor per queue this principal polls at this relay. Read
    /// via [`RelayEntry::cursor_for`]; write via
    /// [`RelayEntry::set_cursor_for`]. Cursor is `0` for queues the
    /// daemon has never polled (then `since(0)` returns the full
    /// stream on first sync).
    #[serde(default)]
    pub queue_cursors: Vec<QueueCursor>,
}

/// Per-`(owner, handle)` cursor — the highest envelope id we've ingested
/// from this queue at this relay. Stored as a flat list rather than a
/// HashMap because JSON map keys must be strings; the linear scan is
/// fine at v0.8 scale (handfuls of queues per principal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueCursor {
    pub owner: Did,
    pub handle: QueueHandle,
    pub cursor: u64,
}

impl RelayEntry {
    /// Highest envelope id we've ingested from `(owner, handle)` at this
    /// relay. Returns `0` for queues we've never polled — first poll
    /// then asks `since(0)` and ingests everything.
    pub fn cursor_for(&self, owner: &Did, handle: &QueueHandle) -> u64 {
        self.queue_cursors
            .iter()
            .find(|q| &q.owner == owner && &q.handle == handle)
            .map(|q| q.cursor)
            .unwrap_or(0)
    }

    /// Set the cursor for `(owner, handle)`. Inserts a new entry if this
    /// queue isn't tracked yet.
    pub fn set_cursor_for(&mut self, owner: &Did, handle: &QueueHandle, cursor: u64) {
        if let Some(q) = self
            .queue_cursors
            .iter_mut()
            .find(|q| &q.owner == owner && &q.handle == handle)
        {
            q.cursor = cursor;
        } else {
            self.queue_cursors.push(QueueCursor {
                owner: owner.clone(),
                handle: handle.clone(),
                cursor,
            });
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StateFile {
    version: u32,
    relays: Vec<RelayEntry>,
}

impl Default for StateFile {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            relays: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RelayState {
    relays: Vec<RelayEntry>,
}

impl RelayState {
    pub fn load(path: &Path) -> Result<Self, RelayStateError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path).map_err(|e| RelayStateError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let parsed: StateFile = serde_json::from_str(&raw)?;
        if parsed.version != STATE_VERSION {
            return Err(RelayStateError::UnsupportedVersion(parsed.version));
        }
        Ok(Self {
            relays: parsed.relays,
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), RelayStateError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| RelayStateError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let snapshot = StateFile {
            version: STATE_VERSION,
            relays: self.relays.clone(),
        };
        let pretty = serde_json::to_string_pretty(&snapshot)?;

        let parent = path.parent().unwrap_or(Path::new("."));
        let mut tmp = NamedTempFile::new_in(parent).map_err(|e| RelayStateError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
        use std::io::Write as _;
        tmp.write_all(pretty.as_bytes())
            .and_then(|_| tmp.write_all(b"\n"))
            .map_err(|e| RelayStateError::Io {
                path: tmp.path().to_path_buf(),
                source: e,
            })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(tmp.path(), perms).map_err(|e| RelayStateError::Io {
                path: tmp.path().to_path_buf(),
                source: e,
            })?;
        }

        tmp.persist(path).map_err(|e| RelayStateError::Io {
            path: path.to_path_buf(),
            source: e.error,
        })?;
        Ok(())
    }

    pub fn entry_mut(&mut self, endpoint: &str) -> &mut RelayEntry {
        if let Some(idx) = self.relays.iter().position(|r| r.endpoint == endpoint) {
            &mut self.relays[idx]
        } else {
            self.relays.push(RelayEntry {
                endpoint: endpoint.to_string(),
                ..Default::default()
            });
            self.relays.last_mut().unwrap()
        }
    }

    pub fn entry(&self, endpoint: &str) -> Option<&RelayEntry> {
        self.relays.iter().find(|r| r.endpoint == endpoint)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RelayEntry> {
        self.relays.iter()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn relay_state_load_missing_returns_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("relay-state.json");
        let state = RelayState::load(&path).unwrap();
        assert_eq!(state.iter().count(), 0);
    }

    #[test]
    fn relay_state_save_then_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("relay-state.json");

        let owner = crate::Did::from_ed25519_public_key(&[1u8; 32]);
        let handle = crate::domain::QueueHandle::parse("inbox:default").unwrap();

        let mut state = RelayState::default();
        let entry = state.entry_mut("wss://relay.rafa.equanimi.tech");
        entry.registered = true;
        entry.set_cursor_for(&owner, &handle, 42);
        entry.token = Some("abc".to_string());
        state.save(&path).unwrap();

        let reloaded = RelayState::load(&path).unwrap();
        let e = reloaded.entry("wss://relay.rafa.equanimi.tech").unwrap();
        assert!(e.registered);
        assert_eq!(e.cursor_for(&owner, &handle), 42);
        assert_eq!(e.token.as_deref(), Some("abc"));
    }

    #[test]
    fn relay_state_save_writes_0600() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("relay-state.json");
        let mut state = RelayState::default();
        state.entry_mut("wss://relay.example.com").registered = true;
        state.save(&path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn relay_state_entry_mut_creates_and_updates() {
        let owner = crate::Did::from_ed25519_public_key(&[1u8; 32]);
        let handle = crate::domain::QueueHandle::parse("inbox:default").unwrap();

        let mut state = RelayState::default();
        {
            let e = state.entry_mut("wss://x");
            e.set_cursor_for(&owner, &handle, 1);
        }
        {
            let e = state.entry_mut("wss://x");
            assert_eq!(e.cursor_for(&owner, &handle), 1);
            e.set_cursor_for(&owner, &handle, 2);
        }
        let e = state.entry("wss://x").unwrap();
        assert_eq!(e.cursor_for(&owner, &handle), 2);
        assert_eq!(state.iter().count(), 1);
    }

    #[test]
    fn per_queue_cursors_independent_within_one_entry() {
        let did1 = crate::Did::from_ed25519_public_key(&[1u8; 32]);
        let did2 = crate::Did::from_ed25519_public_key(&[2u8; 32]);
        let dm = crate::domain::QueueHandle::parse("inbox:default").unwrap();
        let dev = crate::domain::QueueHandle::parse("dev:secretariat").unwrap();

        let mut state = RelayState::default();
        let e = state.entry_mut("wss://relay.example");
        e.set_cursor_for(&did1, &dm, 10);
        e.set_cursor_for(&did1, &dev, 50);
        e.set_cursor_for(&did2, &dev, 99);

        assert_eq!(e.cursor_for(&did1, &dm), 10);
        assert_eq!(e.cursor_for(&did1, &dev), 50);
        assert_eq!(e.cursor_for(&did2, &dev), 99);
        // Unknown (owner, handle) returns 0 — fresh subscription state.
        assert_eq!(e.cursor_for(&did2, &dm), 0);
        assert_eq!(e.queue_cursors.len(), 3);
    }

    #[test]
    fn relay_state_unsupported_version_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("relay-state.json");
        std::fs::write(&path, r#"{"version": 999, "relays": []}"#).unwrap();
        assert!(matches!(
            RelayState::load(&path),
            Err(RelayStateError::UnsupportedVersion(999))
        ));
    }
}
