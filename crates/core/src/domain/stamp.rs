//! `Stamp` — the signed human act on a document.
//!
//! Once issued, a stamp is immutable. Construction here does NOT verify the
//! signature against the body hash — that is the `AttestedDocument` aggregate's
//! invariant, since signature verification is IO-bound (it requires resolving
//! the signer's DID document).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Did, DocHash, Signature, StampAct};

/// A signed attestation that the principal performed `act` on a document
/// whose canonical body hashes to `doc_hash`.
///
/// Lexicon: `tech.equanimi.secretariat.stamp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub signer: Did,
    pub act: StampAct,
    pub doc_hash: DocHash,
    pub doc_filename: Option<String>,
    pub stamped_at: DateTime<Utc>,
    pub signature: Signature,
}

impl Stamp {
    pub const TYPE_ID: &'static str = "tech.equanimi.secretariat.stamp";

    pub fn new(
        signer: Did,
        act: StampAct,
        doc_hash: DocHash,
        doc_filename: Option<String>,
        stamped_at: DateTime<Utc>,
        signature: Signature,
    ) -> Self {
        Self {
            signer,
            act,
            doc_hash,
            doc_filename,
            stamped_at,
            signature,
        }
    }
}

// -- Serde wire format --------------------------------------------------------
//
// We mirror `Stamp` to a private `StampWire` struct so we can:
//   1. Emit `$type` in JSON/YAML output for AT-proto compat.
//   2. Reject documents whose `$type` doesn't match on input.
//
// Field order in the struct fixes serialization order (serde_yaml is stable).

#[derive(Serialize, Deserialize)]
struct StampWire {
    #[serde(rename = "$type")]
    type_id: String,
    signer: Did,
    act: StampAct,
    #[serde(rename = "docHash")]
    doc_hash: DocHash,
    #[serde(rename = "docFilename", default, skip_serializing_if = "Option::is_none")]
    doc_filename: Option<String>,
    #[serde(rename = "stampedAt")]
    stamped_at: DateTime<Utc>,
    signature: Signature,
}

impl Serialize for Stamp {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        StampWire {
            type_id: Self::TYPE_ID.to_string(),
            signer: self.signer.clone(),
            act: self.act,
            doc_hash: self.doc_hash.clone(),
            doc_filename: self.doc_filename.clone(),
            stamped_at: self.stamped_at,
            signature: self.signature.clone(),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Stamp {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let wire = StampWire::deserialize(d)?;
        if wire.type_id != Self::TYPE_ID {
            return Err(serde::de::Error::custom(format!(
                "expected $type {}, got {}",
                Self::TYPE_ID,
                wire.type_id
            )));
        }
        Ok(Stamp {
            signer: wire.signer,
            act: wire.act,
            doc_hash: wire.doc_hash,
            doc_filename: wire.doc_filename,
            stamped_at: wire.stamped_at,
            signature: wire.signature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixture_stamp() -> Stamp {
        Stamp::new(
            Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            StampAct::Attest,
            DocHash::from_bytes([0xab; 32]),
            Some("chapter-3.md".into()),
            Utc.with_ymd_and_hms(2026, 4, 30, 14, 25, 0).unwrap(),
            Signature::from_bytes([0x42; 64]),
        )
    }

    #[test]
    fn stamp_roundtrip_yaml() {
        let s = fixture_stamp();
        let yaml = serde_yaml::to_string(&s).unwrap();
        let back: Stamp = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn stamp_emits_type_field() {
        let yaml = serde_yaml::to_string(&fixture_stamp()).unwrap();
        assert!(yaml.contains("$type: tech.equanimi.secretariat.stamp"));
    }

    #[test]
    fn stamp_rejects_wrong_type() {
        let bad = "$type: wrong.type\nsigner: did:web:rafa.equanimi.tech\nact: attest\ndocHash: sha256:0000000000000000000000000000000000000000000000000000000000000000\nstampedAt: 2026-04-30T14:25:00Z\nsignature: ed25519:QkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkI=\n";
        let r: Result<Stamp, _> = serde_yaml::from_str(bad);
        assert!(r.is_err());
    }
}
