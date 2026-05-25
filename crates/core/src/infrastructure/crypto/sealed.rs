//! Body encryption — sealed-box AEAD using X25519 ECDH + XChaCha20-Poly1305.
//!
//! ## Wire format
//!
//! ```text
//! x25519:<base64(ephemeral_pubkey)>:<base64(nonce)>:<base64(ciphertext)>
//! ```
//!
//! - `ephemeral_pubkey` — 32 bytes, X25519
//! - `nonce` — 24 bytes, XChaCha20-Poly1305
//! - `ciphertext` — variable; includes the 16-byte Poly1305 authentication
//!   tag at the tail (combined-mode AEAD output)
//!
//! ## Key conversion (ed25519 → x25519)
//!
//! - **public:** `verifying_key.to_montgomery()` — birational map from
//!   Edwards form (used by ed25519) to Montgomery form (used by X25519).
//! - **secret:** `SHA-512(seed)[..32]` with the standard X25519 clamping
//!   (RFC 7748 §5: clear bottom 3 bits, clear top bit, set bit 254).
//!
//! This matches libsodium's `crypto_sign_ed25519_{pk,sk}_to_curve25519`
//! semantics — the on-the-wire bytes are interoperable.
//!
//! ## Why no HKDF on the shared secret
//!
//! The X25519 shared secret is 32 uniform-random bytes. We use it directly
//! as the XChaCha20 key. Acceptable because:
//!
//! 1. A fresh ephemeral keypair is generated per [`seal`] call (no key
//!    reuse across messages — different ephemeral, different shared, different
//!    XChaCha20 key).
//! 2. The surrounding envelope authenticates with a separate ed25519
//!    signature over the ciphertext bytes (see the application layer).
//!
//! HKDF would add another dependency for negligible security gain in this
//! configuration.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha512};
use thiserror::Error;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum SealError {
    #[error("XChaCha20-Poly1305 encryption failed")]
    Encryption,
}

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("authentication / decryption failed (wrong key, tampered ciphertext, or wrong nonce)")]
    AuthFailure,
}

#[derive(Debug, Error)]
pub enum SealedBoxParseError {
    #[error("sealed box must start with `x25519:`")]
    MissingScheme,
    #[error("sealed box must have exactly 3 base64 components separated by `:` (got {0})")]
    WrongComponentCount(usize),
    #[error("base64 decode failed: {0}")]
    InvalidBase64(#[from] base64::DecodeError),
    #[error("ephemeral pubkey must be 32 bytes (got {0})")]
    BadEphemeralLength(usize),
    #[error("nonce must be 24 bytes (got {0})")]
    BadNonceLength(usize),
}

// ---------------------------------------------------------------------------
// Key conversion (ed25519 → x25519)
// ---------------------------------------------------------------------------

/// Convert an ed25519 verifying key to an X25519 public key (Montgomery).
pub fn pubkey_to_x25519(verifying_key: &VerifyingKey) -> X25519PublicKey {
    X25519PublicKey::from(verifying_key.to_montgomery().to_bytes())
}

/// Convert an ed25519 signing key to an X25519 static secret with RFC 7748 §5
/// clamping applied. Matches libsodium semantics.
pub fn signing_to_x25519(signing_key: &SigningKey) -> X25519StaticSecret {
    let h = Sha512::digest(signing_key.as_bytes());
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&h[..32]);
    // RFC 7748 §5 clamping for X25519 secrets.
    bytes[0] &= 248;
    bytes[31] &= 127;
    bytes[31] |= 64;
    X25519StaticSecret::from(bytes)
}

// ---------------------------------------------------------------------------
// SealedBox
// ---------------------------------------------------------------------------

/// Output of [`seal`]. Owns the ephemeral pubkey, nonce, and ciphertext+tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedBox {
    pub ephemeral_pubkey: [u8; 32],
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>, // includes the 16-byte Poly1305 tag at the tail
}

impl SealedBox {
    /// Wire-format string: `x25519:<b64-eph>:<b64-nonce>:<b64-ct>`.
    pub fn to_wire_string(&self) -> String {
        format!(
            "x25519:{}:{}:{}",
            B64.encode(self.ephemeral_pubkey),
            B64.encode(self.nonce),
            B64.encode(&self.ciphertext)
        )
    }

    pub fn parse_wire_string(s: &str) -> Result<Self, SealedBoxParseError> {
        let body = s
            .strip_prefix("x25519:")
            .ok_or(SealedBoxParseError::MissingScheme)?;
        let parts: Vec<&str> = body.split(':').collect();
        if parts.len() != 3 {
            return Err(SealedBoxParseError::WrongComponentCount(parts.len()));
        }
        let eph = B64.decode(parts[0])?;
        let nonce = B64.decode(parts[1])?;
        let ct = B64.decode(parts[2])?;
        if eph.len() != 32 {
            return Err(SealedBoxParseError::BadEphemeralLength(eph.len()));
        }
        if nonce.len() != 24 {
            return Err(SealedBoxParseError::BadNonceLength(nonce.len()));
        }
        let mut eph_arr = [0u8; 32];
        eph_arr.copy_from_slice(&eph);
        let mut nonce_arr = [0u8; 24];
        nonce_arr.copy_from_slice(&nonce);
        Ok(Self {
            ephemeral_pubkey: eph_arr,
            nonce: nonce_arr,
            ciphertext: ct,
        })
    }

