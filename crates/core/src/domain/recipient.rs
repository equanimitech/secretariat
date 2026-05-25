//! `Recipient` — where an envelope is addressed.
//!
//! **Queues are the primitive.** Every envelope is addressed to a queue
//! identified by `(owner, handle)` — owner is the DID who *owns* that
//! queue (controls reads/writes), handle is the queue's bare-slug name
//! on the owner's machine.
//!
//! Under the substrate-for-themia collapse (2026-05-21), there is one
//! primitive — a channel — and two roots distinguishing where the
//! channel lives on disk:
//!
//! - **Org-scoped channel.** `owner ∈ org membership` →
//!   `orgs/<alias>/channels/<handle>/`. Federates to the org owner's
//!   relay.
//! - **Self-owned channel.** `owner == self_did` →
//!   `channels/<handle>/`. Local-only (journal, capture).
//!
//! Stamps are allowed on any envelope, regardless of recipient. A
//! principal stamping their own journal entry is valid (tamper-evident
//! self-attestation); a principal stamping an org-channel envelope is
//! the curation case (sign ≠ stamp). The send-routing rule decides what
//! *happens* to a signed envelope: daemon resolves the endpoint via the
//! org-membership index, NOT via a domain-layer `is_local` check.
//!
//! Wire format: `to: <owner-did>` + `handle: <bare-slug>`. Both always
//! present. The legacy DM-as-`(peer, inbox:default)` model is gone;
//! pre-collapse envelopes don't read under this build.

use super::{Did, QueueHandle};

/// Address pair: who owns the queue, and which queue on their machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipient {
    pub owner: Did,
    pub handle: QueueHandle,
}

impl Recipient {
    /// Construct a recipient from owner DID + queue handle.
    pub fn new(owner: Did, handle: QueueHandle) -> Self {
        Self { owner, handle }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alice_did() -> Did {
        Did::from_ed25519_public_key(&[0xa1; 32])
    }

    fn bob_did() -> Did {
        Did::from_ed25519_public_key(&[0xb0; 32])
    }

    #[test]
    fn owner_is_self_when_constructed_with_self_did() {
        let r = Recipient::new(alice_did(), QueueHandle::parse("triage").unwrap());
        assert_eq!(r.owner, alice_did());
    }

    #[test]
    fn owner_is_peer_when_constructed_with_peer_did() {
        // Peer-owned queue — under the collapse, this is an org-scoped
        // channel published by a peer principal (the org owner).
        let r = Recipient::new(bob_did(), QueueHandle::parse("book-progress").unwrap());
        assert_eq!(r.owner, bob_did());
        assert_ne!(r.owner, alice_did());
    }

    #[test]
    fn nested_handle_round_trips() {
        let r = Recipient::new(
            bob_did(),
            QueueHandle::parse("dommage-corporel:paris-cohort").unwrap(),
        );
        assert_eq!(r.handle.as_str(), "dommage-corporel:paris-cohort");
        assert_eq!(r.handle.as_path_segment(), "dommage-corporel/paris-cohort");
    }
}
