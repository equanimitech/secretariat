//! `EnvelopeSignature` — author signature applied at compose time.
//!
//! Substrate-for-themia Move 2 (2026-05-21). Distinct cryptographic layer
//! from [`crate::domain::Stamp`]:
//!
//! - `EnvelopeSignature` (this module) — author's signature; mandatory on
//!   every post-Move-2 envelope on the wire. Typically signed by an
//!   agent's key (scribe role); may be signed by the principal for
//!   manually-composed envelopes.
//! - `Stamp` — principal's Touch-ID-gated attestation; selective.
//!
//! Per AGENTS.md hard rule #4 (three-layer trust model), separate keys
//! mean receivers can cryptographically distinguish 'scribe-composed' from
//! 'principal-composed' from 'Touch-ID-attested' without trusting a UI
//! convention.
//!
//! Pure domain: no IO. Sign + verify operate over the body's canonical
//! hash (the same `docHash` shape the `Stamp` uses), so a tamper to the
//! body invalidates both layers identically.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{canonical_body_hash, Did, DocHash, Signature};

// ---------------------------------------------------------------------------
// SignerRole
// ---------------------------------------------------------------------------

/// Role of the author at signing time. Hint for receiver-side UI; the
/// cryptographic check still relies on `signer` + the agent-manifest
/// chain (for `Agent`) or direct DID resolution (for `Principal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignerRole {
    Agent,
    Principal,
}

#[derive(Debug, Error)]
pub enum SignerRoleParseError {
    #[error("unknown signer_role `{0}` (known: agent, principal)")]
    Unknown(String),
}

impl SignerRole {
    pub fn parse(s: &str) -> Result<Self, SignerRoleParseError> {
        match s {
            "agent" => Ok(Self::Agent),
            "principal" => Ok(Self::Principal),
            other => Err(SignerRoleParseError::Unknown(other.to_string())),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Principal => "principal",
        }
    }
}

impl std::fmt::Display for SignerRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// EnvelopeSignature
// ---------------------------------------------------------------------------

/// A detached ed25519 signature by the envelope's author over the
/// canonical body hash. See module docs for the trust-model context.
///
/// Lexicon: `tech.equanimi.secretariat.signature`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeSignature {
    pub signer: Did,
    pub signer_role: SignerRole,
    pub doc_hash: DocHash,
    pub signed_at: DateTime<Utc>,
    pub signature: Signature,
}

impl EnvelopeSignature {
    pub const TYPE_ID: &'static str = "tech.equanimi.secretariat.signature";

    /// Build a signature value from parts. Does NOT verify the signature
    /// against a verifying key — that is the application layer's job (it
    /// resolves the signer's DID to a key, and for agents consults the
    /// `agentManifest` cache to confirm authorization).
    pub fn new(
        signer: Did,
        signer_role: SignerRole,
        doc_hash: DocHash,
        signed_at: DateTime<Utc>,
        signature: Signature,
    ) -> Self {
        Self {
            signer,
            signer_role,
            doc_hash,
            signed_at,
            signature,
        }
    }

