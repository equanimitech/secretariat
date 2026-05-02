//! Challenge-response authentication for inbox-pull.
//!
//! Recipients prove ownership of their DID by signing a server-issued nonce
//! with their ed25519 key. On success the relay issues a short-lived bearer
//! token, used in the `Authorization: Bearer <token>` header for subsequent
//! `GET /v0/inbox/{did}` requests.
//!
//! No password is ever stored or transmitted. The relay never sees a secret.

use std::collections::HashMap;
use std::sync::RwLock;

use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature as DalekSig, Verifier, VerifyingKey};
use secretariat_core::Did;
use thiserror::Error;
use uuid::Uuid;

const CHALLENGE_TTL_SECS: i64 = 60;
pub const SESSION_TTL_SECS: i64 = 3600;

/// Domain-separation tag for what the recipient is signing.
const AUTH_DOMAIN: &[u8] = b"secretariat-relay-auth:v0:";

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("no pending challenge for DID and nonce")]
    UnknownChallenge,
    #[error("challenge expired")]
    ChallengeExpired,
    #[error("DID is not registered with this relay")]
    UnregisteredDid,
    #[error("signature verification failed")]
    BadSignature,
    #[error("malformed signature: {0}")]
    MalformedSignature(String),
    #[error("session token unknown or expired")]
    InvalidToken,
}

#[derive(Debug, Clone)]
pub struct Challenge {
    pub did: Did,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub did: Did,
    pub expires_at: DateTime<Utc>,
}

#[derive(Default)]
pub struct AuthState {
    challenges: RwLock<HashMap<String, Challenge>>,
    sessions: RwLock<HashMap<String, Session>>,
}

