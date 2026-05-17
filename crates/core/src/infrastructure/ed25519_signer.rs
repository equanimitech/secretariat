//! Ed25519-based [`Signer`] implementation, gated by a [`BiometricGate`].
//!
//! Gates that ship:
//! - [`AlwaysAllowGate`] / [`AlwaysDenyGate`] — for unit tests; never prompts.
//! - [`super::NativeBiometricGate`] — in-process: macOS LAContext / Windows
//!   Hello. Lives in `infrastructure/native_biometric.rs`.

use ed25519_dalek::{Signer as _, SigningKey};
use thiserror::Error;

use crate::domain::{Did, DocHash, Signature};
use crate::ports::{Signer, SignerError};

/// Pure capability for "verify a human is present at the keyboard."
///
/// The gate has no knowledge of what's being signed beyond a UI-facing reason
/// string. Implementations must NOT have access to the signing key.
pub trait BiometricGate {
    fn prompt(&self, reason: &str) -> Result<(), SignerError>;
}

/// Test-only gate that always authorizes. Intentionally annotated so the
/// CLI can require an explicit `--allow-test-biometrics` flag at the call
/// site (see plan: "Test infrastructure" section).
#[derive(Debug, Default, Clone, Copy)]
pub struct AlwaysAllowGate;

impl BiometricGate for AlwaysAllowGate {
    fn prompt(&self, _reason: &str) -> Result<(), SignerError> {
        Ok(())
    }
}

/// Test-only gate that always refuses. Useful for adversarial-test fixtures.
#[derive(Debug, Default, Clone, Copy)]
pub struct AlwaysDenyGate;

impl BiometricGate for AlwaysDenyGate {
    fn prompt(&self, _reason: &str) -> Result<(), SignerError> {
        Err(SignerError::BiometricRefused)
    }
}

#[derive(Debug, Error)]
pub enum Ed25519SignerError {
    #[error("DID and signing key do not correspond (DID was issued for a different key)")]
    DidKeyMismatch,
}

pub struct Ed25519Signer<B: BiometricGate> {
    did: Did,
    signing_key: SigningKey,
    biometric: B,
}

impl<B: BiometricGate> Ed25519Signer<B> {
    pub fn new(did: Did, signing_key: SigningKey, biometric: B) -> Self {
        Self {
            did,
            signing_key,
            biometric,
        }
    }
}

impl<B: BiometricGate> Signer for Ed25519Signer<B> {
    fn signer_did(&self) -> &Did {
        &self.did
    }

    fn sign(&self, doc_hash: &DocHash, reason: &str) -> Result<Signature, SignerError> {
        self.biometric.prompt(reason)?;
        let sig = self.signing_key.sign(doc_hash.as_bytes());
        Ok(Signature::from_bytes(sig.to_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::keys::generate_keypair;
    use ed25519_dalek::Verifier;

    #[test]
    fn signs_with_always_allow_gate_and_signature_verifies() {
        let key = generate_keypair();
        let verifying = key.verifying_key();
        let signer = Ed25519Signer::new(
            Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            key,
            AlwaysAllowGate,
        );
        let hash = DocHash::from_bytes([0x11; 32]);
        let sig = signer.sign(&hash, "test").unwrap();

        let dalek_sig = ed25519_dalek::Signature::from_bytes(sig.as_bytes());
        verifying.verify(hash.as_bytes(), &dalek_sig).unwrap();
    }

    #[test]
    fn always_deny_gate_blocks_signing() {
        let key = generate_keypair();
        let signer = Ed25519Signer::new(
            Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            key,
            AlwaysDenyGate,
        );
        let r = signer.sign(&DocHash::from_bytes([0u8; 32]), "test");
        assert!(matches!(r, Err(SignerError::BiometricRefused)));
    }
}
