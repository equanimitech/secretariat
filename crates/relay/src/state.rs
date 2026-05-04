//! Server-wide state held in axum's `State` extractor.
//!
//! When a `data_dir` is configured (see [`Config`]), every mutation triggers
//! an atomic save to `<data_dir>/state.json`. On startup, [`AppState::load`]
//! restores the registry, queues, and invites from that file. The relay
//! survives Railway redeploys without losing tenants or queued envelopes.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use ed25519_dalek::VerifyingKey;
use secretariat_core::codec::{decode_ed25519_multibase, encode_ed25519_multibase};
use secretariat_core::Did;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::auth::AuthState;
use crate::config::Config;
use crate::persist;
use crate::queue::TenantQueue;

/// A registered tenant.
#[derive(Debug, Clone)]
pub struct RegisteredTenant {
    pub did: Did,
    pub pubkey: VerifyingKey,
    pub registered_at: DateTime<Utc>,
}

/// On-disk wire form (VerifyingKey doesn't `serde` directly).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistedTenant {
    pub did: Did,
    pub pubkey_multibase: String,
    pub registered_at: DateTime<Utc>,
}

impl From<&RegisteredTenant> for PersistedTenant {
    fn from(t: &RegisteredTenant) -> Self {
        Self {
            did: t.did.clone(),
            pubkey_multibase: encode_ed25519_multibase(&t.pubkey.to_bytes()),
            registered_at: t.registered_at,
        }
    }
}

impl TryFrom<PersistedTenant> for RegisteredTenant {
    type Error = String;
    fn try_from(p: PersistedTenant) -> Result<Self, Self::Error> {
        let bytes = decode_ed25519_multibase(&p.pubkey_multibase)
            .map_err(|e| format!("malformed pubkey_multibase: {e}"))?;
        let pubkey = VerifyingKey::from_bytes(&bytes)
            .map_err(|e| format!("invalid ed25519 pubkey: {e}"))?;
        Ok(Self {
            did: p.did,
            pubkey,
            registered_at: p.registered_at,
        })
    }
}

/// A pending invite. Created by an inviter, claimed by exactly one peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invite {
    pub token: String,
    pub inviter_did: Did,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub purpose: Option<String>,
    pub claimed_by: Option<Did>,
    pub claimed_at: Option<DateTime<Utc>>,
}

#[derive(Default)]
pub struct AppState {
    pub config: Config,
    pub auth: AuthState,
    registry: RwLock<HashMap<Did, RegisteredTenant>>,
    queues: RwLock<HashMap<Did, TenantQueue>>,
    invites: RwLock<HashMap<String, Invite>>,
}

impl AppState {
    /// Snapshot the in-memory state into the on-disk wire shape and persist
    /// to `<data_dir>/state.json`. No-op when no `data_dir` is configured.
    fn save_snapshot(&self) {
        let Some(dir) = &self.config.data_dir else {
            return;
        };
        let registry = self.registry.read().unwrap();
        let queues = self.queues.read().unwrap();
        let invites = self.invites.read().unwrap();
        let snapshot = persist::StateFile {
            version: 1,
            tenants: registry.values().map(PersistedTenant::from).collect(),
            queues: queues
                .iter()
                .map(|(d, q)| (d.clone(), clone_queue(q)))
                .collect(),
            invites: invites.clone(),
        };
        if let Err(e) = persist::save(dir, &snapshot) {
            warn!(error = %e, dir = %dir.display(), "could not persist relay state");
        }
    }
}

/// `TenantQueue` doesn't derive `Clone` because the underlying VecDeque
/// would clone all envelope bodies — wasteful for normal use. For the
/// persistence path we *do* need a snapshot copy, so we provide a
/// purpose-built helper here.
fn clone_queue(q: &TenantQueue) -> TenantQueue {
    serde_json::from_str(&serde_json::to_string(q).unwrap()).unwrap()
}

impl AppState {
    pub fn new(config: Config) -> Arc<Self> {
        Arc::new(Self {
            config,
            auth: AuthState::new(),
            registry: RwLock::new(HashMap::new()),
            queues: RwLock::new(HashMap::new()),
            invites: RwLock::new(HashMap::new()),
        })
    }

    /// Build state from a fresh `Config` and rehydrate registry / queues /
    /// invites from `data_dir/state.json` if present.
    pub fn load(config: Config) -> Result<Arc<Self>, persist::PersistError> {
        let state = Self::new(config);
        let Some(dir) = &state.config.data_dir else {
            return Ok(state);
        };

        let snapshot = persist::load(dir)?;
        let mut registry = state.registry.write().unwrap();
        for tenant_wire in snapshot.tenants {
            let label = tenant_wire.did.as_str().to_string();
            let t: RegisteredTenant = tenant_wire
                .try_into()
                .map_err(persist::PersistError::InvalidTenant)?;
            let _ = label; // already in t.did; kept above for the error label
            registry.insert(t.did.clone(), t);
        }
        drop(registry);

        let mut queues = state.queues.write().unwrap();
        for (did, queue) in snapshot.queues {
            queues.insert(did, queue);
        }
        drop(queues);

        let mut invites = state.invites.write().unwrap();
        for (token, invite) in snapshot.invites {
            invites.insert(token, invite);
        }
        drop(invites);

        info!(
            dir = %state.config.data_dir.as_ref().unwrap().display(),
            tenants = state.registry.read().unwrap().len(),
            queues = state.queues.read().unwrap().len(),
            invites = state.invites.read().unwrap().len(),
            "rehydrated relay state"
        );
        Ok(state)
    }

