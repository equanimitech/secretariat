//! Server-wide state held in axum's `State` extractor.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use ed25519_dalek::VerifyingKey;
use secretariat_core::Did;

use crate::auth::AuthState;
use crate::config::Config;
use crate::queue::TenantQueue;

/// A registered tenant.
#[derive(Debug, Clone)]
pub struct RegisteredTenant {
    pub did: Did,
    pub pubkey: VerifyingKey,
    pub registered_at: DateTime<Utc>,
}

/// A pending invite. Created by an inviter, claimed by exactly one peer.
#[derive(Debug, Clone)]
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
    pub fn new(config: Config) -> Arc<Self> {
        Arc::new(Self {
            config,
            auth: AuthState::new(),
            registry: RwLock::new(HashMap::new()),
            queues: RwLock::new(HashMap::new()),
            invites: RwLock::new(HashMap::new()),
        })
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
        let mut q = self.queues.write().unwrap();
        let queue = q.entry(recipient).or_default();
        queue.push(body, content_type, sender_did, now)
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
        let mut total = 0;
        for queue in self.queues.write().unwrap().values_mut() {
            total += queue.prune_older_than(cutoff);
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
    }

    pub fn get_invite(&self, token: &str) -> Option<Invite> {
        self.invites.read().unwrap().get(token).cloned()
    }

    /// Mark an invite as claimed. Returns the now-claimed invite, or `None`
    /// if the token is unknown or already claimed.
    pub fn claim_invite(
        &self,
        token: &str,
        claimant: Did,
        now: DateTime<Utc>,
    ) -> Option<Invite> {
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
    }

    /// Drop expired and claimed-and-old invites (TTL is the invite's own
    /// expires_at, plus a 7-day grace for claimed invites so the inviter's
    /// daemon can pick up the claim acknowledgment).
    pub fn prune_invites(&self, now: DateTime<Utc>) -> usize {
        use chrono::Duration;
        let mut invites = self.invites.write().unwrap();
        let before = invites.len();
        invites.retain(|_, i| match i.claimed_at {
            Some(claimed_at) => now < claimed_at + Duration::days(7),
            None => i.expires_at >= now,
        });
        before - invites.len()
    }
}
