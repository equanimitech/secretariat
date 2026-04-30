//! Pure encoding helpers shared across layers (no IO, no clock).
//!
//! Multicodec / multibase plumbing for ed25519 keys, used by:
//! - `domain::identity::Did` to extract the embedded key from a `did:key` value.
//! - `infrastructure::keys` to write the `publicKeyMultibase` field of a
//!   hosted `did:web` document.
//! - `infrastructure::did_web_resolver` to decode the same field on input.

use thiserror::Error;

/// Multicodec varint prefix for `ed25519-pub` (codepoint `0xed`, encoded as
/// the two-byte LEB128 sequence `0xed 0x01`).
const ED25519_PUB_MULTICODEC: [u8; 2] = [0xed, 0x01];

#[derive(Debug, Error)]
pub enum CodecError {
    #[error("multibase decode failed: {0}")]
    Multibase(String),
    #[error("expected ed25519-pub multicodec prefix")]
    WrongMulticodec,
    #[error("expected 32-byte ed25519 public key, got {0} bytes")]
    WrongLength(usize),
}

/// Encode a 32-byte ed25519 verifying key as `z`-prefixed base58btc multibase
/// with the `ed25519-pub` multicodec prefix. Matches the W3C
/// `Ed25519VerificationKey2020` `publicKeyMultibase` and `did:key` formats.
pub fn encode_ed25519_multibase(key: &[u8; 32]) -> String {
    let mut payload = Vec::with_capacity(2 + 32);
    payload.extend_from_slice(&ED25519_PUB_MULTICODEC);
    payload.extend_from_slice(key);
    multibase::encode(multibase::Base::Base58Btc, &payload)
}

/// Inverse of [`encode_ed25519_multibase`]. Returns the raw 32-byte ed25519
/// verifying key.
pub fn decode_ed25519_multibase(input: &str) -> Result<[u8; 32], CodecError> {
    let (_base, bytes) =
        multibase::decode(input).map_err(|e| CodecError::Multibase(e.to_string()))?;
    if bytes.len() != ED25519_PUB_MULTICODEC.len() + 32 {
        return Err(CodecError::WrongLength(bytes.len()));
    }
    if bytes[..ED25519_PUB_MULTICODEC.len()] != ED25519_PUB_MULTICODEC {
        return Err(CodecError::WrongMulticodec);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes[ED25519_PUB_MULTICODEC.len()..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let key = [0xab_u8; 32];
        let s = encode_ed25519_multibase(&key);
        assert!(s.starts_with('z'));
        let back = decode_ed25519_multibase(&s).unwrap();
        assert_eq!(back, key);
    }

    #[test]
    fn rejects_wrong_length() {
        // base58btc encoding of just `0xed 0x01` (no key bytes) should fail.
        let s = multibase::encode(multibase::Base::Base58Btc, [0xed, 0x01]);
        assert!(matches!(
            decode_ed25519_multibase(&s),
            Err(CodecError::WrongLength(_))
        ));
    }

    #[test]
    fn rejects_wrong_multicodec() {
        // 32 bytes prefixed with the wrong multicodec.
        let mut payload = vec![0x01, 0x02];
        payload.extend_from_slice(&[0u8; 32]);
        let s = multibase::encode(multibase::Base::Base58Btc, payload);
        assert!(matches!(
            decode_ed25519_multibase(&s),
            Err(CodecError::WrongMulticodec)
        ));
    }
}
