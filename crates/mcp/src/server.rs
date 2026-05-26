//! Secretariat MCP server — stdio transport.
//!
//! Tools exposed:
//!
//! | Tool | Purpose |
//! |---|---|
//! | `compose` | Write a draft envelope into the queue's `envelopes/YYYY/MM/DD/` tree (frontmatter omits `delivered:`; principal stamps separately) |
//! | `capture` | Drop a body into a local queue (never sent, never stamped without consent) |
//! | `stamp` | Trigger biometric stamp on a draft (Touch ID gates regardless of caller) |
//! | `secretariat://orgs` | Org + channel-tree directory — resource |
//! | `secretariat://compositions` | Pending drafts awaiting stamp — resource |
//! | `defer` | Move a capture envelope out of its active queue ('remind me later') |
//! | `archive` | Move a capture envelope to its queue's `archived/` ('handled') |
//! | `unarchive` | Reverse of `archive` — move from `archived/` back into `envelopes/` |
//! | `read` | Decrypt + return body of an envelope |
//! | `verify` | Check a stamped artifact |
//!
//! On `stamp`: the call only *initiates* the ceremony; the platform
//! biometric gate (Touch ID via the Swift helper) blocks until the
//! principal physically authorizes. Claude cannot bypass that. The
//! tradeoff vs. principal-only initiation (rule 4 in earlier AGENTS.md
//! drafts) is recorded explicitly: phishing/habituation risk is accepted
//! because the dialog still requires a fingerprint, and the alternative
//! (principal must context-switch to a terminal) eroded the workflow.
//!
//! Tools deliberately **not** exposed:
//!
//! - `send` — daemon-only. Once a principal stamps an envelope, the daemon
//!   transmits it on cadence. Cleaner three-actor separation: Claude
//!   composes, principal stamps, daemon transmits.

use std::path::PathBuf;

use chrono::Utc;
use rmcp::{
    handler::server::{
        router::{prompt::PromptRouter, tool::ToolRouter},
        wrapper::{Json, Parameters},
    },
    model::{
        Annotated, ErrorCode, ErrorData, GetPromptRequestParam, GetPromptResult, Implementation,
        ListPromptsResult, ListResourcesResult, PaginatedRequestParam, PromptMessage,
        PromptMessageRole, ProtocolVersion, RawResource, ReadResourceRequestParam,
        ReadResourceResult, Resource, ResourceContents, ServerCapabilities, ServerInfo,
    },
    prompt, prompt_handler, prompt_router,
    service::RequestContext,
    tool, tool_handler, tool_router, RoleServer, ServerHandler,
};
use schemars::JsonSchema;
use secretariat_core::application::{
    add_agent as app_add_agent, archive_envelope, capture_to_queue_with_ag, channels_root_for,
    claim_invite, compose_envelope_with_ag, create_channel as app_create_channel, create_invite,
    create_org as app_create_org, delete_channel as app_delete_channel,
    delete_org as app_delete_org, get_channel_contract as app_get_channel_contract,
    get_org_contract as app_get_org_contract, list_agents as app_list_agents, list_channels,
    list_draft_files, list_orgs as app_list_orgs, read_channel, read_envelope,
    remove_agent as app_remove_agent, resolve_channel_contract as app_resolve_channel_contract,
    rotate_agent as app_rotate_agent, set_channel_contract as app_set_channel_contract,
    set_org_contract as app_set_org_contract, show_org as app_show_org, stamp_document,
    try_contextify_after_capture, unarchive_envelope, verify_document_layered, view_invite,
    CaptureRequest, ComposeRequest, ContractLevel, ContractPatch, ContractView,
    LayeredVerifyOutcome, PatchField, ResolvedContract, StampError, VerifyOutcome,
};
use secretariat_core::domain::{OrgAlias, QueueHandle, Recipient, Root, StampAct, TrustGate};
use secretariat_core::infrastructure::biometric::build_signer;
use secretariat_core::infrastructure::composite_did_resolver::CompositeDidResolver;
use secretariat_core::infrastructure::did_web_resolver::DidWebResolver;
use secretariat_core::infrastructure::keys::{load_signing_key, KeyPaths};
use secretariat_core::infrastructure::org_store::org_channels_root;
use secretariat_core::infrastructure::preferences::load_or_migrate as load_or_migrate_preferences;
use secretariat_core::infrastructure::transport::RelayState;
use secretariat_core::ports::SignerError;
use secretariat_core::{Did, EnvelopeDepth, EnvelopeUrgency};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Clone)]
pub struct SecretariatServer {
    pub paths: KeyPaths,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

impl SecretariatServer {
    pub fn new(paths: KeyPaths) -> Self {
        Self {
            paths,
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }

    /// Resolve the channels root for an optional org alias. Returns the
    /// personal channels root when `org_alias` is None; returns the
    /// org-scoped channels root after verifying the org exists on disk.
    fn resolve_channels_root(
        &self,
        org_alias: Option<&str>,
    ) -> Result<std::path::PathBuf, ErrorData> {
        match org_alias {
            None => Ok(self.paths.personal_channels_root()),
            Some(s) => {
                let alias = OrgAlias::parse(s)
                    .map_err(|e| invalid_request(format!("invalid org alias `{s}`: {e}")))?;
                let dir = self.paths.orgs_root.join(alias.as_str());
                if !dir.exists() {
                    return Err(invalid_request(format!(
                        "org `{}` does not exist — create it with `create_org` first",
                        alias.as_str()
                    )));
                }
                Ok(org_channels_root(&self.paths.orgs_root, &alias))
            }
        }
    }

    /// Parse an optional org alias into a `Root` for use with
    /// resolver-shaped APIs (capture, contextify).
    fn resolve_root(&self, org_alias: Option<&str>) -> Result<Root, ErrorData> {
        match org_alias {
            None => Ok(Root::Self_),
            Some(s) => {
                let alias = OrgAlias::parse(s)
                    .map_err(|e| invalid_request(format!("invalid org alias `{s}`: {e}")))?;
                let dir = self.paths.orgs_root.join(alias.as_str());
                if !dir.exists() {
                    return Err(invalid_request(format!(
                        "org `{}` does not exist — create it with `create_org` first",
                        alias.as_str()
                    )));
                }
                Ok(Root::Org(alias))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Prompts — Secretariat correspondence vocabulary. Each prompt is a static
// markdown body shipped alongside the binary; they surface as slash commands
// in MCP-aware clients (Claude Code, Claude Desktop). All prompts wrap
// existing tools — they're rituals around the correspondence model, not
// orthogonal workflows.
//
// Substrate verbs (capture-shaped):
//   /idea, /pain                — wrap `capture` to a self-addressed queue.
//
// Native verbs (envelope-shaped):
//   /review                     — paced walker over orgs + pending drafts
//                                  (full contract-aware orchestration with a
//                                  per-vault cursor + dive lands in a follow-up
//                                  slice — see
//                                  `docs/pitches/2026-05-17-review-orchestration.md`).
//   /compose                    — AG-template-aware envelope draft.
//   /onboard                    — init + invite_create / invite_claim ceremony.
//   /stamp                      — explicit show-body → confirm → stamp ceremony.
//
// Deliberately NOT shipped (out of correspondence bounded context):
//   /share, /shaping, /roundtable — Rafa-personal PM/sharing vocabulary,
//                                   stays in `~/.claude/skills/`.
//
// Pitch: `docs/pitches/2026-05-05-mcp-prompts-as-substrate-vocabulary.md`.
// ---------------------------------------------------------------------------

#[prompt_router]
impl SecretariatServer {
    /// Capture a raw idea — a product thought, a fleeting note, anything
    /// worth keeping. Routes through the `capture` tool with
    /// `queue: triage`.
    #[prompt(name = "idea")]
    pub async fn idea_prompt(&self) -> Result<Vec<PromptMessage>, ErrorData> {
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            include_str!("prompts/idea.md"),
        )])
    }

    /// Capture a bug, friction, or improvement. Routes through the
    /// `capture` tool with `queue: pain`.
    #[prompt(name = "pain")]
    pub async fn pain_prompt(&self) -> Result<Vec<PromptMessage>, ErrorData> {
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            include_str!("prompts/pain.md"),
        )])
    }

    /// Walk the principal through a paced review session — orient via the
    /// org / channel-tree, then walk pending drafts awaiting a stamp, one
    /// envelope at a time.
    #[prompt(name = "review")]
    pub async fn review_prompt(&self) -> Result<Vec<PromptMessage>, ErrorData> {
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            include_str!("prompts/review.md"),
        )])
    }

    /// Draft an envelope addressed to a channel, formatted per the
    /// principal's attentional-granularity template, with the
    /// inline-render-first consent gate before the draft hits disk.
    #[prompt(name = "compose")]
    pub async fn compose_prompt(&self) -> Result<Vec<PromptMessage>, ErrorData> {
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            include_str!("prompts/compose.md"),
        )])
    }

    /// Bring the principal into Secretariat: confirm identity (set up
    /// via the Secretariat.app first-launch popover or `sec init`), then
    /// establish the first stampable correspondence relationship via
    /// `invite` (you invite someone) or `accept_invite` (you accept
    /// someone's invitation).
    #[prompt(name = "onboard")]
    pub async fn onboard_prompt(&self) -> Result<Vec<PromptMessage>, ErrorData> {
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            include_str!("prompts/onboard.md"),
        )])
    }

    /// Explicit stamp ceremony: read → render verbatim → wait for consent
    /// → call `stamp`. Formalizes the multi-turn pre-check that the
    /// `stamp` tool's description requires.
    #[prompt(name = "stamp")]
    pub async fn stamp_prompt(&self) -> Result<Vec<PromptMessage>, ErrorData> {
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            include_str!("prompts/stamp.md"),
        )])
    }
}

