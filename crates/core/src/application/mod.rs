//! Application — orchestrates use cases (composition of domain + ports).

pub mod compose_envelope;
pub mod contact_ops;
pub mod delivery_policy;
pub mod inbox_ops;
pub mod stamp_document;
pub mod verify_document;

pub use compose_envelope::{compose_envelope, ComposeError, ComposeRequest};
pub use contact_ops::{
    add_contact, find_by_did, find_by_slug, list_contacts, remove_contact, ContactOpError,
};
pub use delivery_policy::{decide_poll, CadenceConfig, CadenceConfigError, PollDecision};
pub use inbox_ops::{
    list_inbox_files, list_outbox_files, read_envelope, InboxOpError, ListedEnvelope, ReadResult,
};
pub use stamp_document::{stamp_document, StampError, StampOutcome};
pub use verify_document::{verify_document, VerifyError, VerifyOutcome};