    /// Canonical bytes for hashing. Concatenates `ephemeral_pubkey || nonce ||
    /// ciphertext`. Used by the envelope hash invariant: when an envelope's
    /// body is encrypted, `docHash` covers these bytes (so the ed25519
    /// signature authenticates the wire-side bytes the recipient receives).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 24 + self.ciphertext.len());
        out.extend_from_slice(&self.ephemeral_pubkey);
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&self.ciphertext);
        out
    }
}

// ---------------------------------------------------------------------------
// seal / open
// ---------------------------------------------------------------------------

/// Encrypt `plaintext` to `recipient_pubkey`. Generates a fresh ephemeral
/// X25519 keypair and a random nonce per call.
pub fn seal(plaintext: &[u8], recipient_pubkey: &X25519PublicKey) -> Result<SealedBox, SealError> {
    let mut rng = OsRng;
    let ephemeral_secret = X25519StaticSecret::random_from_rng(rng);
    let ephemeral_pubkey = X25519PublicKey::from(&ephemeral_secret);
    let shared = ephemeral_secret.diffie_hellman(recipient_pubkey);

    let mut nonce_bytes = [0u8; 24];
    rng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);

    let cipher = XChaCha20Poly1305::new(shared.as_bytes().into());
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| SealError::Encryption)?;

    Ok(SealedBox {
        ephemeral_pubkey: ephemeral_pubkey.to_bytes(),
        nonce: nonce_bytes,
        ciphertext,
    })
}

