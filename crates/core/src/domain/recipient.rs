//! `Recipient` — where an envelope is addressed.
//!
//! Three variants:
//!
//! - **`Peer(Did)`** — addressed to another principal. Crosses the H↔H
//!   trust boundary; eventually requires a stamp before delivery.
//! - **`LocalQueue(QueueHandle)`** — addressed to a local collection
//!   inside the principal's own state (e.g. `inbox:triage` for ideas
//!   captured for later review). Never travels transport. Stamp-forbidden
//!   by domain invariant — there's no boundary to cross, so signing is
//!   meaningless and would only confuse later semantics.
//! - **`SelfAddressed`** — legacy `to: None` semantics (compose `--to`
//!   omitted; lands in `outbox/_self/`). Kept for back-compat with
//!   v0.2.x outbox files. Treated like `Peer(my_did)` for stamp
//!   purposes (the principal CAN stamp a letter to themselves).
//!
//! Wire-format strategy: `Peer(did)` serializes as the existing
//! `to: <did>` field for backward compatibility with v0.2.x peers.
//! `LocalQueue` serializes as a new `queue: <handle>` field.
//! `SelfAddressed` serializes as neither field present. At most one of
//! `to` / `queue` is set; setting both is rejected at deserialization.
//!
//! See `docs/pitches/2026-05-05-event-sourced-envelope-substrate.md`.

use super::{Did, QueueHandle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recipient {
    Peer(Did),
    LocalQueue(QueueHandle),
    SelfAddressed,
}

impl Recipient {
    /// True when this recipient permits a stamp. Local queues forbid
    /// stamps by invariant; peer + self-addressed allow them.
    pub fn allows_stamp(&self) -> bool {
        match self {
            Self::Peer(_) | Self::SelfAddressed => true,
            Self::LocalQueue(_) => false,
        }
    }

    /// True when this recipient travels over a transport (the wire to a
    /// peer's relay). Local queues never travel.
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Peer(_))
    }

    /// Convenience: extract the peer DID if this is a Peer variant.
    pub fn as_peer_did(&self) -> Option<&Did> {
        match self {
            Self::Peer(did) => Some(did),
            _ => None,
        }
    }

    /// Convenience: extract the queue handle if this is a LocalQueue variant.
    pub fn as_queue_handle(&self) -> Option<&QueueHandle> {
        match self {
            Self::LocalQueue(h) => Some(h),
            _ => None,
        }
    }
}

/// What category of envelope this is. Drives behavior in the walker
/// (different action bars for `Letter` vs `Idea`) and lets capture
/// flows label intent at write-time.
///
/// Reserved variants (`Pain`, `Note`, `Task`) are intentionally absent
/// from v1 — additive, not breaking, when added later. v1 ships only
/// what the substrate proves: principal-authored peer letters and
/// principal-captured ideas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvelopeKind {
    /// Authored correspondence to a peer (or self). The classical
    /// envelope shape. Stamp eventually required for `Peer`/`SelfAddressed`
    /// recipients.
    Letter,
    /// Captured thought, hint, or proto-message. Lives in a local queue
    /// until promoted (to a Letter) or archived. Never stamped.
    Idea,
}

impl EnvelopeKind {
    pub const LETTER_WIRE: &'static str = "letter";
    pub const IDEA_WIRE: &'static str = "idea";

    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Letter => Self::LETTER_WIRE,
            Self::Idea => Self::IDEA_WIRE,
        }
    }
}

impl serde::Serialize for EnvelopeKind {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_wire_str())
    }
}

impl<'de> serde::Deserialize<'de> for EnvelopeKind {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_str() {
            Self::LETTER_WIRE => Ok(Self::Letter),
            Self::IDEA_WIRE => Ok(Self::Idea),
            other => Err(serde::de::Error::custom(format!(
                "unknown envelope kind `{other}` (known: letter, idea)"
            ))),
        }
    }
}

impl Default for EnvelopeKind {
    fn default() -> Self {
        Self::Letter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alice_did() -> Did {
        Did::from_ed25519_public_key(&[0xa1; 32])
    }

    #[test]
    fn peer_recipient_allows_stamp_and_is_remote() {
        let r = Recipient::Peer(alice_did());
        assert!(r.allows_stamp());
        assert!(r.is_remote());
        assert!(r.as_peer_did().is_some());
        assert!(r.as_queue_handle().is_none());
    }

    #[test]
    fn local_queue_recipient_forbids_stamp_and_is_local() {
        let r = Recipient::LocalQueue(QueueHandle::parse("inbox:triage").unwrap());
        assert!(!r.allows_stamp());
        assert!(!r.is_remote());
        assert!(r.as_peer_did().is_none());
        assert!(r.as_queue_handle().is_some());
    }

    #[test]
    fn self_addressed_recipient_allows_stamp_but_is_local() {
        let r = Recipient::SelfAddressed;
        assert!(r.allows_stamp());
        assert!(!r.is_remote());
        assert!(r.as_peer_did().is_none());
        assert!(r.as_queue_handle().is_none());
    }

    #[test]
    fn envelope_kind_serde_roundtrip() {
        let json = serde_json::to_string(&EnvelopeKind::Letter).unwrap();
        assert_eq!(json, "\"letter\"");
        let back: EnvelopeKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, EnvelopeKind::Letter);

        let json = serde_json::to_string(&EnvelopeKind::Idea).unwrap();
        assert_eq!(json, "\"idea\"");
        let back: EnvelopeKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, EnvelopeKind::Idea);
    }

    #[test]
    fn envelope_kind_rejects_unknown_wire() {
        let r: Result<EnvelopeKind, _> = serde_json::from_str("\"pain\"");
        assert!(r.is_err());
    }

    #[test]
    fn envelope_kind_default_is_letter() {
        assert_eq!(EnvelopeKind::default(), EnvelopeKind::Letter);
    }
}