// ---------------------------------------------------------------------------
// Parameter / output schemas
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ComposeParams {
    /// Channel-owner DID (`did:web:...` / `did:key:...`). The envelope
    /// addresses a channel owned by this DID (often the principal's
    /// own DID for own-org channels). Move 3b removed DM / peer /
    /// bilateral primitives — every compose targets a channel.
    pub to: String,
    /// Plaintext body (markdown). v0 writes it as-is; encryption happens at
    /// stamp / send time. (v0.x: optional `encrypt: bool` here.)
    pub body: String,
    /// `gross` or `subtle`. Defaults to `subtle`.
    #[serde(default)]
    pub depth: Option<String>,
    /// `now`, `soon`, or `whenever`. Defaults to `whenever`.
    #[serde(default)]
    pub urgency: Option<String>,
    /// Free-form provenance hint (e.g. "claude-code-2026-05-02").
    #[serde(default)]
    pub source: Option<String>,
    /// Optional cadence hint for the channel.
    #[serde(default)]
    pub cadence_hint: Option<String>,
    /// Channel handle on the owner's machine — colon-separated path
    /// segments (e.g. `assemblee_generale`,
    /// `dommage-corporel:paris-cohort`). Defaults to `inbox` for
    /// MCP-compat; CLI requires this explicitly post-Move-3a.
    #[serde(default)]
    pub handle: Option<String>,
    /// Optional headline (AG gross signal, 2-6 words). Setting any of
    /// `title` / `lede` / `summary` suppresses the AI auto-fill pass.
    /// When all three are omitted and a cognition adapter is configured,
    /// the scribe drafts them from the body and tags `ag_source = "ai"`.
    #[serde(default)]
    pub title: Option<String>,
    /// Optional one-line lede (AG subtle layer). See `title`.
    #[serde(default)]
    pub lede: Option<String>,
    /// Optional multi-sentence summary (AG deepening pathway). See `title`.
    #[serde(default)]
    pub summary: Option<String>,
    /// Optional explicit authorized agent (by `name` from
    /// `authorized_agents`). When omitted, the first scribe in the
    /// principal's `authorized_agents` signs. When the principal has no
    /// scribe configured, the principal's own key signs and
    /// `signer_role: principal` is recorded on the envelope's
    /// `$signature`. Substrate-for-themia Move 2.
    #[serde(default)]
    pub agent: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ComposeOutput {
    pub file_path: String,
    pub note: String,
    /// DID of the key that signed this envelope's `$signature` block.
    /// Receivers verify this against the relevant `authorized_agents`
    /// list (for agents) or directly resolve the DID (for principals).
    pub signed_by: String,
    /// `agent` or `principal`. Hint for receiver-side UI; the
    /// cryptographic check still relies on `signed_by`.
    pub signer_role: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureParams {
    /// Target queue handle — colon-separated path segments, e.g.
    /// `triage`, `articles`, `dommage-corporel:paris-cohort`. Tree
    /// depth = colon depth.
    pub queue: String,
    /// Body of the capture (markdown-friendly plain text).
    pub body: String,
    /// Free-form origin marker, e.g. `idea-skill`, `quick-pane`. Defaults
    /// to `mcp-capture` when omitted.
    #[serde(default)]
    pub source: Option<String>,
    /// Optional org alias (`themia.pro`, `equanimi.tech`). When set the
    /// capture lands inside that org's channel tree. Omit (or set null)
    /// for personal captures (under the self channels root).
    #[serde(default)]
    pub org: Option<String>,
    /// Optional AG title (gross signal). Setting any of `title` / `lede`
    /// / `summary` suppresses the AI auto-fill pass. When all three are
    /// omitted and the body is substantive (>=280 chars or contains a
    /// paragraph break), the scribe drafts them and tags
    /// `ag_source = "ai"`.
    #[serde(default)]
    pub title: Option<String>,
    /// Optional AG lede (subtle layer). See `title`.
    #[serde(default)]
    pub lede: Option<String>,
    /// Optional AG summary (deepening pathway). See `title`.
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CaptureOutput {
    pub file_path: String,
    pub queue: String,
    pub note: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadParams {
    /// Absolute path to the envelope `.md` file.
    pub file_path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InboxActionParams {
    /// Absolute path to the inbox envelope `.md` file. Must live
    /// under `~/.secretariat/channels/inbox/envelopes/...`.
    pub file_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InboxActionOutput {
    pub moved_to: String,
    pub note: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadOutput {
    pub body: String,
    pub envelope_from: Option<String>,
    pub envelope_to: Option<String>,
    pub encrypted: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StampParams {
    /// Absolute path to the envelope to stamp. Post-Move 4 every
    /// envelope (draft, federated, received) lives at one of the two
    /// channel-tree roots (Move 3c):
    /// `<root>/channels/<handle-path>/envelopes/YYYY/MM/DD/<rkey>.md` (self-owned)
    /// or `<root>/orgs/<org-alias>/channels/<handle-path>/envelopes/YYYY/MM/DD/<rkey>.md`
    /// (org-owned). Stamping embeds the `$attestation` block in place; no rename.
    pub file_path: String,
    /// Re-stamp even if a stamp is already present.
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StampOutput {
    pub stamped_path: String,
    pub signer: String,
    pub stamped_at: String,
    pub doc_hash: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DaemonTickOutput {
    pub note: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DaemonStatusOutput {
    /// True when the daemon's IPC socket is reachable. False means
    /// either no daemon is running, or it crashed and didn't clean up.
    pub daemon_reachable: bool,
    /// Every relay this principal has registered with, in the order
    /// they were added. Cursor advances as inbound envelopes are
    /// filed; mismatched cursors across machines indicate a sync gap.
    pub relays: Vec<DaemonRelayStatus>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DaemonRelayStatus {
    pub endpoint: String,
    pub registered: bool,
    /// Number of `(owner, handle)` queues this relay is tracking for the
    /// principal. v0.8+ per-queue cursor model.
    pub queues_tracked: usize,
    /// Maximum cursor across all tracked queues — a summary for at-a-glance
    /// "is the daemon making progress" reads. `0` if no queues tracked.
    pub max_cursor: u64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VerifyParams {
    pub file_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct VerifyOutput {
    /// Author signature layer (`$signature`). Three-state per substrate-
    /// for-themia element §3: `ok`, `okUnverifiedAgent`, `tampered`,
    /// `signerUnresolvable`, `invalid`, or `none` (absent).
    pub signature: VerifyLayer,
    /// Principal stamp layer (`$attestation`). Selective per AGENTS.md
    /// rule #4 — `none` is the common case (most envelopes are
    /// ambient signed-only).
    pub stamp: VerifyLayer,
}

#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct VerifyLayer {
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signer_role: Option<String>,
    /// Present only for `outcome: "verifiedAgent"` — the principal DID
    /// the agent is bound to via a cached `agentManifest` snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub principal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InviteParams {
    /// Free-form purpose hint shown to the recipient (e.g. "first-contact").
    #[serde(default)]
    pub purpose: Option<String>,
    /// Token TTL in hours. Default: 168 (7 days). Server caps at 720.
    #[serde(default)]
    pub ttl_hours: Option<i64>,
    /// Override the relay endpoint. Defaults to the first registered relay
    /// in `~/.secretariat/relay-state.json`.
    #[serde(default)]
    pub endpoint: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InviteOutput {
    pub token: String,
    pub claim_url: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AcceptInviteParams {
    /// Claim URL the inviter shared
    /// (e.g. `https://secretariat.equanimi.tech/v0/invite/<token>`).
    pub claim_url: String,
    /// Kept for backward compatibility. The local contact book was
    /// removed in the substrate-for-themia slice (Move 3b); this
    /// field is now a no-op.
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AcceptInviteOutput {
    pub inviter_did: String,
    pub claimant_did: String,
    pub claimed_at: String,
    /// Whether the relay registered the claimant's DID during this call.
    pub registered: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListChannelsOutput {
    /// Channels with at least one envelope, sorted by latest activity
    /// (newest first).
    pub channels: Vec<ChannelSummaryDto>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ChannelSummaryDto {
    /// Canonical handle, e.g. `secretariat:dev`.
    pub handle: String,
    /// Human-readable display name from `channel.md` (empty if unset).
    pub name: String,
    /// Free-form description from `channel.md` (empty if unset).
    pub description: String,
    /// Number of envelopes in this channel.
    pub envelope_count: usize,
    /// ISO-8601 timestamp of the most recent envelope (omitted if the
    /// channel is empty or filenames are malformed).
    pub latest_at: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListChannelsParams {
    /// Optional org alias to scope the listing to. Omit to list the
    /// principal's personal (no-org) channel tree.
    #[serde(default)]
    pub org: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadChannelParams {
    /// Channel handle, e.g. `secretariat:dev`. Colon-pathed segments;
    /// tree depth = colon depth.
    pub handle: String,
    /// Maximum number of envelopes to return (newest first). Defaults
    /// to 10 when omitted.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Optional org alias to scope the read to. Must match the org the
    /// channel lives in (if it lives in one). Omit for personal channels.
    #[serde(default)]
    pub org: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadChannelOutput {
    pub handle: String,
    pub envelopes: Vec<ChannelEnvelopeDto>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ChannelEnvelopeDto {
    pub file_path: String,
    pub from: Option<String>,
    /// ISO-8601 captured-at timestamp parsed from the filename.
    pub captured_at: Option<String>,
    pub source: String,
    pub stamped: bool,
    pub encrypted: bool,
    pub body: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentAddParams {
    /// Principal-chosen nickname for the agent. Conventionally matches the
    /// substrate identifier (e.g. `claude` for `--substrate claude-code`).
    /// Must be lowercase `[a-z0-9_-]+`, max 64 chars.
    pub name: String,
    /// Agent role. Today only `scribe`. Defaults to `scribe` when omitted.
    #[serde(default)]
    pub role: Option<String>,
    /// Cognition provider this agent runs under. Today only `claude-code`.
    /// Defaults to `claude-code` when omitted.
    #[serde(default)]
    pub substrate: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AgentRemoveParams {
    /// Nickname of the agent to remove or rotate.
    pub name: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AgentDto {
    pub did: String,
    pub role: String,
    pub name: String,
    pub substrate: String,
    pub added_at: String,
}

impl From<secretariat_core::domain::Agent> for AgentDto {
    fn from(a: secretariat_core::domain::Agent) -> Self {
        Self {
            did: a.did.as_str().to_string(),
            role: a.role.as_str().to_string(),
            name: a.name.as_str().to_string(),
            substrate: a.substrate.as_str().to_string(),
            added_at: a.added_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListAgentsOutput {
    pub agents: Vec<AgentDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateOrgParams {
    /// Friendly alias the org lives under on disk (e.g. `themia.pro`,
    /// `equanimi.tech`). Lowercase, host-safe characters
    /// (`[a-z0-9][a-z0-9-.]*`). Must not be a reserved substrate name.
    pub alias: String,
    /// Optional canonical DID, e.g. `did:web:themia.pro`. Omit for
    /// local-only orgs without a federation identity yet.
    #[serde(default)]
    pub did: Option<String>,
    /// Optional human-readable name. Defaults to the alias.
    #[serde(default)]
    pub name: Option<String>,
    /// Optional free-form description. Empty if omitted.
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct OrgDto {
    pub alias: String,
    pub did: Option<String>,
    pub name: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListOrgsOutput {
    pub orgs: Vec<OrgDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteOrgParams {
    pub alias: String,
    /// Must be set to `true` to actually delete the org tree.
    /// Defense against accidental deletion.
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteOrgOutput {
    pub alias: String,
    pub deleted: bool,
    pub note: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateChannelParams {
    /// Channel handle, e.g. `product:data:baux-commerciaux`.
    /// Colon-pathed segments; tree depth = colon depth.
    pub handle: String,
    /// Optional org alias to create the channel inside. If omitted, the
    /// channel lives in the principal's personal channel tree.
    #[serde(default)]
    pub org: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ChannelDefDto {
    pub handle: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteChannelParams {
    pub handle: String,
    #[serde(default)]
    pub org: Option<String>,
    /// Must be set to `true` to actually delete the channel tree.
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteChannelOutput {
    pub handle: String,
    pub org: Option<String>,
    pub deleted: bool,
    pub note: String,
}

// -- consumption contracts ----------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetChannelContractParams {
    pub handle: String,
    #[serde(default)]
    pub org: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetChannelContractParams {
    pub handle: String,
    #[serde(default)]
    pub org: Option<String>,
    /// New poll-floor in minutes for this channel. Omit to leave the
    /// current value; pair with `clear: ["cadence_floor_minutes"]` to
    /// revert to inheriting from ancestors.
    #[serde(default)]
    pub cadence_floor_minutes: Option<u32>,
    /// New receiver-side trust filter: `signed-only` or
    /// `stamp-required`. Same Leave/Set/Clear semantics as cadence.
    #[serde(default)]
    pub min_trust: Option<String>,
    /// Field names to revert to None (inherit). Allowed entries:
    /// `cadence_floor_minutes`, `min_trust`. Listing a field here AND
    /// passing a set-value for it is a conflict — caller must pick.
    #[serde(default)]
    pub clear: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetOrgContractParams {
    pub org: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetOrgContractParams {
    pub org: String,
    #[serde(default)]
    pub cadence_floor_minutes: Option<u32>,
    #[serde(default)]
    pub min_trust: Option<String>,
    #[serde(default)]
    pub clear: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ContractDto {
    /// Bare handle (e.g. `"foo:bar"`) for channel contracts; the org
    /// alias for org-root contracts.
    pub scope: String,
    pub path: String,
    pub cadence_floor_minutes: Option<u32>,
    pub min_trust: Option<String>,
    pub body: String,
}

impl ContractDto {
    fn from_view(scope: String, view: ContractView) -> Self {
        Self {
            scope,
            path: view.path.display().to_string(),
            cadence_floor_minutes: view.contract.cadence_floor_minutes,
            min_trust: view.contract.min_trust.map(|g| g.as_str().to_string()),
            body: view.body,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResolveChannelContractParams {
    pub handle: String,
    #[serde(default)]
    pub org: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ContractLevelDto {
    pub scope: String,
    pub path: String,
    pub cadence_floor_minutes: Option<u32>,
    pub min_trust: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ResolvedContractDto {
    pub handle: String,
    /// Merged view across the chain — what to actually enforce.
    pub merged_cadence_floor_minutes: Option<u32>,
    pub merged_min_trust: Option<String>,
    /// One entry per `contract.local.md` file found along the walk.
    pub chain: Vec<ContractLevelDto>,
}

impl ContractLevelDto {
    fn from_level(level: ContractLevel) -> Self {
        Self {
            scope: level.scope,
            path: level.path.display().to_string(),
            cadence_floor_minutes: level.contract.cadence_floor_minutes,
            min_trust: level.contract.min_trust.map(|g| g.as_str().to_string()),
        }
    }
}

impl ResolvedContractDto {
    fn from_resolved(handle: String, r: ResolvedContract) -> Self {
        Self {
            handle,
            merged_cadence_floor_minutes: r.merged.cadence_floor_minutes,
            merged_min_trust: r.merged.min_trust.map(|g| g.as_str().to_string()),
            chain: r
                .chain
                .into_iter()
                .map(ContractLevelDto::from_level)
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool router
// ---------------------------------------------------------------------------

#[tool_router]
impl SecretariatServer {
    #[tool(
        name = "compose",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        ),
        description = "Compose a draft envelope addressed to a channel \
        (`{owner_did, channel_handle}`) and write it directly into the \
        channel's `envelopes/YYYY/MM/DD/` day-shard. The envelope's \
        frontmatter omits `delivered:` — absence is the substrate's draft signal. \
        \
        **Substrate-for-themia Move 2: the envelope is signed at compose.** The \
        author signature (`$signature` frontmatter block) is mandatory; by default \
        the first scribe in the principal's `authorized_agents` signs (the scribe's \
        own DID + key, not the principal's). Pass `agent: <name>` to disambiguate \
        among multiple scribes. If no scribe is configured, the principal's own \
        key signs and `signer_role: principal` is recorded — substrate works without \
        an agent, ceremony is just heavier. \
        \
        The principal STAMPS later, selectively (biometric-gated, separate `stamp` \
        tool, never via this tool). Stamping adds `$attestation` alongside the \
        author signature; they are independent layers and a stamped envelope \
        carries both. The daemon's envelope watcher then federates the envelope \
        and writes `delivered: <relay-seq-id>` in place on success. \
        \
        `to` must be a DID (`did:web:...` / `did:key:...`). \
        \
        AG fields (`title` / `lede` / `summary`) are author-attributed when you \
        pass them. If you omit all three and a cognition adapter is configured, \
        the scribe drafts them from the body and tags `ag_source = \"ai\"` so \
        receivers can tell the framing is AI-generated."
    )]
    async fn compose(
        &self,
        Parameters(params): Parameters<ComposeParams>,
    ) -> Result<Json<ComposeOutput>, ErrorData> {
        let to = resolve_to_did(&self.paths, &params.to)?;
        let depth = parse_depth(params.depth.as_deref())?;
        let urgency = parse_urgency(params.urgency.as_deref())?;
        let from = load_principal_did(&self.paths)?;

        let body = if params.body.trim().is_empty() {
            None
        } else {
            Some(params.body)
        };

        let handle_str = params.handle.as_deref().unwrap_or("inbox");
        let handle = QueueHandle::parse(handle_str)
            .map_err(|e| invalid_request(format!("invalid `handle` `{handle_str}`: {e}")))?;

        let req = ComposeRequest {
            from,
            recipient: Recipient::new(to, handle),
            depth,
            urgency,
            source: params.source.unwrap_or_else(|| "mcp".to_string()),
            cadence_hint: params.cadence_hint,
            body,
            title: params.title,
            lede: params.lede,
            summary: params.summary,
        };

        let self_did = load_principal_did(&self.paths)?;
        let aliases = secretariat_core::infrastructure::queue_dir::AliasMap::load(
            self_did.clone(),
            &self.paths,
        )
        .map_err(|e| invalid_request(format!("loading alias map: {e}")))?;
        let prefs = load_or_migrate_preferences(
            &self.paths.preferences,
            &self.paths.legacy_cognition_config,
            &self.paths.legacy_cadence,
        )
        .unwrap_or_default();

        // Substrate-for-themia Move 2: resolve the signing context
        // (agent by default, principal fallback).
        let signing_ctx = resolve_compose_signer(&self.paths, &self_did, params.agent.as_deref())?;
        let signed_by = signing_ctx.signer_did.as_str().to_string();
        let signer_role_str = signing_ctx.signer_role.as_str().to_string();
        let signer = secretariat_core::application::ComposeSigner::new(
            signing_ctx.signer_did,
            signing_ctx.signer_role,
            &signing_ctx.signing_key,
        );

        let path = compose_envelope_with_ag(
            req,
            &signer,
            &self.paths.template,
            &self.paths.root,
            &aliases,
            &prefs.cognition,
            Utc::now(),
        )
        .await
        .map_err(|e| invalid_request(format!("compose failed: {e}")))?;

        info!(
            file = %path.display(),
            signed_by = %signed_by,
            signer_role = %signer_role_str,
            "composed envelope via MCP",
        );

        Ok(Json(ComposeOutput {
            file_path: path.display().to_string(),
            note: "Draft written into the queue's `envelopes/YYYY/MM/DD/` tree. \
                   The envelope frontmatter carries `$signature` (author, mandatory) \
                   and omits `delivered:` (draft signal) and `$attestation` (stamp \
                   is selective). Show the body to the principal, get explicit \
                   confirmation, then stamp via the `stamp` tool (biometric-gated). \
                   The daemon picks up the file, federates it, and writes \
                   `delivered:` in-place on success."
                .to_string(),
            signed_by,
            signer_role: signer_role_str,
        }))
    }

    #[tool(
        name = "capture",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        ),
        description = "Drop a body of text into a local queue. Captures are \
        envelopes addressed to a channel the principal themselves owns — same \
        primitive as `compose`, but the routing rule keeps them on disk (no \
        federation). Use for ideas, journal entries, future-self notes, anything \
        to surface again at the next review session. Stamps are optional \
        (tamper-evident self-attestation), never required. \
        \
        The `queue` parameter is a colon-pathed handle (e.g. `triage`, \
        `pain`, `articles`, `dommage-corporel:paris-cohort`). If you \
        don't know which to pick, default to `triage`. \
        \
        AG fields (`title` / `lede` / `summary`) are author-attributed when \
        supplied. Omit all three for substantive captures (>=280 chars or \
        multi-paragraph) and the scribe drafts them, tagged `ag_source = \"ai\"`. \
        Short one-liners skip extraction entirely."
    )]
    async fn capture(
        &self,
        Parameters(params): Parameters<CaptureParams>,
    ) -> Result<Json<CaptureOutput>, ErrorData> {
        let from = load_principal_did(&self.paths)?;
        let queue = QueueHandle::parse(&params.queue)
            .map_err(|e| invalid_request(format!("invalid `queue` `{}`: {e}", params.queue)))?;

        let req = CaptureRequest {
            from,
            queue: queue.clone(),
            body: params.body,
            source: params.source.unwrap_or_else(|| "mcp-capture".to_string()),
            title: params.title,
            lede: params.lede,
            summary: params.summary,
        };

        let root = self.resolve_root(params.org.as_deref())?;
        let prefs = load_or_migrate_preferences(
            &self.paths.preferences,
            &self.paths.legacy_cognition_config,
            &self.paths.legacy_cadence,
        )
        .unwrap_or_default();
        let path =
            capture_to_queue_with_ag(req, &self.paths.root, &root, &prefs.cognition, Utc::now())
                .await
                .map_err(|e| invalid_request(format!("capture failed: {e}")))?;

        info!(file = %path.display(), queue = %queue.as_str(), "captured to local queue via MCP");

        // Fire-and-forget contextification pass on triage captures.
        // No-op when no cognition adapter is configured (default state);
        // safe even if the file moves before the principal sees this
        // response because list_review_queue resolves paths on read,
        // never holds them in long-lived state.
        if queue.as_str() == secretariat_core::application::ROUTABLE_QUEUE {
            let capture_path = path.clone();
            // Contextify discovery walks the principal's own channels root —
            // the contextify pass routes between local queues, never across
            // org boundaries.
            let queues_root = channels_root_for(&self.paths.root, &root);
            let ledger_path = self.paths.contextification_log.clone();
            let preferences_path = self.paths.preferences.clone();
            let legacy_cognition = self.paths.legacy_cognition_config.clone();
            let legacy_cadence = self.paths.legacy_cadence.clone();
            tokio::spawn(async move {
                let prefs = load_or_migrate_preferences(
                    &preferences_path,
                    &legacy_cognition,
                    &legacy_cadence,
                )
                .unwrap_or_default();
                match try_contextify_after_capture(
                    &capture_path,
                    &queues_root,
                    &ledger_path,
                    &prefs.cognition,
                    Utc::now(),
                )
                .await
                {
                    Ok(Some(outcome)) if outcome.applied => {
                        info!(
                            from = %capture_path.display(),
                            to = %outcome.final_path.display(),
                            "contextification re-filed capture"
                        );
                    }
                    Ok(_) => {} // adapter off, threshold, or same queue
                    Err(e) => {
                        tracing::warn!(error = %e, "contextification pass failed");
                    }
                }
            });
        }

        Ok(Json(CaptureOutput {
            file_path: path.display().to_string(),
            queue: queue.as_str().to_string(),
            note: "Capture written to local queue. It stays on this device and \
                   surfaces again at the next review session — never sent, never stamped."
                .to_string(),
        }))
    }

    #[tool(
        name = "list_channels",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Enumerate every local channel that has at least one envelope. \
        Returns the channel handle, envelope count, and timestamp of the most-recent \
        envelope, sorted newest-activity-first. Use this for a top-level 'what's in \
        my channels?' view before descending into a specific one with `read_channel`. \
        \
        Channels are addressed by colon-pathed handles like `secretariat:dev` \
        or `dommage-corporel:paris-cohort`. Use `create_channel` first; \
        captures into an unknown channel are rejected."
    )]
    async fn list_channels(
        &self,
        Parameters(params): Parameters<ListChannelsParams>,
    ) -> Result<Json<ListChannelsOutput>, ErrorData> {
        let root = self.resolve_channels_root(params.org.as_deref())?;
        let summaries = list_channels(&root)
            .map_err(|e| invalid_request(format!("list_channels failed: {e}")))?;
        let channels = summaries
            .into_iter()
            .map(|s| ChannelSummaryDto {
                handle: s.handle,
                name: s.name,
                description: s.description,
                envelope_count: s.envelope_count,
                latest_at: s.latest_at.map(|t| t.to_rfc3339()),
            })
            .collect();
        Ok(Json(ListChannelsOutput { channels }))
    }

    #[tool(
        name = "read_channel",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Read the most-recent envelopes from a single channel, sorted \
        newest-first. The `handle` is a colon-pathed handle (e.g. \
        `secretariat:dev`). `limit` defaults to 10. \
        \
        Returns each envelope's body, sender, captured-at timestamp, and metadata \
        flags (stamped/encrypted). Use this to descend into one channel after \
        `list_channels` shows you what's available."
    )]
    async fn read_channel(
        &self,
        Parameters(params): Parameters<ReadChannelParams>,
    ) -> Result<Json<ReadChannelOutput>, ErrorData> {
        let handle = QueueHandle::parse(&params.handle)
            .map_err(|e| invalid_request(format!("invalid `handle` `{}`: {e}", params.handle)))?;
        let limit = params.limit.unwrap_or(10);
        let root = self.resolve_channels_root(params.org.as_deref())?;
        let envelopes = read_channel(&root, &handle, limit)
            .map_err(|e| invalid_request(format!("read_channel failed: {e}")))?;
        let envelopes = envelopes
            .into_iter()
            .map(|e| ChannelEnvelopeDto {
                file_path: e.file_path,
                from: e.from,
                captured_at: e.captured_at.map(|t| t.to_rfc3339()),
                source: e.source,
                stamped: e.stamped,
                encrypted: e.encrypted,
                body: e.body,
            })
            .collect();
        Ok(Json(ReadChannelOutput {
            handle: handle.as_str().to_string(),
            envelopes,
        }))
    }

    #[tool(
        name = "stamp",
        annotations(
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        ),
        description = "Stamp a draft envelope. Computes the canonical body hash, \
        triggers the platform biometric gate (Touch ID on macOS), embeds the \
        ed25519 signature into the file's frontmatter, and writes it back. The \
        biometric prompt blocks until the principal physically authorizes — the \
        tool cannot bypass it. \
        \
        REQUIRED PRE-CALL CHECKLIST (do not skip — phishing/habituation defense): \
        (1) Call `read` on the same `file_path` first. \
        (2) Render the FULL decrypted body to the user verbatim, in a code block \
        or quoted region — never a summary, never paraphrase. \
        (3) Wait for the user to explicitly say 'stamp it' (or equivalent) AFTER \
        seeing the body. Implicit consent from the prior turn does not count if \
        the body has changed since. \
        (4) Only then call `stamp`. \
        \
        The Touch ID dialog reason string includes the document's first-line \
        headline plus a short hash prefix; the user can cross-check that against \
        the body you displayed. If they differ, it means a different file was \
        stamped — abort and investigate. \
        \
        Returns signer DID, timestamp, and full document hash on success. \
        Errors: user cancelled Touch ID; file already stamped (pass \
        `force: true` to re-stamp); helper missing."
    )]
    async fn stamp(
        &self,
        Parameters(params): Parameters<StampParams>,
    ) -> Result<Json<StampOutput>, ErrorData> {
        let path = PathBuf::from(&params.file_path);
        let did = load_principal_did(&self.paths)?;
        let key = load_signing_key(&self.paths.signing_key).map_err(|e| {
            invalid_request(format!(
                "loading signing key from {}: {e} (run `sec init` first)",
                self.paths.signing_key.display()
            ))
        })?;
        // `allow_test_biometrics=false`: in production the only honored gates are
        // Touch ID (macOS) and explicitly-debug AlwaysAllow/AlwaysDeny. MCP never
        // sees an `--allow-test-biometrics` CLI flag, so this is correct.
        let signer = build_signer(did, key, false)
            .map_err(|e| invalid_request(format!("biometric gate setup failed: {e}")))?;

        let now = Utc::now();
        let outcome = stamp_document(&path, &signer, StampAct::Attest, params.force, now).map_err(
            |e| match e {
                StampError::AlreadyStamped => invalid_request(
                    "file already has a stamp; pass `force: true` to re-stamp".into(),
                ),
                StampError::Signer(SignerError::BiometricRefused) => {
                    invalid_request("biometric refused or cancelled".into())
                }
                other => invalid_request(format!("stamp failed: {other}")),
            },
        )?;

        // Stamp embeds the `$attestation` block in place; the file
        // path is unchanged. Federation runs in the daemon — it picks
        // up envelopes whose frontmatter lacks `delivered:` (regardless
        // of whether they're stamped) and writes the field on success.
        info!(file = %outcome.stamped_path.display(), "stamped envelope via MCP");

        Ok(Json(StampOutput {
            stamped_path: outcome.stamped_path.display().to_string(),
            signer: outcome.stamp.signer.as_str().to_string(),
            stamped_at: outcome.stamp.stamped_at.to_rfc3339(),
            doc_hash: outcome.stamp.doc_hash.to_string(),
        }))
    }

    // Note: `list_inbox` and `list_drafts` were tools in 0.2.7-0.2.10
    // (then named `list_outbox`). Moved to resources
    // (`secretariat://compositions`) in 0.2.11 — listing IS reading,
    // so resource semantics fit; the model fetches via
    // `resources/read` rather than `tools/call`.

    // Note: `defer` was a tool in 0.2.7-0.2.10. Dropped in 0.2.11 because
    // the bubble-up logic that would make "deferred" semantically distinct
    // from "archived" doesn't exist yet — without it, defer is archive with
    // a different folder name. When bubble-up ships, defer comes back (or
    // archive becomes a parameterized `move_to_queue`, depending on what
    // the right shape is then).

    #[tool(
        name = "archive",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Archive an envelope — move it from its queue's \
        `envelopes/` tree into the sibling `archived/` directory. Use during a \
        review session when the principal says 'handled' / 'ignore' / 'done \
        with this'. Files stay on disk for history; just out of the active \
        queue. Idempotent."
    )]
    async fn archive(
        &self,
        Parameters(params): Parameters<InboxActionParams>,
    ) -> Result<Json<InboxActionOutput>, ErrorData> {
        let path = PathBuf::from(&params.file_path);
        let moved =
            archive_envelope(&path).map_err(|e| invalid_request(format!("archive failed: {e}")))?;
        info!(file = %path.display(), to = %moved.display(), "archived envelope via MCP");
        Ok(Json(InboxActionOutput {
            moved_to: moved.display().to_string(),
            note: "Envelope archived. Out of the active queue; kept on disk for history."
                .to_string(),
        }))
    }

    #[tool(
        name = "unarchive",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Reverse of `archive` — move an envelope from its queue's \
        `archived/` directory back into `envelopes/`. Use when the principal \
        revisits an archived item and wants it back in the active queue. \
        Destination is flat under `envelopes/` (date shard not reconstructed). \
        Idempotent."
    )]
    async fn unarchive(
        &self,
        Parameters(params): Parameters<InboxActionParams>,
    ) -> Result<Json<InboxActionOutput>, ErrorData> {
        let path = PathBuf::from(&params.file_path);
        let moved = unarchive_envelope(&path)
            .map_err(|e| invalid_request(format!("unarchive failed: {e}")))?;
        info!(file = %path.display(), to = %moved.display(), "unarchived envelope via MCP");
        Ok(Json(InboxActionOutput {
            moved_to: moved.display().to_string(),
            note: "Envelope unarchived. Back in the active queue under `envelopes/`.".to_string(),
        }))
    }

    #[tool(
        name = "read",
        annotations(read_only_hint = true, open_world_hint = false),
        description = "Decrypt and return the body of an envelope file. Works for both \
        plaintext and X25519/XChaCha20-Poly1305 encrypted envelopes. Decryption uses the \
        principal's local signing key."
    )]
    async fn read(
        &self,
        Parameters(params): Parameters<ReadParams>,
    ) -> Result<Json<ReadOutput>, ErrorData> {
        let path = PathBuf::from(&params.file_path);
        let result = read_envelope(&path, &self.paths.signing_key)
            .map_err(|e| invalid_request(format!("read failed: {e}")))?;
        Ok(Json(ReadOutput {
            body: result.body,
            envelope_from: result.envelope_from.map(|d| d.as_str().to_string()),
            envelope_to: result.envelope_to.map(|d| d.as_str().to_string()),
            encrypted: result.was_encrypted,
        }))
    }

    #[tool(
        name = "verify",
        annotations(read_only_hint = true, open_world_hint = true),
        description = "Verify an envelope file with the substrate-for-themia three-state \
        layered verifier. Returns BOTH layers independently: \
        `signature` (author signature, mandatory on post-Move-2 envelopes) + \
        `stamp` (principal Touch-ID attestation, selective). Each layer reports one of: \
        ok, okUnverifiedAgent (signature only; agent→principal binding not yet wired), \
        tampered, signerUnresolvable, invalid, none."
    )]
    async fn verify(
        &self,
        Parameters(params): Parameters<VerifyParams>,
    ) -> Result<Json<VerifyOutput>, ErrorData> {
        use secretariat_core::infrastructure::identity_store::load_identity;
        let resolver =
            CompositeDidResolver::new(DidWebResolver::new(self.paths.peers_cache.clone()));
        let path = PathBuf::from(&params.file_path);
        let local_did = load_identity(&self.paths.identity_md)
            .ok()
            .flatten()
            .map(|id| id.did);
        let outcome =
            verify_document_layered(&path, &resolver, local_did.as_ref(), Some(&self.paths.root))
                .map_err(|e| invalid_request(format!("verify failed: {e}")))?;
        Ok(Json(layered_outcome_to_view(outcome)))
    }

    // Note: `list_contacts`, `add_contact`, and `secretariat://contacts`
    // were removed in the substrate-for-themia slice (Move 3b). DM /
    // peer / bilateral correspondence primitives are gone — recipients
    // address by DID (or, soon, by AT-URI channel address) directly.

    #[tool(
        name = "invite",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        ),
        description = "Create a one-shot invite token at the relay. The principal \
        must already be a registered tenant of `endpoint` (or the first registered \
        relay in relay-state.json). Returns a claim URL the principal can share \
        with a peer. Default TTL is 168 hours (7 days). Pair with `accept_invite` \
        on the recipient side."
    )]
    async fn invite(
        &self,
        Parameters(params): Parameters<InviteParams>,
    ) -> Result<Json<InviteOutput>, ErrorData> {
        let did = load_principal_did(&self.paths)?;
        let key = load_signing_key(&self.paths.signing_key).map_err(|e| {
            invalid_request(format!(
                "loading signing key from {}: {e} (run `sec init` first)",
                self.paths.signing_key.display()
            ))
        })?;
        let endpoint = match params.endpoint {
            Some(s) => s,
            None => first_registered_relay(&self.paths.relay_state)?,
        };

        let invite = create_invite(
            &endpoint,
            &did,
            &key,
            params.purpose.as_deref(),
            params.ttl_hours,
            None,
        )
        .map_err(|e| invalid_request(format!("create_invite failed: {e}")))?;

        info!(token = %invite.token, "invite created via MCP");

        Ok(Json(InviteOutput {
            token: invite.token,
            claim_url: invite.claim_url,
            expires_at: invite.expires_at.to_rfc3339(),
        }))
    }

    // Note: `init`, `daemon_install`, `daemon_status` were tools in
    // 0.2.7-0.2.9. Removed in 0.2.10 — Tauri owns identity onboarding
    // (tray-anchored popover) and daemon lifecycle (silent-wire on app
    // launch with version-aware marker). CLI has `sec init` /
    // `sec daemon install` / `sec daemon status` for headless use.
    // MCP doesn't add a third path.

    #[tool(
        name = "accept_invite",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        ),
        description = "Accept an invite issued by another principal. Auto-registers \
        the local DID with the relay if not already registered. Returns inviter DID \
        + acceptance metadata. Pair with `invite` on the inviter side."
    )]
    async fn accept_invite(
        &self,
        Parameters(params): Parameters<AcceptInviteParams>,
    ) -> Result<Json<AcceptInviteOutput>, ErrorData> {
        let _ = params.name; // accepted for backward compat, no-op now
        let did = load_principal_did(&self.paths)?;
        let key = load_signing_key(&self.paths.signing_key).map_err(|e| {
            invalid_request(format!(
                "loading signing key from {}: {e} (run `sec init` first)",
                self.paths.signing_key.display()
            ))
        })?;

        // Preview first — refuse to claim an already-claimed invite.
        let preview = view_invite(&params.claim_url)
            .map_err(|e| invalid_request(format!("invite preview failed: {e}")))?;
        if let Some(claimed_by) = &preview.claimed_by {
            return Err(invalid_request(format!(
                "invite has already been claimed (by {claimed_by})"
            )));
        }

        let claimed = claim_invite(&params.claim_url, &did, &key)
            .map_err(|e| invalid_request(format!("claim_invite failed: {e}")))?;

        // Persist the relay endpoint in relay-state so the daemon polls it.
        // Contact-book auto-add was removed in the substrate-for-themia
        // slice (Move 3b).
        let endpoint_origin = relay_origin_from_claim_url(&params.claim_url)?;
        if let Ok(mut state) = RelayState::load(&self.paths.relay_state) {
            let entry = state.entry_mut(&endpoint_origin);
            entry.registered = true;
            let _ = state.save(&self.paths.relay_state);
        }

        info!(inviter = %claimed.inviter_did, "invite claimed via MCP");

        Ok(Json(AcceptInviteOutput {
            inviter_did: claimed.inviter_did.as_str().to_string(),
            claimant_did: claimed.claimant_did.as_str().to_string(),
            claimed_at: claimed.claimed_at.to_rfc3339(),
            registered: claimed.registered,
        }))
    }

    #[tool(
        name = "create_org",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        ),
        description = "Create a new organization in the local substrate. The org gets \
        a directory at `~/.secretariat/orgs/<alias>/` containing a `.org` metadata file \
        and a `channels/` subtree. The org is local-only — it does NOT register anywhere \
        on the network. Use this to model logical groupings (a company, a project, a \
        team) the principal coordinates within. \
        \
        Use `create_channel` to populate the org with channels. A bare handle like \
        `foo:bar` becomes a nested directory inside the org's `channels/`."
    )]
    async fn create_org(
        &self,
        Parameters(params): Parameters<CreateOrgParams>,
    ) -> Result<Json<OrgDto>, ErrorData> {
        let alias = OrgAlias::parse(&params.alias)
            .map_err(|e| invalid_request(format!("invalid alias `{}`: {e}", params.alias)))?;
        let did = match params.did.as_deref() {
            None => None,
            Some(s) => Some(
                Did::parse(s).map_err(|e| invalid_request(format!("invalid did `{s}`: {e}")))?,
            ),
        };
        let name = params.name.unwrap_or_else(|| alias.as_str().to_string());
        let description = params.description.unwrap_or_default();
        let org = app_create_org(
            &self.paths.orgs_root,
            alias,
            did,
            name,
            description,
            Utc::now(),
            Some(&self.paths.contract_stub),
        )
        .map_err(|e| invalid_request(format!("create_org failed: {e}")))?;
        info!(alias = %org.alias.as_str(), "org created via MCP");
        Ok(Json(OrgDto {
            alias: org.alias.as_str().to_string(),
            did: org.did.as_ref().map(|d| d.as_str().to_string()),
            name: org.name,
            description: org.description,
            created_at: org.created_at.to_rfc3339(),
        }))
    }

    #[tool(
        name = "list_orgs",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "List every organization the principal has created locally, \
        sorted alphabetically by alias. Each entry includes the alias, optional DID, \
        display name, description, and creation timestamp."
    )]
    async fn list_orgs(&self) -> Result<Json<ListOrgsOutput>, ErrorData> {
        let orgs = app_list_orgs(&self.paths.orgs_root)
            .map_err(|e| invalid_request(format!("list_orgs failed: {e}")))?;
        let orgs = orgs
            .into_iter()
            .map(|o| OrgDto {
                alias: o.alias.as_str().to_string(),
                did: o.did.as_ref().map(|d| d.as_str().to_string()),
                name: o.name,
                description: o.description,
                created_at: o.created_at.to_rfc3339(),
            })
            .collect();
        Ok(Json(ListOrgsOutput { orgs }))
    }

    #[tool(
        name = "delete_org",
        annotations(
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        ),
        description = "Hard-delete an organization's entire directory tree — metadata, \
        all channels, all envelopes, all decrypted history. This is destructive and \
        irreversible. Requires `confirm: true`. \
        \
        Before calling, render the org's name + channel count to the principal and \
        get explicit verbal confirmation. Equivalent to `rm -rf ~/.secretariat/orgs/<alias>/`."
    )]
    async fn delete_org(
        &self,
        Parameters(params): Parameters<DeleteOrgParams>,
    ) -> Result<Json<DeleteOrgOutput>, ErrorData> {
        let alias = OrgAlias::parse(&params.alias)
            .map_err(|e| invalid_request(format!("invalid alias `{}`: {e}", params.alias)))?;
        if !params.confirm {
            return Ok(Json(DeleteOrgOutput {
                alias: alias.as_str().to_string(),
                deleted: false,
                note: "Refusing to delete without `confirm: true`. \
                       Re-call with confirm=true after showing the principal what will be removed."
                    .to_string(),
            }));
        }
        // Surface NotFound as a non-error response so the caller can react cleanly.
        match app_show_org(&self.paths.orgs_root, &alias) {
            Ok(Some(_)) => {}
            Ok(None) => {
                return Ok(Json(DeleteOrgOutput {
                    alias: alias.as_str().to_string(),
                    deleted: false,
                    note: format!("Org `{}` not found.", alias.as_str()),
                }));
            }
            Err(e) => {
                return Err(invalid_request(format!("show_org failed: {e}")));
            }
        }
        app_delete_org(&self.paths.orgs_root, &alias)
            .map_err(|e| invalid_request(format!("delete_org failed: {e}")))?;
        info!(alias = %alias.as_str(), "org deleted via MCP");
        Ok(Json(DeleteOrgOutput {
            alias: alias.as_str().to_string(),
            deleted: true,
            note: "Org tree removed from disk.".to_string(),
        }))
    }

    #[tool(
        name = "agent_add",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        ),
        description = "Grant a new agent (scribe today; future roles reuse the shape) \
        signing authority on the principal's behalf. Generates a fresh did:key keypair, \
        stores the key at `~/.secretariat/identity/agents/<name>/key` (mode 0600), \
        appends an entry to the principal's `authorized_agents`, re-signs the identity \
        record with the principal's active key. \
        \
        Use this on first launch to wire Claude (or any cognition substrate) as the \
        principal's scribe. After this, MCP `compose` signs envelopes with the agent \
        key, not the principal's — Touch ID stays cold for stamping curation only."
    )]
    async fn agent_add(
        &self,
        Parameters(params): Parameters<AgentAddParams>,
    ) -> Result<Json<AgentDto>, ErrorData> {
        use secretariat_core::domain::{AgentName, AgentRole, AgentSubstrate};
        let name = AgentName::parse(&params.name)
            .map_err(|e| invalid_request(format!("invalid agent name `{}`: {e}", params.name)))?;
        let role = AgentRole::parse(params.role.as_deref().unwrap_or("scribe"))
            .map_err(|e| invalid_request(format!("invalid role: {e}")))?;
        let substrate = AgentSubstrate::parse(
            params
                .substrate
                .as_deref()
                .unwrap_or("claude-code")
                .to_string(),
        )
        .map_err(|e| invalid_request(format!("invalid substrate: {e}")))?;
        let agent = app_add_agent(&self.paths, name, role, substrate, Utc::now())
            .map_err(|e| invalid_request(format!("agent_add failed: {e}")))?;
        info!(name = %agent.name, did = %agent.did, "agent added via MCP");
        Ok(Json(AgentDto::from(agent)))
    }

    #[tool(
        name = "agent_list",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "List the principal's authorized agents. Each entry includes the \
        agent's DID, role (scribe today), nickname, substrate identifier (claude-code, etc.), \
        and the timestamp the agent was granted authority. Returns an empty list when \
        no agents have been provisioned (the substrate works without scribes — manual \
        compose stays available)."
    )]
    async fn agent_list(&self) -> Result<Json<ListAgentsOutput>, ErrorData> {
        let agents = app_list_agents(&self.paths)
            .map_err(|e| invalid_request(format!("agent_list failed: {e}")))?;
        Ok(Json(ListAgentsOutput {
            agents: agents.into_iter().map(AgentDto::from).collect(),
        }))
    }

    #[tool(
        name = "agent_remove",
        annotations(
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        ),
        description = "Remove an agent by nickname. The agent's signing key file is \
        archived (renamed with a timestamp suffix) rather than deleted, preserving the \
        audit trail. The principal's identity record is re-signed to remove the entry. \
        \
        After removal, any envelope signed by the removed agent will fail the \
        `agent → principal` binding check on the receiver side — receivers will see the \
        signature as valid but the agent as unauthorized."
    )]
    async fn agent_remove(
        &self,
        Parameters(params): Parameters<AgentRemoveParams>,
    ) -> Result<Json<AgentDto>, ErrorData> {
        use secretariat_core::domain::AgentName;
        let name = AgentName::parse(&params.name)
            .map_err(|e| invalid_request(format!("invalid agent name `{}`: {e}", params.name)))?;
        let removed = app_remove_agent(&self.paths, &name, Utc::now())
            .map_err(|e| invalid_request(format!("agent_remove failed: {e}")))?;
        info!(name = %removed.name, "agent removed via MCP");
        Ok(Json(AgentDto::from(removed)))
    }

    #[tool(
        name = "agent_rotate",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        ),
        description = "Rotate an agent's keypair. Generates a fresh did:key, archives \
        the prior key file (timestamped), updates the agent's DID in `authorized_agents`. \
        Name, role, and substrate are preserved. \
        \
        Use when key compromise is suspected, or as part of routine key hygiene. \
        Envelopes signed before rotation remain verifiable against the archived key; \
        new envelopes must use the fresh key."
    )]
    async fn agent_rotate(
        &self,
        Parameters(params): Parameters<AgentRemoveParams>,
    ) -> Result<Json<AgentDto>, ErrorData> {
        use secretariat_core::domain::AgentName;
        let name = AgentName::parse(&params.name)
            .map_err(|e| invalid_request(format!("invalid agent name `{}`: {e}", params.name)))?;
        let rotated = app_rotate_agent(&self.paths, &name, Utc::now())
            .map_err(|e| invalid_request(format!("agent_rotate failed: {e}")))?;
        info!(name = %rotated.name, new_did = %rotated.did, "agent rotated via MCP");
        Ok(Json(AgentDto::from(rotated)))
    }

    #[tool(
        name = "create_channel",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        ),
        description = "Create a channel inside an org (or in the principal's personal \
        tree if `org` is omitted). Writes a `channel.md` manifest (YAML frontmatter + \
        markdown body) and pre-creates the `envelopes/` directory so the channel is \
        visible in `list_channels` even before any captures land. \
        \
        The `handle` is a colon-pathed handle, e.g. `product:data:baux-commerciaux`. \
        Use `name` and `description` to provide human-readable metadata."
    )]
    async fn create_channel(
        &self,
        Parameters(params): Parameters<CreateChannelParams>,
    ) -> Result<Json<ChannelDefDto>, ErrorData> {
        let handle = QueueHandle::parse(&params.handle)
            .map_err(|e| invalid_request(format!("invalid handle `{}`: {e}", params.handle)))?;
        let root = self.resolve_channels_root(params.org.as_deref())?;
        // Default channel name = the bare slug itself (last segment for
        // nested handles, the whole handle for single-segment ones).
        let name = params
            .name
            .unwrap_or_else(|| handle.segments().last().copied().unwrap_or("").to_string());
        let description = params.description.unwrap_or_default();
        let def = app_create_channel(
            &root,
            handle,
            name,
            description,
            Utc::now(),
            Some(&self.paths.contract_stub),
        )
        .map_err(|e| invalid_request(format!("create_channel failed: {e}")))?;
        info!(handle = %def.handle.as_str(), "channel created via MCP");
        Ok(Json(ChannelDefDto {
            handle: def.handle.as_str().to_string(),
            name: def.name,
            description: def.description,
            created_at: def.created_at.to_rfc3339(),
        }))
    }

    #[tool(
        name = "delete_channel",
        annotations(
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Hard-delete a channel's entire directory tree — `channel.md`, \
        envelopes, sub-channels, everything beneath the handle's path. Destructive and \
        irreversible. Requires `confirm: true`. \
        \
        Before calling, render the channel handle + envelope count to the principal and \
        get explicit verbal confirmation."
    )]
    async fn delete_channel(
        &self,
        Parameters(params): Parameters<DeleteChannelParams>,
    ) -> Result<Json<DeleteChannelOutput>, ErrorData> {
        let handle = QueueHandle::parse(&params.handle)
            .map_err(|e| invalid_request(format!("invalid handle `{}`: {e}", params.handle)))?;
        let org_str = params.org.clone();
        if !params.confirm {
            return Ok(Json(DeleteChannelOutput {
                handle: handle.as_str().to_string(),
                org: org_str,
                deleted: false,
                note: "Refusing to delete without `confirm: true`.".to_string(),
            }));
        }
        let root = self.resolve_channels_root(params.org.as_deref())?;
        app_delete_channel(&root, &handle)
            .map_err(|e| invalid_request(format!("delete_channel failed: {e}")))?;
        info!(handle = %handle.as_str(), "channel deleted via MCP");
        Ok(Json(DeleteChannelOutput {
            handle: handle.as_str().to_string(),
            org: org_str,
            deleted: true,
            note: "Channel tree removed from disk.".to_string(),
        }))
    }

    #[tool(
        name = "get_channel_contract",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Read this principal's private consumption contract for a channel — \
        the `<channel-dir>/contract.local.md` file declaring how *I* poll, filter, and \
        surface this channel's traffic. Auto-scaffolds an empty-contribution stub if the \
        file is absent (which only happens for channels created before the contract \
        primitive shipped). Returns the contract's frontmatter fields plus the body prose.

        This file is **never sent on wire** and **never shared with other roster members**. \
        Channel governance (roster, channel-wide artifact policy) lives separately in \
        `channel.md` or future signed governance envelopes — not surfaced by this tool."
    )]
    async fn get_channel_contract(
        &self,
        Parameters(params): Parameters<GetChannelContractParams>,
    ) -> Result<Json<ContractDto>, ErrorData> {
        let handle = QueueHandle::parse(&params.handle)
            .map_err(|e| invalid_request(format!("invalid handle `{}`: {e}", params.handle)))?;
        let root = self.resolve_channels_root(params.org.as_deref())?;
        let view = app_get_channel_contract(&root, &handle)
            .map_err(|e| invalid_request(format!("get_channel_contract failed: {e}")))?;
        Ok(Json(ContractDto::from_view(
            handle.as_str().to_string(),
            view,
        )))
    }

    #[tool(
        name = "set_channel_contract",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Update this principal's private consumption contract for a channel. \
        Partial-merge semantics: fields you don't mention are left untouched. To revert a \
        field to inheriting from ancestors, include its name in `clear`. Listing a field \
        in `clear` AND passing a value for it in the same call is rejected.

        Allowed `clear` entries: `cadence_floor_minutes`, `min_trust`. \
        Allowed `min_trust` values: `signed-only`, `stamp-required`.

        Body prose is preserved across calls — only frontmatter mutates."
    )]
    async fn set_channel_contract(
        &self,
        Parameters(params): Parameters<SetChannelContractParams>,
    ) -> Result<Json<ContractDto>, ErrorData> {
        let handle = QueueHandle::parse(&params.handle)
            .map_err(|e| invalid_request(format!("invalid handle `{}`: {e}", params.handle)))?;
        let root = self.resolve_channels_root(params.org.as_deref())?;
        let patch = build_contract_patch(
            params.cadence_floor_minutes,
            params.min_trust.as_deref(),
            &params.clear,
        )?;
        if patch.is_noop() {
            return Err(invalid_request(
                "no fields to set — pass at least one of cadence_floor_minutes / min_trust / clear"
                    .to_string(),
            ));
        }
        let view = app_set_channel_contract(&root, &handle, patch)
            .map_err(|e| invalid_request(format!("set_channel_contract failed: {e}")))?;
        info!(handle = %handle.as_str(), "channel contract updated via MCP");
        Ok(Json(ContractDto::from_view(
            handle.as_str().to_string(),
            view,
        )))
    }

    #[tool(
        name = "resolve_channel_contract",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Walk this principal's contract chain (org-root → ancestor channel \
        directories → leaf) and return the merged consumption view per the accumulate \
        rules: `cadence_floor_minutes` takes the MAX (tightest floor wins), `min_trust` \
        takes the MAX-RESTRICTIVE (`stamp-required` dominates `signed-only`). \
        \
        Returns both the merged values (what to enforce) and the per-level breakdown \
        (which file contributed what). Empty-frontmatter levels still appear in the chain \
        — they just contribute nothing to the merge."
    )]
    async fn resolve_channel_contract(
        &self,
        Parameters(params): Parameters<ResolveChannelContractParams>,
    ) -> Result<Json<ResolvedContractDto>, ErrorData> {
        let handle = QueueHandle::parse(&params.handle)
            .map_err(|e| invalid_request(format!("invalid handle `{}`: {e}", params.handle)))?;
        let alias = match params.org.as_deref() {
            None => None,
            Some(s) => Some(
                OrgAlias::parse(s)
                    .map_err(|e| invalid_request(format!("invalid alias `{s}`: {e}")))?,
            ),
        };
        let resolved = app_resolve_channel_contract(
            &self.paths.orgs_root,
            &self.paths.personal_channels_root(),
            alias.as_ref(),
            &handle,
        )
        .map_err(|e| invalid_request(format!("resolve_channel_contract failed: {e}")))?;
        Ok(Json(ResolvedContractDto::from_resolved(
            handle.as_str().to_string(),
            resolved,
        )))
    }

    #[tool(
        name = "get_org_contract",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Read this principal's private consumption contract at the org root \
        (`<org-dir>/contract.local.md`). Org-root overrides accumulate down to all channels \
        in this org — set a value here once instead of repeating it on every channel."
    )]
    async fn get_org_contract(
        &self,
        Parameters(params): Parameters<GetOrgContractParams>,
    ) -> Result<Json<ContractDto>, ErrorData> {
        let alias = OrgAlias::parse(&params.org)
            .map_err(|e| invalid_request(format!("invalid alias `{}`: {e}", params.org)))?;
        let view = app_get_org_contract(&self.paths.orgs_root, &alias)
            .map_err(|e| invalid_request(format!("get_org_contract failed: {e}")))?;
        Ok(Json(ContractDto::from_view(
            alias.as_str().to_string(),
            view,
        )))
    }

    #[tool(
        name = "set_org_contract",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Update this principal's private consumption contract at the org root. \
        Same partial-merge + `clear` semantics as `set_channel_contract`. Org-root overrides \
        accumulate down to descendant channels per [[project-contracts-accumulate]]."
    )]
    async fn set_org_contract(
        &self,
        Parameters(params): Parameters<SetOrgContractParams>,
    ) -> Result<Json<ContractDto>, ErrorData> {
        let alias = OrgAlias::parse(&params.org)
            .map_err(|e| invalid_request(format!("invalid alias `{}`: {e}", params.org)))?;
        let patch = build_contract_patch(
            params.cadence_floor_minutes,
            params.min_trust.as_deref(),
            &params.clear,
        )?;
        if patch.is_noop() {
            return Err(invalid_request(
                "no fields to set — pass at least one of cadence_floor_minutes / min_trust / clear"
                    .to_string(),
            ));
        }
        let view = app_set_org_contract(&self.paths.orgs_root, &alias, patch)
            .map_err(|e| invalid_request(format!("set_org_contract failed: {e}")))?;
        info!(alias = %alias.as_str(), "org contract updated via MCP");
        Ok(Json(ContractDto::from_view(
            alias.as_str().to_string(),
            view,
        )))
    }

    #[tool(
        name = "daemon_tick",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = true
        ),
        description = "Run one sync cycle against every registered relay: poll inbound \
        envelopes on the principal's subscribed org channels. Idempotent and safe to \
        call repeatedly. \
        \
        Prefers the running daemon's IPC socket so it doesn't race the daemon's own poll \
        loop on `RelayState` saves; falls back to running the cycle in-proc when no \
        daemon is reachable (same fallback the CLI's `sec daemon tick` uses)."
    )]
    async fn daemon_tick(&self) -> Result<Json<DaemonTickOutput>, ErrorData> {
        let did = load_principal_did(&self.paths)?;
        let key = load_signing_key(&self.paths.signing_key).map_err(|e| {
            invalid_request(format!(
                "loading signing key from {}: {e}",
                self.paths.signing_key.display()
            ))
        })?;
        secretariat_daemon::ipc::tick_via_ipc_or_inproc(&self.paths, &did, &key)
            .await
            .map_err(|e| invalid_request(format!("daemon tick: {e}")))?;
        Ok(Json(DaemonTickOutput {
            note: "tick completed; new envelopes (if any) have landed under their \
                   target queues — surface them via `/review` or `secretariat://orgs`"
                .to_string(),
        }))
    }

    #[tool(
        name = "daemon_status",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Report whether the long-running daemon is reachable on its IPC \
        socket and list the registered relays + their cursor positions. Useful for \
        diagnosing 'why hasn't my message arrived' before falling back to `daemon_tick`. \
        \
        Read-only — doesn't trigger sync."
    )]
    async fn daemon_status(&self) -> Result<Json<DaemonStatusOutput>, ErrorData> {
        let running = secretariat_daemon::ipc::is_running(&self.paths).await;
        let state = RelayState::load(&self.paths.relay_state).map_err(|e| {
            invalid_request(format!(
                "loading relay state from {}: {e}",
                self.paths.relay_state.display()
            ))
        })?;
        let relays = state
            .iter()
            .map(|r| DaemonRelayStatus {
                endpoint: r.endpoint.clone(),
                registered: r.registered,
                queues_tracked: r.queue_cursors.len(),
                max_cursor: r.queue_cursors.iter().map(|q| q.cursor).max().unwrap_or(0),
            })
            .collect();
        Ok(Json(DaemonStatusOutput {
            daemon_reachable: running,
            relays,
        }))
    }
}

