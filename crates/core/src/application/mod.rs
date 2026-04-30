//! Application — orchestrates use cases (composition of domain + ports).

pub mod compose_envelope;
pub mod stamp_document;
pub mod verify_document;

pub use compose_envelope::{compose_envelope, ComposeError, ComposeRequest};
pub use stamp_document::{stamp_document, StampError, StampOutcome};
pub use verify_document::{verify_document, VerifyError, VerifyOutcome};
