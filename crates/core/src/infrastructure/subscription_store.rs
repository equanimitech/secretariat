//! Subscription persistence — the receiver-side sync primitive.
//!
//! A subscription declares "this principal wants to sync queue
//! `(owner_did, handle)` from `relay_endpoint`." The daemon enumerates
//! subscriptions on every `sync_now` tick and polls each via
//! [`crate::infrastructure::transport::RelayClient::poll`], filing inbound
//! envelopes through [`crate::application::sync::file_inbound`] (existing
//! AliasMap → queue_dir machinery routes to the right channel dir).
//!
//! ## Substrate framing — sync, not joining
//!
//! Per the v0.8 channel-relay-sequencer pitch, the substrate's model is
//! *know the URI, subscribe, sync*. There is no "join the org" ceremony,
//! no `rosterUpdate` publish on subscribe, no implicit roster membership.
//! A subscription is unilateral: the subscriber's machine declares intent;
//! the owner's relay either serves bytes (today, ungated) or doesn't
//! (when the roster gate lands in element 4).
//!
//! DMs are subscriptions like any other — `(self_did, "inbox:default",
//! self_relay)` is a queue the daemon polls, same primitive as
//! `(themia_did, "dev:secretariat", relay.themia.pro)`. The receiver-side
//! code path is uniform.
//!
//! ## Wire shape
//!
//! ```json
//! {
//!   "version": 1,
//!   "queues": [
//!     {
//!       "owner_did": "did:web:themia.pro",
//!       "handle": "dev:secretariat",
//!       "relay_endpoint": "https://relay.themia.pro",
//!       "subscribed_at": "2026-05-19T12:34:56Z"
//!     }
//!   ]
//! }
//! ```
//!
//! Persisted at `<root>/subscriptions.json` (atomic write, mode 0600 on
//! Unix). Cursors are NOT here — they live in `relay-state.json` alongside
//! tokens and registration markers (runtime state vs user intent).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::domain::{Did, QueueHandle, RelayEndpoint};

const STORE_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum SubscriptionStoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("subscriptions file has unsupported version {0} (this build understands {STORE_VERSION})")]
    UnsupportedVersion(u32),
    #[error("duplicate subscription for ({owner}, {handle})")]
    Duplicate { owner: String, handle: String },
}

/// One queue the principal has chosen to sync.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Subscription {
    pub owner_did: Did,
    pub handle: QueueHandle,
    pub relay_endpoint: RelayEndpoint,
    pub subscribed_at: DateTime<Utc>,
}

