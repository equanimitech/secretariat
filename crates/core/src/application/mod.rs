//! Application — orchestrates use cases (composition of domain + ports).

pub mod ag_extract;
pub mod agent_manifest_ops;
pub mod agent_ops;
pub mod channels_ops;
pub mod compose_ops;
pub mod inbox_ops;
pub mod launch_channel;
pub mod org_ops;
pub mod repo_ops;
pub mod stamp_document;
pub mod timeline_ops;
pub mod verify_document;
pub mod workflow_ops;

pub use agent_manifest_ops::{
    emit_manifest_into_channel, ingest_manifest_from_file, AgentManifestOpsError,
};
pub use agent_ops::{add_agent, list_agents, remove_agent, rotate_agent, AgentOpsError};
// Retained for the Tauri timeline keeper (`read_channel_envelopes`), which
// projects a channel-dir's `envelopes/` tree. The channel-CRUD callers
// (MCP / CLI / Tauri channel commands) were cut in the git-native teardown;
// `read_channel` + `ChannelEnvelope` survive because timeline reads them.
pub use channels_ops::{
    create_channel, delete_channel, handle_is_reserved, list_channels, read_channel,
    ChannelEnvelope, ChannelOpError, ChannelSummary, META_HANDLE,
};
pub use compose_ops::{
    compose_document, resolve_sole_scribe, ComposeError, ComposeOutcome, DocType,
    ScribeResolveError,
};
pub use inbox_ops::{read_envelope, InboxOpError, ReadResult};
pub use launch_channel::{launch_channel, launch_channel_with_binding, LaunchChannelError};
pub use org_ops::{create_org, delete_org, list_orgs, show_org, OrgOpsError};
pub use repo_ops::{list_repos, register_repo, unregister_repo, RepoOpsError};
pub use stamp_document::{stamp_document, StampError, StampOutcome};
pub use timeline_ops::{
    build_timeline, resolve_range, DayBucket, DocState, Timeline, TimelineEntry, TimelineError,
    TimelineFilter,
};
pub use verify_document::{
    verify_document, verify_document_layered, LayeredVerifyOutcome, SignatureOutcome, VerifyError,
    VerifyOutcome,
};
pub use workflow_ops::{load_workflows, match_workflows, parse_workflow, WorkflowError};