// ---------------------------------------------------------------------------
// Resource URIs — kept here so the prompt bodies and resource handlers
// stay in sync.
// ---------------------------------------------------------------------------

const RESOURCE_ORGS_URI: &str = "secretariat://orgs";
const RESOURCE_COMPOSITIONS_URI: &str = "secretariat://compositions";

fn build_resource(uri: &str, name: &str, description: &str) -> Resource {
    Annotated::new(
        RawResource {
            uri: uri.to_string(),
            name: name.to_string(),
            title: Some(name.to_string()),
            description: Some(description.to_string()),
            mime_type: Some("text/markdown".to_string()),
            size: None,
            icons: None,
        },
        None,
    )
}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for SecretariatServer {
    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let mut resources = Vec::new();

        // Orgs — the channel tree directory. Fetch before routing a capture
        // or composing to a channel so you know what orgs and channels exist.
        if self.paths.orgs_root.exists() {
            resources.push(build_resource(
                RESOURCE_ORGS_URI,
                "Orgs",
                "All orgs the principal has set up, each with its channel tree. \
                 Fetch before routing a capture to an org channel or composing \
                 to an org context — tells you what orgs and channels exist.",
            ));
        }

        // Compositions — pending drafts awaiting stamp. Fetch only when the
        // principal explicitly asks to review pending work.
        resources.push(build_resource(
            RESOURCE_COMPOSITIONS_URI,
            "Compositions",
            "Pending drafts — envelopes whose frontmatter lacks `delivered:` — \
             across every queue's `envelopes/YYYY/MM/DD/` tree, rendered with \
             subject, recipient, and age. Fetch ONLY when the principal asks \
             'what drafts do I have?' or initiates a stamp session.",
        ));

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        let text = match request.uri.as_str() {
            RESOURCE_ORGS_URI => render_orgs(&self.paths.orgs_root)?,
            RESOURCE_COMPOSITIONS_URI => render_compositions(&self.paths.root)?,
            other => {
                return Err(ErrorData::new(
                    ErrorCode::INVALID_REQUEST,
                    format!("unknown resource uri: {other}"),
                    None,
                ));
            }
        };

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(text, request.uri)],
        })
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .enable_resources()
                .build(),
            server_info: Implementation {
                name: "secretariat".to_string(),
                title: Some("Secretariat".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some(SERVER_INSTRUCTIONS.to_string()),
        }
    }
}