impl Subscription {
    pub fn new(
        owner_did: Did,
        handle: QueueHandle,
        relay_endpoint: RelayEndpoint,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            owner_did,
            handle,
            relay_endpoint,
            subscribed_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoreFile {
    version: u32,
    queues: Vec<Subscription>,
}

impl Default for StoreFile {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            queues: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SubscriptionStore {
    queues: Vec<Subscription>,
}

impl SubscriptionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: &Path) -> Result<Self, SubscriptionStoreError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path).map_err(|e| SubscriptionStoreError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let parsed: StoreFile = serde_json::from_str(&raw)?;
        if parsed.version != STORE_VERSION {
            return Err(SubscriptionStoreError::UnsupportedVersion(parsed.version));
        }
        Ok(Self {
            queues: parsed.queues,
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), SubscriptionStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| SubscriptionStoreError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        let snapshot = StoreFile {
            version: STORE_VERSION,
            queues: self.queues.clone(),
        };
        let pretty = serde_json::to_string_pretty(&snapshot)?;
        let parent = path.parent().unwrap_or(Path::new("."));
        let mut tmp = NamedTempFile::new_in(parent).map_err(|e| SubscriptionStoreError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
        tmp.write_all(pretty.as_bytes())
            .and_then(|_| tmp.write_all(b"\n"))
            .map_err(|e| SubscriptionStoreError::Io {
                path: tmp.path().to_path_buf(),
                source: e,
            })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(tmp.path(), perms).map_err(|e| SubscriptionStoreError::Io {
                path: tmp.path().to_path_buf(),
                source: e,
            })?;
        }
        tmp.persist(path).map_err(|e| SubscriptionStoreError::Io {
            path: path.to_path_buf(),
            source: e.error,
        })?;
        Ok(())
    }

    /// Append a new subscription. Idempotency: a `(owner, handle)` pair
    /// already present returns `Duplicate` without mutating — callers
    /// treat this as "already subscribed, fine."
    pub fn add(&mut self, sub: Subscription) -> Result<(), SubscriptionStoreError> {
        if self.find(&sub.owner_did, &sub.handle).is_some() {
            return Err(SubscriptionStoreError::Duplicate {
                owner: sub.owner_did.as_str().to_string(),
                handle: sub.handle.as_str().to_string(),
            });
        }
        self.queues.push(sub);
        Ok(())
    }

    pub fn remove(&mut self, owner: &Did, handle: &QueueHandle) -> bool {
        let before = self.queues.len();
        self.queues
            .retain(|s| !(&s.owner_did == owner && &s.handle == handle));
        before != self.queues.len()
    }

    pub fn find(&self, owner: &Did, handle: &QueueHandle) -> Option<&Subscription> {
        self.queues
            .iter()
            .find(|s| &s.owner_did == owner && &s.handle == handle)
    }

    /// First subscription whose owner matches — useful for `send_envelope`
    /// to resolve a relay endpoint when the recipient isn't in contacts.
    /// Any queue owned by `owner` shares the same `relay_endpoint`.
    pub fn find_by_owner(&self, owner: &Did) -> Option<&Subscription> {
        self.queues.iter().find(|s| &s.owner_did == owner)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Subscription> {
        self.queues.iter()
    }

    pub fn len(&self) -> usize {
        self.queues.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queues.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn did(seed: u8) -> Did {
        Did::from_ed25519_public_key(&[seed; 32])
    }

    fn handle(s: &str) -> QueueHandle {
        QueueHandle::parse(s).unwrap()
    }

    fn endpoint(s: &str) -> RelayEndpoint {
        RelayEndpoint::parse(s).unwrap()
    }

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    #[test]
    fn empty_load_when_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("subscriptions.json");
        let store = SubscriptionStore::load(&path).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn add_then_find() {
        let mut s = SubscriptionStore::new();
        let sub = Subscription::new(
            did(1),
            handle("dev:secretariat"),
            endpoint("https://relay.example"),
            now(),
        );
        s.add(sub.clone()).unwrap();
        assert_eq!(s.len(), 1);
        let found = s.find(&did(1), &handle("dev:secretariat")).unwrap();
        assert_eq!(found, &sub);
    }

    #[test]
    fn add_duplicate_rejected() {
        let mut s = SubscriptionStore::new();
        let sub = Subscription::new(
            did(1),
            handle("dev:secretariat"),
            endpoint("https://relay.example"),
            now(),
        );
        s.add(sub.clone()).unwrap();
        let r = s.add(sub);
        assert!(matches!(r, Err(SubscriptionStoreError::Duplicate { .. })));
    }

    #[test]
    fn distinct_keys_coexist() {
        let mut s = SubscriptionStore::new();
        s.add(Subscription::new(
            did(1),
            handle("dev:secretariat"),
            endpoint("https://relay.example"),
            now(),
        ))
        .unwrap();
        s.add(Subscription::new(
            did(1),
            handle("book"),
            endpoint("https://relay.example"),
            now(),
        ))
        .unwrap();
        s.add(Subscription::new(
            did(2),
            handle("dev:secretariat"),
            endpoint("https://relay.other"),
            now(),
        ))
        .unwrap();
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn remove_returns_whether_found() {
        let mut s = SubscriptionStore::new();
        let sub = Subscription::new(
            did(1),
            handle("dev:secretariat"),
            endpoint("https://relay.example"),
            now(),
        );
        s.add(sub).unwrap();
        assert!(s.remove(&did(1), &handle("dev:secretariat")));
        assert!(!s.remove(&did(1), &handle("dev:secretariat")));
        assert!(s.is_empty());
    }

    #[test]
    fn find_by_owner_returns_first() {
        let mut s = SubscriptionStore::new();
        s.add(Subscription::new(
            did(1),
            handle("dev:secretariat"),
            endpoint("https://relay.example"),
            now(),
        ))
        .unwrap();
        s.add(Subscription::new(
            did(1),
            handle("book"),
            endpoint("https://relay.example"),
            now(),
        ))
        .unwrap();
        let f = s.find_by_owner(&did(1)).unwrap();
        assert_eq!(f.relay_endpoint.as_str(), "https://relay.example");
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("subscriptions.json");

        let mut s = SubscriptionStore::new();
        s.add(Subscription::new(
            did(1),
            handle("dev:secretariat"),
            endpoint("https://relay.example"),
            now(),
        ))
        .unwrap();
        s.save(&path).unwrap();

        let loaded = SubscriptionStore::load(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        let f = loaded.find(&did(1), &handle("dev:secretariat")).unwrap();
        assert_eq!(f.relay_endpoint.as_str(), "https://relay.example");
    }

    #[test]
    fn unsupported_version_rejected() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("subscriptions.json");
        fs::write(&path, r#"{"version": 99, "queues": []}"#).unwrap();
        let r = SubscriptionStore::load(&path);
        assert!(matches!(
            r,
            Err(SubscriptionStoreError::UnsupportedVersion(99))
        ));
    }
}
