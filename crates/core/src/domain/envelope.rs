//! `Envelope` — one bid for the receiver's attention.
//!
//! Composed by the scribe (AI) and addressed by the principal. The envelope is
//! routing metadata; in MVP the cryptographic stamp covers only the body, not
//! the envelope (see decision log #1 in the plan). v2 may add envelope signing
//! for bilateral bound enforcement.
//!
//! ## Encryption
//!
//! When `encryption` is `Some(_)`, the document body is no longer plaintext
//! markdown — it is the wire-string form of an encrypted blob (see
//! [`crate::infrastructure::crypto::sealed::SealedBox`]). The hash invariant
//! is unchanged: `docHash` covers the body bytes (which are the wire string),
//! so the ed25519 signature authenticates the bytes that travel over the
//! transport. Decryption happens *after* verification, on the recipient side,
//! using their X25519 secret derived from their ed25519 signing key.
//!
//! When `encryption` is `None`, the body is plaintext markdown and behavior
//! matches the Day-1 design.

use serde::{Deserialize, Serialize};

use super::{Did, DocHash, EnvelopeDepth, EnvelopeUrgency, QueueHandle, Recipient};

/// The encryption scheme applied to the document body. v0 ships a single
/// scheme; future versions may add others (post-quantum, etc.) by extending
/// this enum and bumping the wire-format identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncryptionScheme {
    /// X25519 ECDH key agreement + XChaCha20-Poly1305 AEAD.
    /// Wire identifier: `x25519-xchacha20poly1305`.
    X25519XChaCha20Poly1305,
}

impl EncryptionScheme {
    pub const X25519_XCHACHA20POLY1305_ID: &'static str = "x25519-xchacha20poly1305";

    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::X25519XChaCha20Poly1305 => Self::X25519_XCHACHA20POLY1305_ID,
        }
    }
}

impl Serialize for EncryptionScheme {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_wire_str())
    }
}

impl<'de> Deserialize<'de> for EncryptionScheme {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_str() {
            Self::X25519_XCHACHA20POLY1305_ID => Ok(Self::X25519XChaCha20Poly1305),
            other => Err(serde::de::Error::custom(format!(
                "unknown encryption scheme `{other}`"
            ))),
        }
    }
}

/// Lexicon: `tech.equanimi.secretariat.envelope`.
///
/// Queues are the primitive: every envelope addresses a `(owner, handle)`
/// queue. Wire format: `to: <owner-did>` + `handle: <namespace:slug>`,
/// both always present. Local captures, peer letters, and channel
/// broadcasts collapse to the same shape — discrimination is by
/// `owner == self_did?` at routing time, not by enum variant at parse
/// time.
///
/// Legacy v0.2.x envelopes had `to: <did>` with no handle; on read we
/// synthesize `inbox:default` so old peer letters keep working.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub from: Did,
    pub recipient: Recipient,
    pub depth: EnvelopeDepth,
    pub urgency: EnvelopeUrgency,
    pub source: String,
    pub cadence_hint: Option<String>,
    /// Body encryption scheme. `None` = plaintext markdown body. `Some(_)` =
    /// body is the wire string of an encrypted blob; decrypt after verify.
    pub encryption: Option<EncryptionScheme>,
    /// Optional reference to a prior envelope's body hash that this
    /// envelope is replying to. Root envelopes leave this `None`; threads
    /// are chains of `reply_to` links. Causality is cryptographic
    /// (`DocHash` of the parent body) — adapters materialize chains into
    /// their native conversation view (e.g. Crisp session, Intercom
    /// thread). Orphan replies (parent not yet seen) are valid; the
    /// substrate does not enforce parent reachability.
    pub reply_to: Option<DocHash>,
}

impl Envelope {
    pub const TYPE_ID: &'static str = "tech.equanimi.secretariat.envelope";

    pub fn builder(from: Did, recipient: Recipient) -> EnvelopeBuilder {
        EnvelopeBuilder::new(from, recipient)
    }

    /// Convenience: is this envelope's body encrypted?
    pub fn is_encrypted(&self) -> bool {
        self.encryption.is_some()
    }
}