    /// Sign a body's canonical hash with the provided ed25519 signing
    /// key. Pure domain — no IO; the caller threads in the loaded key.
    /// Computes the body hash via [`canonical_body_hash`] so the same
    /// canonicalization rules drive both `$signature` and `$attestation`.
    pub fn sign_body(
        signer: Did,
        signer_role: SignerRole,
        body: &str,
        signed_at: DateTime<Utc>,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> Self {
        use ed25519_dalek::Signer as _;
        let doc_hash = canonical_body_hash(body);
        let dalek_sig = signing_key.sign(doc_hash.as_bytes());
        Self {
            signer,
            signer_role,
            doc_hash,
            signed_at,
            signature: Signature::from_bytes(dalek_sig.to_bytes()),
        }
    }

    /// Verify the signature against the provided verifying key AND against
    /// the body's current canonical hash. Returns `true` iff both
    /// (a) the body hash matches the signed-over hash, and (b) the
    /// signature verifies under the verifying key. A receiver that
    /// catches `false` here knows the envelope is either tampered (case a)
    /// or signed by a different key than provided (case b).
    pub fn verify_body(&self, body: &str, verifying_key: &ed25519_dalek::VerifyingKey) -> bool {
        use ed25519_dalek::Verifier as _;
        let computed = canonical_body_hash(body);
        if computed != self.doc_hash {
            return false;
        }
        let dalek_sig = ed25519_dalek::Signature::from_bytes(self.signature.as_bytes());
        verifying_key
            .verify(self.doc_hash.as_bytes(), &dalek_sig)
            .is_ok()
    }
}

// -- Serde wire format --------------------------------------------------------
//
// Mirrors the `Stamp` pattern: a private `EnvelopeSignatureWire` so we can
// emit `$type` and reject documents whose `$type` doesn't match on input.

#[derive(Serialize, Deserialize)]
struct EnvelopeSignatureWire {
    #[serde(rename = "$type")]
    type_id: String,
    signer: Did,
    #[serde(rename = "signerRole")]
    signer_role: SignerRole,
    #[serde(rename = "docHash")]
    doc_hash: DocHash,
    #[serde(rename = "signedAt")]
    signed_at: DateTime<Utc>,
    signature: Signature,
}

impl Serialize for EnvelopeSignature {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        EnvelopeSignatureWire {
            type_id: Self::TYPE_ID.to_string(),
            signer: self.signer.clone(),
            signer_role: self.signer_role,
            doc_hash: self.doc_hash.clone(),
            signed_at: self.signed_at,
            signature: self.signature.clone(),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for EnvelopeSignature {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let wire = EnvelopeSignatureWire::deserialize(d)?;
        if wire.type_id != Self::TYPE_ID {
            return Err(serde::de::Error::custom(format!(
                "expected $type {}, got {}",
                Self::TYPE_ID,
                wire.type_id
            )));
        }
        Ok(EnvelopeSignature {
            signer: wire.signer,
            signer_role: wire.signer_role,
            doc_hash: wire.doc_hash,
            signed_at: wire.signed_at,
            signature: wire.signature,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixture_signing_key() -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[0x42; 32])
    }

    fn fixture_signer_did(key: &ed25519_dalek::SigningKey) -> Did {
        Did::from_ed25519_public_key(&key.verifying_key().to_bytes())
    }

    #[test]
    fn signer_role_parse_roundtrip() {
        assert_eq!(SignerRole::parse("agent").unwrap(), SignerRole::Agent);
        assert_eq!(
            SignerRole::parse("principal").unwrap(),
            SignerRole::Principal
        );
        assert!(SignerRole::parse("scribe").is_err());
    }

    #[test]
    fn sign_then_verify_roundtrip() {
        let key = fixture_signing_key();
        let did = fixture_signer_did(&key);
        let body = "# Hello\n\nworld\n";
        let when = Utc.with_ymd_and_hms(2026, 5, 25, 12, 0, 0).unwrap();
        let sig = EnvelopeSignature::sign_body(did, SignerRole::Agent, body, when, &key);
        assert!(sig.verify_body(body, &key.verifying_key()));
    }

    #[test]
    fn verify_fails_on_body_tamper() {
        let key = fixture_signing_key();
        let did = fixture_signer_did(&key);
        let body = "# Hello\n";
        let when = Utc.with_ymd_and_hms(2026, 5, 25, 12, 0, 0).unwrap();
        let sig = EnvelopeSignature::sign_body(did, SignerRole::Agent, body, when, &key);
        // Body changed; signature was over the original hash.
        assert!(!sig.verify_body("# Hello\n\ntampered\n", &key.verifying_key()));
    }

    #[test]
    fn verify_fails_under_wrong_key() {
        let key = fixture_signing_key();
        let did = fixture_signer_did(&key);
        let body = "# Hello\n";
        let when = Utc.with_ymd_and_hms(2026, 5, 25, 12, 0, 0).unwrap();
        let sig = EnvelopeSignature::sign_body(did, SignerRole::Agent, body, when, &key);
        let other = ed25519_dalek::SigningKey::from_bytes(&[0x99; 32]);
        assert!(!sig.verify_body(body, &other.verifying_key()));
    }

    #[test]
    fn signature_roundtrip_yaml() {
        let key = fixture_signing_key();
        let did = fixture_signer_did(&key);
        let body = "# Hello\n";
        let when = Utc.with_ymd_and_hms(2026, 5, 25, 12, 0, 0).unwrap();
        let sig = EnvelopeSignature::sign_body(did, SignerRole::Principal, body, when, &key);
        let yaml = serde_yaml::to_string(&sig).unwrap();
        assert!(yaml.contains("$type: tech.equanimi.secretariat.signature"));
        assert!(yaml.contains("signerRole: principal"));
        let back: EnvelopeSignature = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back, sig);
        // Round-tripped signature still verifies.
        assert!(back.verify_body(body, &key.verifying_key()));
    }

    #[test]
    fn signature_rejects_wrong_type() {
        let bad = "$type: wrong.type\nsigner: did:web:rafa.equanimi.tech\nsignerRole: agent\ndocHash: sha256:0000000000000000000000000000000000000000000000000000000000000000\nsignedAt: 2026-05-25T12:00:00Z\nsignature: ed25519:QkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkI=\n";
        let r: Result<EnvelopeSignature, _> = serde_yaml::from_str(bad);
        assert!(r.is_err());
    }
}
