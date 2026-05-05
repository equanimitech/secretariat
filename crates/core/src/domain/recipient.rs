//! `Recipient` — where an envelope is addressed.
//!
//! **Queues are the primitive.** Every envelope is addressed to a queue
//! identified by `(owner, handle)` — owner is the DID who *owns* that
//! queue (controls reads/writes), handle is the queue's local name on
//! the owner's machine.
//!
//! Three real-world cases all collapse to this primitive:
//!
//! - **Local capture** — `owner == self_did`, e.g.
//!   `(self, inbox:triage)`. Stays on the principal's disk.
//! - **Direct peer letter** — `owner == peer_did`, e.g.
//!   `(marcelo, inbox:default)`. Today's "letter to a peer" is a
//!   queue-of-1 on the peer's relay.
//! - **Channel / newsletter** — `owner == publisher_did`, e.g.
//!   `(marcelo, channel:book-progress)`. Multiple subscribers poll the
//!   same `(owner, handle)` tuple from the publisher's relay. Same
//!   primitive, different access pattern.
//!
//! Stamps are allowed on any envelope, regardless of recipient. A
//! principal stamping their own journal entry is valid (tamper-evident
//! self-attestation); a principal stamping a peer letter is the usual
//! case. The send-routing rule decides what *happens* to a stamped
//! envelope: `owner == self_did` stays put; `owner != self_did` goes
//! out to the owner's relay.
//!
//! Wire format: `to: <owner-did>` + `handle: <namespace:slug>`. Both
//! always present. Legacy peer letters that pre-date this collapse use
//! `inbox:default` as the synthetic handle on read.
//!
//! See `docs/pitches/2026-05-05-event-sourced-envelope-substrate.md`
//! and the queues-as-primitive collapse (2026-05-05).

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

    /// True when the queue is owned by `me` — i.e. lives on this
    /// principal's local disk and never crosses a transport.
    pub fn is_local(&self, me: &Did) -> bool {
        &self.owner == me
    }

    /// True when the queue is owned by someone else — i.e. delivery
    /// requires hitting the owner's relay.
    pub fn is_remote(&self, me: &Did) -> bool {
        !self.is_local(me)
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
    fn local_when_owner_is_self() {
        let r = Recipient::new(alice_did(), QueueHandle::parse("inbox:triage").unwrap());
        assert!(r.is_local(&alice_did()));
        assert!(!r.is_remote(&alice_did()));
    }

    #[test]
    fn remote_when_owner_is_peer() {
        let r = Recipient::new(bob_did(), QueueHandle::parse("inbox:default").unwrap());
        assert!(!r.is_local(&alice_did()));
        assert!(r.is_remote(&alice_did()));
    }

    #[test]
    fn channel_handle_works() {
        // A newsletter is just a queue with a `channel:` namespace.
        let r = Recipient::new(
            bob_did(),
            QueueHandle::parse("channel:book-progress").unwrap(),
        );
        assert!(r.is_remote(&alice_did()));
        assert_eq!(r.handle.namespace(), "channel");
        assert_eq!(r.handle.slug(), "book-progress");
    }
}