#[derive(Serialize, Deserialize)]
struct EnvelopeWire {
    #[serde(rename = "$type")]
    type_id: String,
    from: Did,
    /// Recipient queue owner DID. Always present.
    to: Did,
    /// Recipient queue handle (`<namespace>:<slug>`). Always present in
    /// envelopes written by v0.3+. Legacy v0.2.x envelopes omitted this
    /// field; the deserializer synthesizes `inbox:default` for them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    handle: Option<QueueHandle>,
    /// Legacy field name from the brief v0.3 pre-collapse window, when
    /// local-queue envelopes used `queue: <handle>` and `to` carried
    /// only peer DIDs. Promoted on read to the unified `(to, handle)`
    /// pair. Never written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    queue: Option<QueueHandle>,
    depth: EnvelopeDepth,
    urgency: EnvelopeUrgency,
    source: String,
    #[serde(rename = "cadenceHint", default, skip_serializing_if = "Option::is_none")]
    cadence_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    encryption: Option<EncryptionScheme>,
    #[serde(rename = "replyTo", default, skip_serializing_if = "Option::is_none")]
    reply_to: Option<DocHash>,
}

impl Serialize for Envelope {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        EnvelopeWire {
            type_id: Self::TYPE_ID.to_string(),
            from: self.from.clone(),
            to: self.recipient.owner.clone(),
            handle: Some(self.recipient.handle.clone()),
            queue: None,
            depth: self.depth,
            urgency: self.urgency,
            source: self.source.clone(),
            cadence_hint: self.cadence_hint.clone(),
            encryption: self.encryption,
            reply_to: self.reply_to.clone(),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Envelope {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let w = EnvelopeWire::deserialize(d)?;
        if w.type_id != Self::TYPE_ID {
            return Err(serde::de::Error::custom(format!(
                "expected $type {}, got {}",
                Self::TYPE_ID,
                w.type_id
            )));
        }
        // Handle resolution priority:
        //   1. explicit `handle:` (v0.3+ canonical form)
        //   2. legacy `queue:` (brief pre-collapse window — promote)
        //   3. neither → synthesize `inbox:default` (v0.2.x peer letter)
        let handle = match (w.handle, w.queue) {
            (Some(h), None) => h,
            (None, Some(h)) => h,
            (Some(h), Some(_)) => h,
            (None, None) => QueueHandle::parse("inbox:default")
                .expect("inbox:default is a valid handle"),
        };
        Ok(Envelope {
            from: w.from,
            recipient: Recipient::new(w.to, handle),
            depth: w.depth,
            urgency: w.urgency,
            source: w.source,
            cadence_hint: w.cadence_hint,
            encryption: w.encryption,
            reply_to: w.reply_to,
        })
    }
}

/// Fluent builder. Mandatory: `from`, `recipient`. Defaults:
/// `depth = Subtle`, `urgency = Whenever`, `source = ""`, `encryption = None`.
///
/// `recipient` is `(owner, handle)`. Letter to a peer:
/// `Recipient::new(peer_did, QueueHandle::parse("inbox:default")?)`.
/// Local capture: `Recipient::new(self_did, QueueHandle::parse("inbox:triage")?)`.
/// Channel post: `Recipient::new(self_did, QueueHandle::parse("channel:foo")?)`.
#[derive(Debug, Clone)]
pub struct EnvelopeBuilder {
    from: Did,
    recipient: Recipient,
    depth: EnvelopeDepth,
    urgency: EnvelopeUrgency,
    source: String,
    cadence_hint: Option<String>,
    encryption: Option<EncryptionScheme>,
    reply_to: Option<DocHash>,
}

impl EnvelopeBuilder {
    pub fn new(from: Did, recipient: Recipient) -> Self {
        Self {
            from,
            recipient,
            depth: EnvelopeDepth::Subtle,
            urgency: EnvelopeUrgency::Whenever,
            source: String::new(),
            cadence_hint: None,
            encryption: None,
            reply_to: None,
        }
    }

    pub fn depth(mut self, depth: EnvelopeDepth) -> Self {
        self.depth = depth;
        self
    }

