//! Per-tenant queue of envelopes waiting to be polled by the recipient.
//!
//! v0 keeps queues in memory only — restart loses queued envelopes.
//! Acceptable because (a) recipients poll on a regular cadence so the gap is
//! bounded, (b) senders retry on transport failure (deferred to v0.x), and
//! (c) a disk-backed WAL can be added without changing the API surface.

use std::collections::VecDeque;

use chrono::{DateTime, Duration, Utc};
use secretariat_core::Did;
use serde::Serialize;

/// One envelope sitting in a recipient's queue.
#[derive(Debug, Clone, Serialize)]
pub struct QueuedEnvelope {
    pub id: u64,
    pub queued_at: DateTime<Utc>,
    pub sender_did: Option<Did>,
    /// Raw envelope bytes (markdown with frontmatter, optionally encrypted body).
    /// Serialized as base64 for JSON transport.
    #[serde(serialize_with = "serialize_b64")]
    pub body: Vec<u8>,
    pub content_type: String,
}

fn serialize_b64<S: serde::Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
    s.serialize_str(&B64.encode(bytes))
}

/// Per-tenant queue. Monotonic `next_id` so cursor pagination is stable.
///
/// IDs start at 1 (not 0). Combined with the `id > after` semantics in
/// [`since`], a fresh client passing `after=0` receives every queued
/// envelope; subsequent polls pass the largest id they've seen.
#[derive(Debug)]
pub struct TenantQueue {
    entries: VecDeque<QueuedEnvelope>,
    next_id: u64,
}

impl Default for TenantQueue {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            next_id: 1,
        }
    }
}

impl TenantQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push(
        &mut self,
        body: Vec<u8>,
        content_type: String,
        sender_did: Option<Did>,
        now: DateTime<Utc>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push_back(QueuedEnvelope {
            id,
            queued_at: now,
            sender_did,
            body,
            content_type,
        });
        id
    }

    /// All entries with `id > after`, in queue order.
    pub fn since(&self, after: u64) -> Vec<QueuedEnvelope> {
        self.entries
            .iter()
            .filter(|e| e.id > after)
            .cloned()
            .collect()
    }

    /// Drop entries queued before `cutoff`. Returns count pruned.
    pub fn prune_older_than(&mut self, cutoff: DateTime<Utc>) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| e.queued_at >= cutoff);
        before - self.entries.len()
    }
}

/// Convenience for the daemon: the standard TTL we prune queues against.
pub fn ttl_cutoff(now: DateTime<Utc>, ttl_days: i64) -> DateTime<Utc> {
    now - Duration::days(ttl_days)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    #[test]
    fn push_assigns_monotonic_ids_starting_at_one() {
        let mut q = TenantQueue::new();
        let id1 = q.push(b"a".to_vec(), "text/markdown".to_string(), None, now());
        let id2 = q.push(b"b".to_vec(), "text/markdown".to_string(), None, now());
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn since_returns_entries_after_cursor() {
        let mut q = TenantQueue::new();
        q.push(b"a".to_vec(), "text/markdown".to_string(), None, now()); // id 1
        q.push(b"b".to_vec(), "text/markdown".to_string(), None, now()); // id 2
        q.push(b"c".to_vec(), "text/markdown".to_string(), None, now()); // id 3

        // after=0 means "fresh client, give me everything"
        let from_zero = q.since(0);
        assert_eq!(from_zero.len(), 3);
        assert_eq!(from_zero[0].id, 1);

        // after=1 means "I've seen id 1, give me what came next"
        let from_one = q.since(1);
        assert_eq!(from_one.len(), 2);
        assert_eq!(from_one[0].id, 2);

        let from_high = q.since(99);
        assert!(from_high.is_empty());
    }

    #[test]
    fn prune_removes_old_entries() {
        let mut q = TenantQueue::new();
        let old = Utc::now() - Duration::days(10);
        let recent = Utc::now();
        q.push(b"old".to_vec(), "text/markdown".to_string(), None, old);
        q.push(b"recent".to_vec(), "text/markdown".to_string(), None, recent);

        let cutoff = Utc::now() - Duration::days(7);
        let pruned = q.prune_older_than(cutoff);
        assert_eq!(pruned, 1);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn ids_continue_after_prune() {
        let mut q = TenantQueue::new();
        q.push(b"old".to_vec(), "text/markdown".to_string(), None, Utc::now() - Duration::days(10)); // id 1
        q.push(b"old".to_vec(), "text/markdown".to_string(), None, Utc::now() - Duration::days(10)); // id 2
        q.prune_older_than(Utc::now() - Duration::days(7));
        let id = q.push(b"new".to_vec(), "text/markdown".to_string(), None, Utc::now());
        // next_id is monotonic and not reset by pruning.
        assert_eq!(id, 3);
    }
}
