//! Application — orchestrates use cases (composition of domain + ports).

pub mod compose_envelope;
pub mod contact_ops;
pub mod delivery_policy;
pub mod inbox_ops;
pub mod invite_ops;
pub mod process_correspondence_claims;
pub mod send_envelope;
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
pub use invite_ops::{
    claim_invite, create_invite, view_invite, InviteClaimed, InviteCreated, InviteError,
    InviteView, DEFAULT_INVITE_TTL_HOURS,
};
pub use process_correspondence_claims::{
    process_correspondence_claims, ClaimProcessError, ClaimProcessOutcome, CorrespondenceClaim,
    SkipReason,
};
pub use send_envelope::{send_stamped_envelope, SendError, SendOutcome};
pub use stamp_document::{stamp_document, StampError, StampOutcome};
pub use verify_document::{verify_document, VerifyError, VerifyOutcome};
