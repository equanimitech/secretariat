//! `AttestedDocument` — the aggregate root.
//!
//! Holds the cross-entity invariant: `stamp.doc_hash == canonical_body_hash(body)`.
//! Construction performs the cheap (hash) check. Signature verification is
//! IO-bound (resolves the signer's DID document) and lives in the `verify_document`
//! application use case, not in the aggregate itself.

use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{DocHash, Stamp};

/// The aggregate. Once constructed, `stamp.doc_hash` is guaranteed to match
/// the hash of `body` under the canonicalization rules.
///
/// The aggregate is intentionally envelope-free: stamp verification depends
/// only on `(stamp, body)`. The advisory `$envelope` routing block (when
/// present in a file) is not part of the cryptographic invariant and is
/// parsed opaquely upstream — see `infrastructure::markdown`.
#[derive(Debug, Clone)]
pub struct AttestedDocument {
    pub stamp: Stamp,
    pub body: String,
}

#[derive(Debug, Error)]
pub enum DocumentInvariantError {
    #[error("stamp doc_hash does not match canonical body hash (claimed: {claimed}, computed: {computed})")]
    HashMismatch { claimed: DocHash, computed: DocHash },
}

impl AttestedDocument {
    /// Build an attested document from parts. Verifies the body hash matches
    /// the stamp's claim. Does NOT verify the signature against the signer's
    /// public key — that is the application layer's job.
    pub fn new(stamp: Stamp, body: String) -> Result<Self, DocumentInvariantError> {
        let computed = canonical_body_hash(&body);
        if stamp.doc_hash != computed {
            return Err(DocumentInvariantError::HashMismatch {
                claimed: stamp.doc_hash.clone(),
                computed,
            });
        }
        Ok(AttestedDocument { stamp, body })
    }

    /// The bytes the signer signed (the doc hash).
    pub fn signed_payload(&self) -> &[u8; 32] {
        self.stamp.doc_hash.as_bytes()
    }
}

/// Pure hash function over a body, applying canonicalization rules from
/// the wire-format spec (decision log #5):
///
/// - Strip a single leading BOM (`U+FEFF`) if present.
/// - Normalize line endings: CRLF → LF.
/// - Strip trailing whitespace from the body.
/// - Leading whitespace inside the body is preserved (heading position matters).
///
/// SHA-256 over the resulting UTF-8 bytes.
pub fn canonical_body_hash(body: &str) -> DocHash {
    let mut s = body;
    if let Some(stripped) = s.strip_prefix('\u{FEFF}') {
        s = stripped;
    }
    let normalized: String = if s.contains('\r') {
        s.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        s.to_string()
    };
    let trimmed = normalized.trim_end();

    let mut hasher = Sha256::new();
    hasher.update(trimmed.as_bytes());
    let bytes: [u8; 32] = hasher.finalize().into();
    DocHash::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Did, Signature, StampAct};
    use chrono::TimeZone;
    use chrono::Utc;

    fn stamp_for(hash: DocHash) -> Stamp {
        Stamp::new(
            Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            StampAct::Attest,
            hash,
            None,
            Utc.with_ymd_and_hms(2026, 4, 30, 14, 25, 0).unwrap(),
            Signature::from_bytes([0u8; 64]),
        )
    }

    #[test]
    fn hash_idempotent_under_crlf() {
        let lf = "# Hello\nworld\n";
        let crlf = "# Hello\r\nworld\r\n";
        assert_eq!(canonical_body_hash(lf), canonical_body_hash(crlf));
    }

    #[test]
    fn hash_idempotent_under_bom() {
        let plain = "# Hello\n";
        let with_bom = "\u{FEFF}# Hello\n";
        assert_eq!(canonical_body_hash(plain), canonical_body_hash(with_bom));
    }

    #[test]
    fn hash_idempotent_under_trailing_whitespace() {
        let a = "# Hello\nworld";
        let b = "# Hello\nworld\n\n  \t  \n";
        assert_eq!(canonical_body_hash(a), canonical_body_hash(b));
    }

    #[test]
    fn hash_distinguishes_different_content() {
        assert_ne!(canonical_body_hash("# A"), canonical_body_hash("# B"));
    }

    #[test]
    fn aggregate_construction_succeeds_with_matching_hash() {
        let body = "# Hello\n".to_string();
        let h = canonical_body_hash(&body);
        let s = stamp_for(h);
        assert!(AttestedDocument::new(s, body).is_ok());
    }

    #[test]
    fn aggregate_construction_fails_on_hash_mismatch() {
        let body = "# Hello\n".to_string();
        let wrong = DocHash::from_bytes([0xff; 32]);
        let s = stamp_for(wrong);
        let r = AttestedDocument::new(s, body);
        assert!(matches!(
            r,
            Err(DocumentInvariantError::HashMismatch { .. })
        ));
    }
}
