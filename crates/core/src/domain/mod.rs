//! Pure domain: bounded context = human attestation of AI-mediated documents.
//!
//! No IO. Types are the architectural guardrails — illegal states unrepresentable.
//! Aggregates (`AttestedDocument`) enforce cross-entity invariants at construction.

mod acts;
mod attention_envelope;
mod attested_document;
mod contact;
mod envelope;
mod identity;
mod stamp;

pub use acts::{EnvelopeDepth, EnvelopeUrgency, StampAct};
pub use attention_envelope::AttentionEnvelope;
pub use attested_document::{canonical_body_hash, AttestedDocument, DocumentInvariantError};
pub use contact::{
    Contact, DisplayName, DisplayNameParseError, RelayEndpoint, RelayEndpointParseError,
};
pub use envelope::{EncryptionScheme, Envelope, EnvelopeBuilder};
pub use identity::{Did, DidMethod, DidParseError, DocHash, Signature, SignatureParseError};
pub use stamp::Stamp;