/// Decrypt a [`SealedBox`] with the recipient's X25519 secret. Authenticates
/// the ciphertext via Poly1305 — failure means wrong key, tampered bytes, or
/// wrong nonce, indistinguishable.
pub fn open(sealed: &SealedBox, my_secret: &X25519StaticSecret) -> Result<Vec<u8>, OpenError> {
    let eph_pub = X25519PublicKey::from(sealed.ephemeral_pubkey);
    let shared = my_secret.diffie_hellman(&eph_pub);
    let nonce = XNonce::from_slice(&sealed.nonce);
    let cipher = XChaCha20Poly1305::new(shared.as_bytes().into());
    cipher
        .decrypt(nonce, sealed.ciphertext.as_slice())
        .map_err(|_| OpenError::AuthFailure)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn fresh_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn seal_open_roundtrip() {
        let k = fresh_key();
        let pk = pubkey_to_x25519(&k.verifying_key());
        let sk = signing_to_x25519(&k);

        let plaintext = b"chapter 7 - staff vs. tools";
        let sealed = seal(plaintext, &pk).unwrap();
        let opened = open(&sealed, &sk).unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn open_with_wrong_key_fails() {
        let k1 = fresh_key();
        let k2 = fresh_key();
        let pk = pubkey_to_x25519(&k1.verifying_key());
        let sk_wrong = signing_to_x25519(&k2);

        let sealed = seal(b"secret", &pk).unwrap();
        assert!(matches!(
            open(&sealed, &sk_wrong),
            Err(OpenError::AuthFailure)
        ));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let k = fresh_key();
        let pk = pubkey_to_x25519(&k.verifying_key());
        let sk = signing_to_x25519(&k);

        let mut sealed = seal(b"hello", &pk).unwrap();
        sealed.ciphertext[0] ^= 1;
        assert!(matches!(open(&sealed, &sk), Err(OpenError::AuthFailure)));
    }

    #[test]
    fn tampered_nonce_fails() {
        let k = fresh_key();
        let pk = pubkey_to_x25519(&k.verifying_key());
        let sk = signing_to_x25519(&k);

        let mut sealed = seal(b"hello", &pk).unwrap();
        sealed.nonce[0] ^= 1;
        assert!(matches!(open(&sealed, &sk), Err(OpenError::AuthFailure)));
    }

    #[test]
    fn each_seal_uses_fresh_ephemeral_and_nonce() {
        let k = fresh_key();
        let pk = pubkey_to_x25519(&k.verifying_key());
        let s1 = seal(b"x", &pk).unwrap();
        let s2 = seal(b"x", &pk).unwrap();
        assert_ne!(s1.ephemeral_pubkey, s2.ephemeral_pubkey);
        assert_ne!(s1.nonce, s2.nonce);
        assert_ne!(s1.ciphertext, s2.ciphertext);
    }

    #[test]
    fn pubkey_conversion_is_deterministic() {
        let k = fresh_key();
        let p1 = pubkey_to_x25519(&k.verifying_key());
        let p2 = pubkey_to_x25519(&k.verifying_key());
        assert_eq!(p1.as_bytes(), p2.as_bytes());
    }

    #[test]
    fn signing_conversion_applies_clamping() {
        let k = fresh_key();
        let sk = signing_to_x25519(&k);
        let bytes = sk.to_bytes();
        assert_eq!(bytes[0] & 0b0000_0111, 0, "bottom 3 bits must be zero");
        assert_eq!(bytes[31] & 0b1000_0000, 0, "top bit must be zero");
        assert_eq!(bytes[31] & 0b0100_0000, 0b0100_0000, "bit 254 must be set");
    }

    #[test]
    fn cross_principal_seal_open() {
        // Real two-party scenario: rafa seals to marcelo, marcelo opens.
        let rafa = fresh_key();
        let marcelo = fresh_key();
        let marcelo_pubkey = pubkey_to_x25519(&marcelo.verifying_key());
        let marcelo_secret = signing_to_x25519(&marcelo);

        let plaintext = b"hello marcelo";
        let sealed = seal(plaintext, &marcelo_pubkey).unwrap();
        let opened = open(&sealed, &marcelo_secret).unwrap();
        assert_eq!(opened, plaintext);

        // Rafa's own secret can't open her own seal-to-marcelo (correct).
        let rafa_secret = signing_to_x25519(&rafa);
        assert!(matches!(
            open(&sealed, &rafa_secret),
            Err(OpenError::AuthFailure)
        ));
    }

    #[test]
    fn empty_plaintext_roundtrip() {
        let k = fresh_key();
        let pk = pubkey_to_x25519(&k.verifying_key());
        let sk = signing_to_x25519(&k);
        let sealed = seal(b"", &pk).unwrap();
        // Even an empty plaintext produces a 16-byte Poly1305 tag.
        assert_eq!(sealed.ciphertext.len(), 16);
        let opened = open(&sealed, &sk).unwrap();
        assert!(opened.is_empty());
    }

    #[test]
    fn large_plaintext_roundtrip() {
        let k = fresh_key();
        let pk = pubkey_to_x25519(&k.verifying_key());
        let sk = signing_to_x25519(&k);
        let plaintext = vec![0xAB_u8; 1_000_000];
        let sealed = seal(&plaintext, &pk).unwrap();
        let opened = open(&sealed, &sk).unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn wire_format_roundtrip() {
        let k = fresh_key();
        let pk = pubkey_to_x25519(&k.verifying_key());
        let sk = signing_to_x25519(&k);

        let sealed = seal(b"wire test", &pk).unwrap();
        let s = sealed.to_wire_string();
        assert!(s.starts_with("x25519:"));
        let parsed = SealedBox::parse_wire_string(&s).unwrap();
        assert_eq!(parsed, sealed);
        let opened = open(&parsed, &sk).unwrap();
        assert_eq!(opened, b"wire test");
    }

    #[test]
    fn wire_format_rejects_missing_scheme() {
        assert!(matches!(
            SealedBox::parse_wire_string("foo:bar:baz"),
            Err(SealedBoxParseError::MissingScheme)
        ));
    }

    #[test]
    fn wire_format_rejects_wrong_component_count() {
        assert!(matches!(
            SealedBox::parse_wire_string("x25519:onlyone"),
            Err(SealedBoxParseError::WrongComponentCount(1))
        ));
    }

    #[test]
    fn wire_format_rejects_bad_ephemeral_length() {
        let s = format!(
            "x25519:{}:{}:{}",
            B64.encode([0u8; 31]), // should be 32
            B64.encode([0u8; 24]),
            B64.encode([0u8; 16]),
        );
        assert!(matches!(
            SealedBox::parse_wire_string(&s),
            Err(SealedBoxParseError::BadEphemeralLength(31))
        ));
    }

    #[test]
    fn wire_format_rejects_bad_nonce_length() {
        let s = format!(
            "x25519:{}:{}:{}",
            B64.encode([0u8; 32]),
            B64.encode([0u8; 23]), // should be 24
            B64.encode([0u8; 16]),
        );
        assert!(matches!(
            SealedBox::parse_wire_string(&s),
            Err(SealedBoxParseError::BadNonceLength(23))
        ));
    }

    #[test]
    fn canonical_bytes_concatenates_in_order() {
        let sealed = SealedBox {
            ephemeral_pubkey: [1u8; 32],
            nonce: [2u8; 24],
            ciphertext: vec![3u8; 16],
        };
        let bytes = sealed.canonical_bytes();
        assert_eq!(bytes.len(), 32 + 24 + 16);
        assert_eq!(&bytes[..32], &[1u8; 32]);
        assert_eq!(&bytes[32..56], &[2u8; 24]);
        assert_eq!(&bytes[56..], &[3u8; 16]);
    }
}