/// Top-level guidance the MCP client (Claude Code, Cursor, claude.ai, …) sees
/// once at session start. The principal-facing rituals — review session,
/// stamp ceremony, onboarding — belong in MCP prompts, not here. This block
/// is just the framing the model needs to call any tool correctly.
const SERVER_INSTRUCTIONS: &str = "\
Secretariat is ambient context for AI, stamped by humans. You live in the \
context stream — read and draft continuously; the principal only enters to \
stamp the moments that count. You are the scribe; the principal stamps, you \
never do. Two channel-tree roots (Move 3c): envelopes addressed to a queue the \
principal owns live under `<root>/channels/<handle-path>/envelopes/YYYY/MM/DD/<rkey>.md`; \
envelopes addressed to an org-owned channel live under \
`<root>/orgs/<org-alias>/channels/<handle-path>/envelopes/YYYY/MM/DD/<rkey>.md`. \
Draft state is the absence of the envelope frontmatter's `delivered:` field; \
the daemon writes that field after federation succeeds. Stamping embeds an \
`$attestation` block in place — no rename, no path change. Stamping is gated \
by Touch ID.

Stamp ceremony (mandatory before calling `stamp`):
  1. Call `read` on the same `file_path`.
  2. Render the FULL decrypted body verbatim — code block or quoted region, \
