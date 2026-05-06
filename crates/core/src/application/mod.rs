//! Application — orchestrates use cases (composition of domain + ports).

pub mod capture_ops;
pub mod compose_envelope;
pub mod contact_ops;
pub mod contextify_capture;
pub mod delivery_policy;
pub mod inbox_actions;
pub mod inbox_ops;
pub mod invite_ops;
pub mod process_correspondence_claims;
pub mod review_queue;
pub mod send_envelope;
pub mod sync;
pub mod stamp_document;
pub mod verify_document;

pub use capture_ops::{capture_to_queue, CaptureError, CaptureRequest};
pub use compose_envelope::{compose_envelope, ComposeError, ComposeRequest};
pub use contact_ops::{
    add_contact, find_by_did, find_by_slug, list_contacts, remove_contact, ContactOpError,
};
pub use contextify_capture::{
    contextify_capture, try_contextify_after_capture, ContextifyError, ContextifyOutcome,
    ContextifySkipReason, ROUTABLE_QUEUE,
};
pub use delivery_policy::{decide_poll, CadenceConfig, CadenceConfigError, PollDecision};
pub use inbox_actions::{archive_envelope, defer_envelope, InboxActionError};
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
pub use review_queue::{list_local_queues, list_outbox_queue, list_review_queue};
pub use send_envelope::{send_stamped_envelope, SendError, SendOutcome};
pub use sync::{sync_now, RelaySyncReport, SyncError, SyncOutcome};
pub use stamp_document::{stamp_document, StampError, StampOutcome};
pub use verify_document::{verify_document, VerifyError, VerifyOutcome};