    pub fn is_registered(&self, did: &Did) -> bool {
        self.registry.read().unwrap().contains_key(did)
    }

    pub fn register(
        &self,
        did: Did,
        pubkey: VerifyingKey,
        now: DateTime<Utc>,
    ) -> RegisteredTenant {
        let tenant = RegisteredTenant {
            did: did.clone(),
            pubkey,
            registered_at: now,
        };
        self.registry.write().unwrap().insert(did, tenant.clone());
        self.save_snapshot();
        tenant
    }

    pub fn pubkey_for(&self, did: &Did) -> Option<VerifyingKey> {
        self.registry.read().unwrap().get(did).map(|t| t.pubkey)
    }

    pub fn enqueue(
        &self,
        recipient: Did,
        body: Vec<u8>,
        content_type: String,
        sender_did: Option<Did>,
        now: DateTime<Utc>,
    ) -> u64 {
        let id = {
            let mut q = self.queues.write().unwrap();
            let queue = q.entry(recipient).or_default();
            queue.push(body, content_type, sender_did, now)
        };
        self.save_snapshot();
        id
    }

    pub fn since(&self, recipient: &Did, after: u64) -> Vec<crate::queue::QueuedEnvelope> {
        self.queues
            .read()
            .unwrap()
            .get(recipient)
            .map(|q| q.since(after))
            .unwrap_or_default()
    }

    pub fn registered_count(&self) -> usize {
        self.registry.read().unwrap().len()
    }

    pub fn queue_lengths(&self) -> Vec<(Did, usize)> {
        self.queues
            .read()
            .unwrap()
            .iter()
            .map(|(d, q)| (d.clone(), q.len()))
            .collect()
    }

    /// Drop entries older than `cutoff` from every per-tenant queue.
    /// Returns total entries pruned. Run periodically by the daemon's
    /// prune loop; see `crates/relay/src/main.rs::spawn_prune_loop`.
    pub fn prune_all(&self, cutoff: DateTime<Utc>) -> usize {
        let total = {
            let mut total = 0;
            for queue in self.queues.write().unwrap().values_mut() {
                total += queue.prune_older_than(cutoff);
            }
            total
        };
        if total > 0 {
            self.save_snapshot();
        }
        total
    }

    // -- Invite primitives --

    /// Store a fresh invite. Caller is responsible for token uniqueness +
    /// signature verification before reaching this point.
    pub fn create_invite(&self, invite: Invite) {
        self.invites
            .write()
            .unwrap()
            .insert(invite.token.clone(), invite);
        self.save_snapshot();
    }

    pub fn get_invite(&self, token: &str) -> Option<Invite> {
        self.invites.read().unwrap().get(token).cloned()
    }

    /// Return all *claimed* invites where `inviter` is the inviter. Used by
    /// the inviter's daemon to discover claim events and auto-add the
    /// claimer as a contact (bidirectional contact-add — the defining
    /// behavior of a correspondence invite, see
    /// `docs/milestones/2026-05-04-tauri-front-door.md` slice 2).
    ///
    /// Idempotent: returns the same list across calls until the invite is
    /// pruned by `prune_invites`. The daemon dedupes by checking its local
    /// contact book — no relay-side ack state needed.
    pub fn invites_claimed_for_inviter(&self, inviter: &Did) -> Vec<Invite> {
        self.invites
            .read()
            .unwrap()
            .values()
            .filter(|i| &i.inviter_did == inviter && i.claimed_by.is_some())
            .cloned()
            .collect()
    }

    /// Mark an invite as claimed. Returns the now-claimed invite, or `None`
    /// if the token is unknown or already claimed.
    pub fn claim_invite(
        &self,
        token: &str,
        claimant: Did,
        now: DateTime<Utc>,
    ) -> Option<Invite> {
        let result = {
            let mut invites = self.invites.write().unwrap();
            let invite = invites.get_mut(token)?;
            if invite.claimed_by.is_some() {
                return None;
            }
            if invite.expires_at < now {
                return None;
            }
            invite.claimed_by = Some(claimant);
            invite.claimed_at = Some(now);
            Some(invite.clone())
        };
        self.save_snapshot();
        result
    }

    /// Drop expired and claimed-and-old invites (TTL is the invite's own
    /// expires_at, plus a 7-day grace for claimed invites so the inviter's
    /// daemon can pick up the claim acknowledgment).
    pub fn prune_invites(&self, now: DateTime<Utc>) -> usize {
        use chrono::Duration;
        let removed = {
            let mut invites = self.invites.write().unwrap();
            let before = invites.len();
            invites.retain(|_, i| match i.claimed_at {
                Some(claimed_at) => now < claimed_at + Duration::days(7),
                None => i.expires_at >= now,
            });
            before - invites.len()
        };
        if removed > 0 {
            self.save_snapshot();
        }
        removed
    }
}