    pub fn urgency(mut self, urgency: EnvelopeUrgency) -> Self {
        self.urgency = urgency;
        self
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    pub fn cadence_hint(mut self, hint: impl Into<String>) -> Self {
        self.cadence_hint = Some(hint.into());
        self
    }

    pub fn encryption(mut self, scheme: EncryptionScheme) -> Self {
        self.encryption = Some(scheme);
        self
    }

    pub fn reply_to(mut self, parent: DocHash) -> Self {
        self.reply_to = Some(parent);
        self
    }

    pub fn build(self) -> Envelope {
        Envelope {
            from: self.from,
            recipient: self.recipient,
            depth: self.depth,
            urgency: self.urgency,
            source: self.source,
            cadence_hint: self.cadence_hint,
            encryption: self.encryption,
            reply_to: self.reply_to,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rafa() -> Did {
        Did::parse("did:web:rafa.equanimi.tech").unwrap()
    }

    fn marcelo() -> Did {
        Did::parse("did:web:marcelo.ballestiero.com").unwrap()
    }

    fn synth_peer() -> Did {
        Did::from_ed25519_public_key(&[0xa1; 32])
    }

    fn peer_to_marcelo() -> Recipient {
        Recipient::new(marcelo(), QueueHandle::parse("inbox:default").unwrap())
    }

    fn fixture() -> Envelope {
        Envelope::builder(rafa(), peer_to_marcelo())
            .depth(EnvelopeDepth::Subtle)
            .urgency(EnvelopeUrgency::Soon)
            .source("claude-code-2026-04-30T14:22:00Z")
            .cadence_hint("morning")
            .build()
    }

    #[test]
    fn envelope_roundtrip_yaml() {
        let e = fixture();
        let yaml = serde_yaml::to_string(&e).unwrap();
        let back: Envelope = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn envelope_emits_camel_case_keys_and_type() {
        let yaml = serde_yaml::to_string(&fixture()).unwrap();
        assert!(yaml.contains("$type: tech.equanimi.secretariat.envelope"));
        assert!(yaml.contains("cadenceHint"));
    }

    #[test]
    fn envelope_serializes_with_to_and_handle_fields() {
        let yaml = serde_yaml::to_string(&fixture()).unwrap();
        assert!(yaml.contains("to: did:web:marcelo.ballestiero.com"));
        assert!(yaml.contains("handle: inbox:default"));
        assert!(!yaml.contains("queue:"));
    }

    #[test]
    fn local_queue_envelope_uses_self_did_as_owner() {
        let e = Envelope::builder(
            rafa(),
            Recipient::new(rafa(), QueueHandle::parse("inbox:triage").unwrap()),
        )
        .source("quick-pane")
        .build();
        let yaml = serde_yaml::to_string(&e).unwrap();
        assert!(yaml.contains("to: did:web:rafa.equanimi.tech"));
        assert!(yaml.contains("handle: inbox:triage"));
        assert!(e.recipient.is_local(&rafa()));
    }

    #[test]
    fn local_queue_envelope_roundtrip_yaml() {
        let e = Envelope::builder(
            synth_peer(),
            Recipient::new(synth_peer(), QueueHandle::parse("inbox:to-self").unwrap()),
        )
        .source("quick-pane")
        .build();
        let yaml = serde_yaml::to_string(&e).unwrap();
        let back: Envelope = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn legacy_envelope_without_handle_synthesizes_inbox_default() {
        // v0.2.x peer letter — `to` only, no `handle`. Read must succeed,
        // synthesizing `inbox:default` so old files keep working.
        let yaml = "$type: tech.equanimi.secretariat.envelope\n\
                    from: did:web:rafa.equanimi.tech\n\
                    to: did:web:marcelo.ballestiero.com\n\
                    depth: subtle\n\
                    urgency: whenever\n\
                    source: legacy-test\n";
        let env: Envelope = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(env.recipient.handle.as_str(), "inbox:default");
        assert_eq!(env.recipient.owner, marcelo());
    }

    #[test]
    fn pre_collapse_queue_field_is_promoted_to_handle() {
        // Brief v0.3 pre-collapse window used `queue: <handle>` for
        // local captures with `to` absent. After collapse those still
        // need to read; we accept `to` + `queue` and promote `queue`
        // into the unified `handle`.
        let yaml = "$type: tech.equanimi.secretariat.envelope\n\
                    from: did:web:rafa.equanimi.tech\n\
                    to: did:web:rafa.equanimi.tech\n\
                    queue: inbox:triage\n\
                    depth: subtle\n\
                    urgency: whenever\n\
                    source: legacy-test\n";
        let env: Envelope = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(env.recipient.handle.as_str(), "inbox:triage");
    }

    #[test]
    fn envelope_omits_encryption_when_plaintext() {
        let e = fixture();
        assert!(!e.is_encrypted());
        let yaml = serde_yaml::to_string(&e).unwrap();
        assert!(!yaml.contains("encryption"));
    }

    #[test]
    fn envelope_emits_encryption_marker_when_encrypted() {
        let e = Envelope::builder(rafa(), peer_to_marcelo())
            .encryption(EncryptionScheme::X25519XChaCha20Poly1305)
            .build();
        assert!(e.is_encrypted());
        let yaml = serde_yaml::to_string(&e).unwrap();
        assert!(yaml.contains("encryption: x25519-xchacha20poly1305"));
    }

    #[test]
    fn envelope_encrypted_yaml_roundtrip() {
        let e = Envelope::builder(
            rafa(),
            Recipient::new(synth_peer(), QueueHandle::parse("inbox:default").unwrap()),
        )
        .depth(EnvelopeDepth::Subtle)
        .urgency(EnvelopeUrgency::Whenever)
        .source("daemon-2026-05-02")
        .encryption(EncryptionScheme::X25519XChaCha20Poly1305)
        .build();
        let yaml = serde_yaml::to_string(&e).unwrap();
        let back: Envelope = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(e, back);
        assert_eq!(
            back.encryption,
            Some(EncryptionScheme::X25519XChaCha20Poly1305)
        );
    }

    #[test]
    fn envelope_omits_reply_to_when_root() {
        let e = fixture();
        assert!(e.reply_to.is_none());
        let yaml = serde_yaml::to_string(&e).unwrap();
        assert!(!yaml.contains("replyTo"));
    }

    #[test]
    fn envelope_emits_reply_to_when_threaded() {
        let parent = DocHash::from_bytes([0x42; 32]);
        let e = Envelope::builder(rafa(), peer_to_marcelo())
            .source("reply")
            .reply_to(parent.clone())
            .build();
        let yaml = serde_yaml::to_string(&e).unwrap();
        assert!(yaml.contains(&format!("replyTo: {parent}")));
    }

    #[test]
    fn envelope_reply_to_yaml_roundtrip() {
        let parent = DocHash::from_bytes([0xab; 32]);
        let e = Envelope::builder(rafa(), peer_to_marcelo())
            .source("reply")
            .reply_to(parent.clone())
            .build();
        let yaml = serde_yaml::to_string(&e).unwrap();
        let back: Envelope = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(e, back);
        assert_eq!(back.reply_to, Some(parent));
    }

    #[test]
    fn legacy_envelope_without_reply_to_reads_as_root() {
        // Pre-threading envelope — no `replyTo` key. Must read with
        // `reply_to: None` (i.e. as a root envelope).
        let yaml = "$type: tech.equanimi.secretariat.envelope\n\
                    from: did:web:rafa.equanimi.tech\n\
                    to: did:web:marcelo.ballestiero.com\n\
                    handle: inbox:default\n\
                    depth: subtle\n\
                    urgency: whenever\n\
                    source: legacy-test\n";
        let env: Envelope = serde_yaml::from_str(yaml).unwrap();
        assert!(env.reply_to.is_none());
    }

    #[test]
    fn envelope_rejects_unknown_encryption_scheme() {
        let yaml = "$type: tech.equanimi.secretariat.envelope\n\
                    from: did:web:rafa.equanimi.tech\n\
                    to: did:web:marcelo.ballestiero.com\n\
                    depth: subtle\n\
                    urgency: whenever\n\
                    source: x\n\
                    encryption: aes-gcm-future-scheme\n";
        let r: Result<Envelope, _> = serde_yaml::from_str(yaml);
        assert!(r.is_err());
    }
}