never a summary.
  3. Wait for explicit consent in the same turn (e.g. \"stamp it\"). Implicit \
consent from a prior turn does not count if the file changed.
  4. Only then call `stamp`. The Touch ID dialog reason carries the \
document's first-line headline + a short hash prefix; if it differs from what \
you displayed, abort.

Cadence: Secretariat is for low-cadence, intentional review. Do not fetch \
`secretariat://compositions` proactively — only when the principal asks \
(\"any drafts pending?\", \"what's waiting for stamp?\"). Do not fetch \
`secretariat://orgs` between unrelated requests. \
Captures (`capture`) stay local and CANNOT be stamped — use them for \
ideas/journal entries the principal will revisit at the next review session. \
Always `verify` inbound envelopes before trusting their content.";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn internal_error(msg: String) -> ErrorData {
    ErrorData::new(ErrorCode::INTERNAL_ERROR, msg, None)
}

fn render_orgs(orgs_root: &std::path::Path) -> Result<String, ErrorData> {
    let orgs = app_list_orgs(orgs_root).map_err(|e| internal_error(format!("list_orgs: {e}")))?;
    if orgs.is_empty() {
        return Ok("# Orgs\n\nNo orgs yet. Use `create_org` to set one up.\n".to_string());
    }
    let mut out = String::from("# Orgs\n\n");
    for org in &orgs {
        let name_part = if org.name.is_empty() {
            String::new()
        } else {
            format!(" — {}", org.name)
        };
        let did_part = org
            .did
            .as_ref()
            .map(|d| format!(" · `{}`", d.as_str()))
            .unwrap_or_default();
        out.push_str(&format!(
            "## {}{}{}\n\n",
            org.alias.as_str(),
            name_part,
            did_part
        ));

        let org_channels_root = orgs_root.join(org.alias.as_str()).join("channels");
        let channels = list_channels(&org_channels_root).unwrap_or_default();
        if channels.is_empty() {
            out.push_str("_No channels yet._\n\n");
        } else {
            for ch in &channels {
                let label = if ch.name.is_empty() {
                    ch.handle.clone()
                } else {
                    format!("{} ({})", ch.handle, ch.name)
                };
                let count = ch.envelope_count;
                out.push_str(&format!(
                    "- `{label}` · {count} envelope{}\n",
                    if count == 1 { "" } else { "s" }
                ));
            }
            out.push('\n');
        }
    }
    Ok(out)
}

fn render_compositions(root: &std::path::Path) -> Result<String, ErrorData> {
    let drafts = list_draft_files(root).map_err(|e| internal_error(format!("list_drafts: {e}")))?;
    if drafts.is_empty() {
        return Ok("# Compositions\n\n_No pending drafts._\n".to_string());
    }
    let mut out = format!(
        "# Compositions\n\n{} pending draft{}:\n\n",
        drafts.len(),
        if drafts.len() == 1 { "" } else { "s" }
    );
    for d in &drafts {
        out.push_str(&format!("- `{}`", d.file_path));
        if let Some(handle) = &d.queue {
            out.push_str(&format!(" → `{handle}`"));
        }
        if d.stamped {
            out.push_str(" · stamped ✓");
        } else {
            out.push_str(" · awaiting stamp");
        }
        if d.encrypted {
            out.push_str(" · encrypted");
        }
        out.push('\n');
    }
    Ok(out)
}

fn invalid_request(msg: String) -> ErrorData {
    ErrorData::new(ErrorCode::INVALID_REQUEST, msg, None)
}

fn build_contract_patch(
    cadence_floor_minutes: Option<u32>,
    min_trust: Option<&str>,
    clear: &[String],
) -> Result<ContractPatch, ErrorData> {
    let clear_cadence = clear.iter().any(|f| f == "cadence_floor_minutes");
    let clear_min_trust = clear.iter().any(|f| f == "min_trust");
    for f in clear {
        if f != "cadence_floor_minutes" && f != "min_trust" {
            return Err(invalid_request(format!(
                "unknown clear field `{f}` (allowed: cadence_floor_minutes, min_trust)"
            )));
        }
    }
    if clear_cadence && cadence_floor_minutes.is_some() {
        return Err(invalid_request(
            "conflict: cadence_floor_minutes set AND listed in `clear`".into(),
        ));
    }
    if clear_min_trust && min_trust.is_some() {
        return Err(invalid_request(
            "conflict: min_trust set AND listed in `clear`".into(),
        ));
    }
    let cadence = match (cadence_floor_minutes, clear_cadence) {
        (Some(n), false) => PatchField::Set(n),
        (None, true) => PatchField::Clear,
        (None, false) => PatchField::Leave,
        (Some(_), true) => unreachable!("guarded above"),
    };
    let trust = match (min_trust, clear_min_trust) {
        (Some(s), false) => PatchField::Set(TrustGate::parse(s).ok_or_else(|| {
            invalid_request(format!(
                "invalid min_trust `{s}` (want signed-only or stamp-required)"
            ))
        })?),
        (None, true) => PatchField::Clear,
        (None, false) => PatchField::Leave,
        (Some(_), true) => unreachable!("guarded above"),
    };
    Ok(ContractPatch {
        cadence_floor_minutes: cadence,
        min_trust: trust,
    })
}

fn resolve_to_did(_paths: &KeyPaths, to: &str) -> Result<Did, ErrorData> {
    // Substrate-for-themia (Move 3b) removed the contact-book lookup path;
    // recipients are now DIDs (or future channel addresses) only.
    Did::parse(to).map_err(|e| {
        invalid_request(format!(
            "invalid did `{to}`: {e} — recipients must be DIDs (contact-slug lookup removed)"
        ))
    })
}

fn parse_depth(s: Option<&str>) -> Result<EnvelopeDepth, ErrorData> {
    match s {
        None => Ok(EnvelopeDepth::Subtle),
        Some("gross") => Ok(EnvelopeDepth::Gross),
        Some("subtle") => Ok(EnvelopeDepth::Subtle),
        Some(other) => Err(invalid_request(format!(
            "depth must be `gross` or `subtle`, got `{other}`"
        ))),
    }
}

fn parse_urgency(s: Option<&str>) -> Result<EnvelopeUrgency, ErrorData> {
    match s {
        None => Ok(EnvelopeUrgency::Whenever),
        Some("now") => Ok(EnvelopeUrgency::Now),
        Some("soon") => Ok(EnvelopeUrgency::Soon),
        Some("whenever") => Ok(EnvelopeUrgency::Whenever),
        Some(other) => Err(invalid_request(format!(
            "urgency must be `now`, `soon`, or `whenever`, got `{other}`"
        ))),
    }
}

fn load_principal_did(paths: &KeyPaths) -> Result<Did, ErrorData> {
    use secretariat_core::infrastructure::identity_store::load_identity;
    let identity = load_identity(&paths.identity_md)
        .map_err(|e| invalid_request(format!("loading identity: {e}")))?;
    identity.map(|i| i.did).ok_or_else(|| {
        invalid_request(format!(
            "no identity at {} — run `sec init` first",
            paths.identity_md.display()
        ))
    })
}

/// Substrate-for-themia Move 2: resolve the calling agent's key for
/// compose signing. Prefers an agent named by `agent_name`; falls back
/// to the first scribe in `authorized_agents`; ultimate fallback is the
/// principal's own key + `signer_role: principal`.
struct ResolvedComposeSigner {
    signer_did: Did,
    signer_role: secretariat_core::domain::SignerRole,
    signing_key: ed25519_dalek::SigningKey,
}

fn resolve_compose_signer(
    paths: &KeyPaths,
    self_did: &Did,
    agent_name: Option<&str>,
) -> Result<ResolvedComposeSigner, ErrorData> {
    use secretariat_core::domain::{AgentRole, SignerRole};
    use secretariat_core::infrastructure::identity_store::load_identity_verified;

    // Load principal key up front: its verifying key is the canonical
    // truth for verifying identity.md's embedded signature, guarding
    // `authorized_agents` against on-disk tampering before we trust the
    // list to pick a signing agent.
    let principal_key = load_signing_key(&paths.signing_key)
        .map_err(|e| invalid_request(format!("loading principal signing key: {e}")))?;
    let vk = principal_key.verifying_key();
    let identity = load_identity_verified(&paths.identity_md, Some(&vk))
        .map_err(|e| invalid_request(format!("loading identity: {e}")))?
        .ok_or_else(|| {
            invalid_request(format!(
                "no identity at {} — run `sec init` first",
                paths.identity_md.display()
            ))
        })?;

    let chosen = match agent_name {
        Some(name) => Some(
            identity
                .authorized_agents
                .iter()
                .find(|a| a.name.as_str() == name)
                .ok_or_else(|| {
                    invalid_request(format!(
                        "no authorized agent named `{name}` — \
                         check `sec agent list`"
                    ))
                })?,
        ),
        None => identity
            .authorized_agents
            .iter()
            .find(|a| a.role == AgentRole::Scribe),
    };

    match chosen {
        Some(agent) => {
            let key_path = paths.agent_signing_key_path(agent.name.as_str());
            let key = load_signing_key(&key_path).map_err(|e| {
                invalid_request(format!(
                    "loading agent signing key at {}: {e}",
                    key_path.display()
                ))
            })?;
            Ok(ResolvedComposeSigner {
                signer_did: agent.did.clone(),
                signer_role: SignerRole::Agent,
                signing_key: key,
            })
        }
        None => Ok(ResolvedComposeSigner {
            signer_did: self_did.clone(),
            signer_role: SignerRole::Principal,
            signing_key: principal_key,
        }),
    }
}

fn first_registered_relay(path: &std::path::Path) -> Result<String, ErrorData> {
    let state =
        RelayState::load(path).map_err(|e| invalid_request(format!("loading relay-state: {e}")))?;
    let endpoint = state
        .iter()
        .find(|r| r.registered)
        .map(|r| r.endpoint.clone());
    endpoint.ok_or_else(|| {
        invalid_request(
            "no registered relay yet; run `sec daemon register --endpoint <url>` first \
             or pass `endpoint` explicitly"
                .to_string(),
        )
    })
}

fn relay_origin_from_claim_url(claim_url: &str) -> Result<String, ErrorData> {
    let idx = claim_url
        .find("/v0/invite/")
        .ok_or_else(|| invalid_request("claim URL does not contain `/v0/invite/`".to_string()))?;
    Ok(claim_url[..idx].to_string())
}

fn layered_outcome_to_view(outcome: LayeredVerifyOutcome) -> VerifyOutput {
    use secretariat_core::application::SignatureOutcome;
    let signature = match outcome.signature {
        SignatureOutcome::None => VerifyLayer {
            outcome: "none".into(),
            ..Default::default()
        },
        SignatureOutcome::Ok {
            signer,
            signer_role,
            signed_at,
        } => VerifyLayer {
            outcome: "ok".into(),
            signer: Some(signer.as_str().to_string()),
            signer_role: Some(signer_role.as_str().to_string()),
            signed_at: Some(signed_at.to_rfc3339()),
            ..Default::default()
        },
        SignatureOutcome::VerifiedAgent {
            agent,
            principal,
            signed_at,
        } => VerifyLayer {
            outcome: "verifiedAgent".into(),
            signer: Some(agent.as_str().to_string()),
            signer_role: Some("agent".into()),
            principal: Some(principal.as_str().to_string()),
            signed_at: Some(signed_at.to_rfc3339()),
            ..Default::default()
        },
        SignatureOutcome::OkUnverifiedAgent { signer, signed_at } => VerifyLayer {
            outcome: "okUnverifiedAgent".into(),
            signer: Some(signer.as_str().to_string()),
            signer_role: Some("agent".into()),
            signed_at: Some(signed_at.to_rfc3339()),
            ..Default::default()
        },
        SignatureOutcome::Tampered {
            claimed_hash,
            computed_hash,
        } => VerifyLayer {
            outcome: "tampered".into(),
            claimed_hash: Some(claimed_hash.to_string()),
            computed_hash: Some(computed_hash.to_string()),
            ..Default::default()
        },
        SignatureOutcome::SignerUnresolvable { signer, cause } => VerifyLayer {
            outcome: "signerUnresolvable".into(),
            signer: Some(signer.as_str().to_string()),
            cause: Some(cause.to_string()),
            ..Default::default()
        },
        SignatureOutcome::Invalid { signer } => VerifyLayer {
            outcome: "invalid".into(),
            signer: Some(signer.as_str().to_string()),
            ..Default::default()
        },
    };
    let stamp = match outcome.stamp {
        VerifyOutcome::Unsigned => VerifyLayer {
            outcome: "none".into(),
            ..Default::default()
        },
        VerifyOutcome::Verified {
            signer,
            stamped_at,
            act,
        } => VerifyLayer {
            outcome: "ok".into(),
            signer: Some(signer.as_str().to_string()),
            signer_role: Some("principal".into()),
            signed_at: Some(stamped_at.to_rfc3339()),
            act: Some(format!("{act}")),
            ..Default::default()
        },
        VerifyOutcome::Tampered {
            claimed_hash,
            computed_hash,
        } => VerifyLayer {
            outcome: "tampered".into(),
            claimed_hash: Some(claimed_hash.to_string()),
            computed_hash: Some(computed_hash.to_string()),
            ..Default::default()
        },
        VerifyOutcome::SignerUnresolvable { signer, cause } => VerifyLayer {
            outcome: "signerUnresolvable".into(),
            signer: Some(signer.as_str().to_string()),
            cause: Some(cause.to_string()),
            ..Default::default()
        },
        VerifyOutcome::SignatureInvalid { signer } => VerifyLayer {
            outcome: "invalid".into(),
            signer: Some(signer.as_str().to_string()),
            ..Default::default()
        },
    };
    VerifyOutput { signature, stamp }
}