impl AuthState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Issue a fresh nonce for `did`. Caller is responsible for confirming
    /// the DID is registered (we don't enforce that here so that the relay
    /// can fail later with a clearer error in the answer step).
    pub fn issue_challenge(&self, did: Did, now: DateTime<Utc>) -> String {
        let nonce = Uuid::new_v4().simple().to_string();
        let expires_at = now + Duration::seconds(CHALLENGE_TTL_SECS);
        self.challenges
            .write()
            .unwrap()
            .insert(nonce.clone(), Challenge { did, expires_at });
        nonce
    }

    /// Verify the signed nonce. On success, issues a session token and
    /// returns it. Consumes the challenge regardless of outcome.
    pub fn verify_and_issue_token(
        &self,
        did: &Did,
        nonce: &str,
        signature_bytes: &[u8],
        pubkey: &VerifyingKey,
        now: DateTime<Utc>,
    ) -> Result<String, AuthError> {
        let challenge = self
            .challenges
            .write()
            .unwrap()
            .remove(nonce)
            .ok_or(AuthError::UnknownChallenge)?;
        if challenge.expires_at < now {
            return Err(AuthError::ChallengeExpired);
        }
        if &challenge.did != did {
            return Err(AuthError::UnknownChallenge);
        }

        let sig: [u8; 64] = signature_bytes
            .try_into()
            .map_err(|_| AuthError::MalformedSignature(format!(
                "expected 64 bytes, got {}",
                signature_bytes.len()
            )))?;
        let dalek_sig = DalekSig::from_bytes(&sig);

        let mut to_verify = AUTH_DOMAIN.to_vec();
        to_verify.extend_from_slice(nonce.as_bytes());
        pubkey
            .verify(&to_verify, &dalek_sig)
            .map_err(|_| AuthError::BadSignature)?;

        let token = Uuid::new_v4().simple().to_string();
        let session = Session {
            did: did.clone(),
            expires_at: now + Duration::seconds(SESSION_TTL_SECS),
        };
        self.sessions.write().unwrap().insert(token.clone(), session);
        Ok(token)
    }

    /// Resolve a bearer token to its DID. Returns error if unknown or expired.
    pub fn validate_token(&self, token: &str, now: DateTime<Utc>) -> Result<Did, AuthError> {
        let session = self
            .sessions
            .read()
            .unwrap()
            .get(token)
            .cloned()
            .ok_or(AuthError::InvalidToken)?;
        if session.expires_at < now {
            // Clean up while we're here.
            self.sessions.write().unwrap().remove(token);
            return Err(AuthError::InvalidToken);
        }
        Ok(session.did)
    }

    /// The bytes a client must sign to answer a challenge. Exposed so the
    /// client adapter (and integration tests) can reproduce the exact input.
    pub fn auth_input(nonce: &str) -> Vec<u8> {
        let mut v = AUTH_DOMAIN.to_vec();
        v.extend_from_slice(nonce.as_bytes());
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;

    fn fresh_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    #[test]
    fn challenge_answer_roundtrip() {
        let state = AuthState::new();
        let key = fresh_key();
        let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        let now = Utc::now();

        let nonce = state.issue_challenge(did.clone(), now);
        let to_sign = AuthState::auth_input(&nonce);
        let sig = key.sign(&to_sign);

        let token = state
            .verify_and_issue_token(&did, &nonce, &sig.to_bytes(), &key.verifying_key(), now)
            .unwrap();

        let resolved = state.validate_token(&token, now).unwrap();
        assert_eq!(resolved, did);
    }

    #[test]
    fn answer_with_wrong_key_fails() {
        let state = AuthState::new();
        let alice = fresh_key();
        let bob = fresh_key();
        let alice_did = Did::from_ed25519_public_key(&alice.verifying_key().to_bytes());
        let now = Utc::now();

        let nonce = state.issue_challenge(alice_did.clone(), now);
        let to_sign = AuthState::auth_input(&nonce);
        let bob_sig = bob.sign(&to_sign);

        let r = state.verify_and_issue_token(
            &alice_did,
            &nonce,
            &bob_sig.to_bytes(),
            &alice.verifying_key(),
            now,
        );
        assert!(matches!(r, Err(AuthError::BadSignature)));
    }

    #[test]
    fn expired_challenge_fails() {
        let state = AuthState::new();
        let key = fresh_key();
        let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        let issued_at = Utc::now();

        let nonce = state.issue_challenge(did.clone(), issued_at);
        let to_sign = AuthState::auth_input(&nonce);
        let sig = key.sign(&to_sign);

        // Pretend a long time passed.
        let later = issued_at + Duration::seconds(CHALLENGE_TTL_SECS + 10);
        let r = state.verify_and_issue_token(
            &did,
            &nonce,
            &sig.to_bytes(),
            &key.verifying_key(),
            later,
        );
        assert!(matches!(r, Err(AuthError::ChallengeExpired)));
    }

    #[test]
    fn unknown_nonce_fails() {
        let state = AuthState::new();
        let key = fresh_key();
        let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        let now = Utc::now();

        let to_sign = AuthState::auth_input("ghost-nonce");
        let sig = key.sign(&to_sign);

        let r = state.verify_and_issue_token(
            &did,
            "ghost-nonce",
            &sig.to_bytes(),
            &key.verifying_key(),
            now,
        );
        assert!(matches!(r, Err(AuthError::UnknownChallenge)));
    }

    #[test]
    fn expired_token_fails() {
        let state = AuthState::new();
        let key = fresh_key();
        let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        let now = Utc::now();

        let nonce = state.issue_challenge(did.clone(), now);
        let to_sign = AuthState::auth_input(&nonce);
        let sig = key.sign(&to_sign);
        let token = state
            .verify_and_issue_token(&did, &nonce, &sig.to_bytes(), &key.verifying_key(), now)
            .unwrap();

        let later = now + Duration::seconds(SESSION_TTL_SECS + 1);
        let r = state.validate_token(&token, later);
        assert!(matches!(r, Err(AuthError::InvalidToken)));
    }
}
