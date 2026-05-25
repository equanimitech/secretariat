//! Application — orchestrates use cases (composition of domain + ports).

pub mod ag_extract;
pub mod agent_ops;
pub mod capture_ops;
pub mod channels_ops;
pub mod compose_envelope;
pub mod contract_ops;
pub mod org_ops;
pub mod contact_ops;
pub mod contextify_capture;
pub mod delivery_policy;
pub mod inbox_actions;
pub mod inbox_ops;
pub mod invite_ops;
pub mod launch_channel;
pub mod process_correspondence_claims;
pub mod review_queue;
pub mod send_envelope;
pub mod sync;
pub mod stamp_document;
pub mod verify_document;

pub use capture_ops::{
    capture_to_queue, capture_to_queue_with_ag, channels_root_for, CaptureError, CaptureRequest,
};
pub use channels_ops::{
    create_channel, delete_channel, list_channels, read_channel, ChannelEnvelope, ChannelOpError,
    ChannelSummary,
};
pub use agent_ops::{add_agent, list_agents, remove_agent, rotate_agent, AgentOpsError};
pub use org_ops::{create_org, delete_org, list_orgs, show_org, OrgOpsError};
pub use compose_envelope::{
    compose_envelope, compose_envelope_with_ag, ComposeError, ComposeRequest,
};
pub use contract_ops::{
    get_channel_contract, get_org_contract, resolve_channel_contract, set_channel_contract,
    set_org_contract, ContractLevel, ContractOpsError, ContractPatch, ContractView, PatchField,
    ResolvedContract,
};
pub use contact_ops::{
    add_contact, find_by_did, find_by_slug, list_contacts, remove_contact, ContactOpError,
};
pub use contextify_capture::{
    contextify_capture, try_contextify_after_capture, ContextifyError, ContextifyOutcome,
    ContextifySkipReason, ROUTABLE_QUEUE,
};
pub use delivery_policy::{decide_poll, CadenceConfig, CadenceConfigError, PollDecision};
pub use inbox_actions::{
    archive_envelope, defer_envelope, unarchive_envelope, InboxActionError,
};
pub use inbox_ops::{
    list_draft_files, list_inbox_files, read_envelope, InboxOpError, ListedEnvelope, ReadResult,
};
// Bridge: outbox-rename slice in flight (`docs/pitches/2026-05-18-drop-outbox.md`).
// `list_outbox_files` is the old name kept alive for CLI + MCP consumers
// until the outbox agent migrates them to `list_draft_files`.
pub use inbox_ops::list_draft_files as list_outbox_files;
pub use invite_ops::{
    claim_invite, create_invite, view_invite, InviteClaimed, InviteCreated, InviteError,
    InviteView, OrgInviteContext, DEFAULT_INVITE_TTL_HOURS,
};
pub use launch_channel::{launch_channel, launch_channel_with_binding, LaunchChannelError};
pub use process_correspondence_claims::{
    process_correspondence_claims, ClaimProcessError, ClaimProcessOutcome, CorrespondenceClaim,
    SkipReason,
};
pub use review_queue::{list_drafts_queue, list_local_queues, list_review_queue};
pub use send_envelope::{send_stamped_envelope, SendError, SendOutcome};
pub use sync::{
    drain_outbox, drain_pending_sends, sync_now, RelaySyncReport, SyncError, SyncOutcome,
};
pub use stamp_document::{stamp_document, StampError, StampOutcome};
pub use verify_document::{verify_document, VerifyError, VerifyOutcome};
