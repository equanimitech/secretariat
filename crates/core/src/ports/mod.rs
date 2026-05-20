//! Ports — traits the domain depends on. Implementations live in `infrastructure`.
//!
//! Step 2 of the implementation plan. Populated below.

use thiserror::Error;

use crate::domain::{Did, DocHash, Signature};

// -- Signer -------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum SignerError {
    #[error("biometric authentication failed or was cancelled")]
    BiometricRefused,
    #[error("signing key not available: {0}")]
    KeyUnavailable(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("internal: {0}")]
    Internal(String),
}

/// Produces detached ed25519 signatures over a doc hash, gated by a humanness check.
pub trait Signer {
    fn signer_did(&self) -> &Did;
    fn sign(&self, doc_hash: &DocHash, reason: &str) -> Result<Signature, SignerError>;
}

// -- DidResolver --------------------------------------------------------------

/// Result of resolving a DID to a usable verifying key set.
#[derive(Debug, Clone)]
pub struct ResolvedDid {
    pub did: Did,
    /// One or more 32-byte ed25519 verifying keys listed under `assertionMethod`.
    pub stamp_public_keys: Vec<[u8; 32]>,
    /// The raw DID document JSON, retained for caching and future fields.
    pub raw_document: serde_json::Value,
}

#[derive(Debug, Clone, Error)]
pub enum DidResolutionError {
    #[error("did document not found at {url}")]
    NotFound { url: String },
    #[error("did document is malformed: {0}")]
    Malformed(String),
    #[error("no ed25519 verification method present in did document")]
    NoEd25519Key,
    #[error("network error: {0}")]
    Network(String),
}

/// Resolves a `Did` to its document. Implementations may cache.
pub trait DidResolver {
    fn resolve(&self, did: &Did) -> Result<ResolvedDid, DidResolutionError>;
}


// -- Cognition ----------------------------------------------------------------
//
// Three sibling ports under one bounded responsibility. See the
// `cognition` submodule for full docs and per-port rationale.

pub mod cognition;
pub use cognition::{
    CognitionError, CognitionLaunching, CognitionRouting, CognitionSession,
    LaunchPlan, LauncherError, RouteSuggestion, SessionError, SessionEvent,
    SessionRef,
};
