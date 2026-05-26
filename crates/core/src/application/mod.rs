//! Application — orchestrates use cases (composition of domain + ports).

pub mod accept_org_membership;
pub mod ag_extract;
pub mod agent_manifest_ops;
pub mod agent_ops;
pub mod channel_def_envelope;
pub mod capture_ops;
pub mod channels_ops;
pub mod compose_envelope;
pub mod contextify_capture;
pub mod contract_ops;
pub mod delivery_policy;
pub mod federation;
pub mod inbox_actions;
pub mod inbox_ops;
pub mod invite_ops;
pub mod launch_channel;
pub mod org_ops;
pub mod review_queue;
pub mod stamp_document;
pub mod sync;
pub mod verify_document;

pub use accept_org_membership::{
    persist_org_membership, AcceptMembershipError, AcceptMembershipOutcome,
    AcceptMembershipRequest,
};
pub use agent_manifest_ops::{
    emit_manifest_into_channel, ingest_manifest_from_file, AgentManifestOpsError,
};
pub use agent_ops::{add_agent, list_agents, remove_agent, rotate_agent, AgentOpsError};
pub use capture_ops::{
    capture_to_queue, capture_to_queue_with_ag, channels_root_for, CaptureError, CaptureRequest,
};
pub use channel_def_envelope::{
    emit_channel_def_envelope, ingest_channel_def_envelope, parse_channel_def_from_envelope,
    ChannelDefEnvelopeError, ChannelDefRecord, IngestOutcome,
    CHANNEL_DEF_TYPE as CHANNEL_DEF_ENVELOPE_TYPE,
};
pub use channels_ops::{
    create_channel, delete_channel, handle_is_reserved, list_channels, read_channel,
    ChannelEnvelope, ChannelOpError, ChannelSummary, META_HANDLE,
};
pub use compose_envelope::{
    compose_envelope, compose_envelope_with_ag, ComposeError, ComposeRequest, ComposeSigner,
};
pub use contextify_capture::{
    contextify_capture, try_contextify_after_capture, ContextifyError, ContextifyOutcome,
    ContextifySkipReason, ROUTABLE_QUEUE,
};
pub use contract_ops::{
    get_channel_contract, get_org_contract, resolve_channel_contract, set_channel_contract,
    set_org_contract, ContractLevel, ContractOpsError, ContractPatch, ContractView, PatchField,
    ResolvedContract,
};
pub use delivery_policy::{decide_poll, CadenceConfig, CadenceConfigError, PollDecision};
pub use federation::{drain_undelivered, FederationError, FederationOutcome};
pub use inbox_actions::{archive_envelope, defer_envelope, unarchive_envelope, InboxActionError};
pub use inbox_ops::{
    list_draft_files, list_inbox_files, read_envelope, InboxOpError, ListedEnvelope, ReadResult,
};
pub use org_ops::{create_org, delete_org, list_orgs, show_org, OrgOpsError};
// Bridge: outbox-rename slice in flight (`docs/pitches/2026-05-18-drop-outbox.md`).
// `list_outbox_files` is the old name kept alive for CLI + MCP consumers
// until the outbox agent migrates them to `list_draft_files`.
pub use inbox_ops::list_draft_files as list_outbox_files;
pub use invite_ops::{
    claim_invite, create_invite, view_invite, InviteClaimed, InviteCreated, InviteError,
    InviteView, OrgInviteContext, DEFAULT_INVITE_TTL_HOURS,
};
pub use launch_channel::{launch_channel, launch_channel_with_binding, LaunchChannelError};
pub use review_queue::{list_drafts_queue, list_local_queues, list_review_queue};
pub use stamp_document::{stamp_document, StampError, StampOutcome};
pub use sync::{sync_now, RelaySyncReport, SyncError, SyncOutcome};
pub use verify_document::{
    verify_document, verify_document_layered, LayeredVerifyOutcome, SignatureOutcome, VerifyError,
    VerifyOutcome,
};
