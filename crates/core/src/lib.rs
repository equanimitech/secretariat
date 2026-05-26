//! Secretariat core — cryptographic attestation primitives, layered DDD shape.
//!
//! The bounded context: human attestation of AI-mediated documents that travel
//! between principals via any transport, verifiable offline.
//!
//! Layer separation:
//! - `domain`         — pure business logic, no IO. Aggregates enforce invariants.
//! - `ports`          — traits the domain depends on (Signer, DidResolver).
//! - `infrastructure` — concrete adapters for ports + persistence.
//!
//! Application use cases (`stamp_document`, `verify_document`) live in `application`.

pub mod application;
pub mod codec;
pub mod domain;
pub mod infrastructure;
pub mod ports;

// Curated re-exports for callers (CLI, Tauri, future MCP server).
pub use application::{stamp_document, verify_document, StampOutcome, VerifyOutcome};
pub use domain::{
    AttestedDocument, Did, DisplayName, DocHash, EncryptionScheme, Envelope, RelayEndpoint,
    Signature, Stamp, StampAct,
};
pub use ports::{DidResolver, Signer};
