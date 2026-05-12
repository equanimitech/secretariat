//! Secretariat MCP server — stdio transport.
//!
//! Tools exposed:
//!
//! | Tool | Purpose |
//! |---|---|
//! | `compose` | Write a peer-addressed envelope to the outbox (principal stamps separately) |
//! | `capture` | Drop a body into a local queue (substrate v0.3 — never sent, never stamped without consent) |
//! | `stamp` | Trigger biometric stamp on a draft (Touch ID gates regardless of caller) |
//! | `secretariat://outbox` | Pending drafts (stamped + unstamped) — resource |
//! | `secretariat://inbox`  | Verified inbound envelopes — resource |
//! | `defer` | Move an inbox envelope to `inbox/deferred/` ('remind me later') |
//! | `archive` | Move an inbox envelope to `inbox/archived/` ('handled') |
//! | `read` | Decrypt + return body of an envelope |
//! | `verify` | Check a stamped artifact |
//! | `list_contacts` | Known peers |
//! | `add_contact` | Manual contact entry |
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
    tool, tool_handler, tool_router,
    RoleServer, ServerHandler,
};
use schemars::JsonSchema;
use secretariat_core::application::{
    add_contact, archive_envelope, capture_to_queue, claim_invite, compose_envelope,
    create_channel as app_create_channel, create_invite, create_org as app_create_org,
    delete_channel as app_delete_channel, delete_org as app_delete_org, find_by_slug,
    list_channels, list_contacts, list_inbox_files, list_orgs as app_list_orgs,
    list_outbox_files, read_channel, read_envelope, show_org as app_show_org, stamp_document,
    try_contextify_after_capture, verify_document, view_invite, CaptureRequest, CaptureRoots,
    ComposeRequest, ListedEnvelope, StampError, VerifyOutcome,
};
use secretariat_core::domain::{OrgAlias, QueueHandle, Recipient, StampAct};
use secretariat_core::infrastructure::org_store::org_channels_root;
use secretariat_core::infrastructure::biometric::build_signer;
use secretariat_core::infrastructure::composite_did_resolver::CompositeDidResolver;
use secretariat_core::infrastructure::did_web_resolver::DidWebResolver;
use secretariat_core::infrastructure::keys::{load_signing_key, KeyPaths};
use secretariat_core::infrastructure::transport::RelayState;
use secretariat_core::ports::SignerError;
use secretariat_core::{Contact, Did, DisplayName, EnvelopeDepth, EnvelopeUrgency, RelayEndpoint};
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
            None => Ok(self.paths.channels.clone()),
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
//   /review                     — paced walker over inbox / outbox.
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
    /// `queue: inbox:triage`.
    #[prompt(name = "idea")]
    pub async fn idea_prompt(&self) -> Result<Vec<PromptMessage>, ErrorData> {
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            include_str!("prompts/idea.md"),
        )])
    }

    /// Capture a bug, friction, or improvement. Routes through the
    /// `capture` tool with `queue: inbox:pain`.
    #[prompt(name = "pain")]
    pub async fn pain_prompt(&self) -> Result<Vec<PromptMessage>, ErrorData> {
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            include_str!("prompts/pain.md"),
        )])
    }

    /// Walk the principal through a paced review session — verify, read,
    /// render, and act on inbox / outbox envelopes one at a time.
    #[prompt(name = "review")]
    pub async fn review_prompt(&self) -> Result<Vec<PromptMessage>, ErrorData> {
        Ok(vec![PromptMessage::new_text(
            PromptMessageRole::User,
            include_str!("prompts/review.md"),
        )])
    }

    /// Draft an envelope to a peer using the principal's
    /// attentional-granularity template, with the inline-render-first
    /// consent gate before the draft hits disk.
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
    /// someone's invitation). Both auto-add the peer to the contact book.
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
    /// Recipient DID (`did:web:...` / `did:key:...`) OR a contact's
    /// display-name slug (case-insensitive).
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
    /// Optional cadence hint for the recipient.
    #[serde(default)]
    pub cadence_hint: Option<String>,
    /// Recipient queue handle on the peer's machine. Defaults to
    /// `inbox:default` (the conventional handle for direct messages).
    /// Specify a different handle to post to a non-default queue the
    /// peer owns — e.g. a channel they publish.
    #[serde(default)]
    pub handle: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ComposeOutput {
    pub file_path: String,
    pub note: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureParams {
    /// Target queue handle, of the form `<namespace>:<slug>[:<segment>...]`
    /// — e.g. `inbox:triage`, `area:health`,
    /// `channel:dommage-corporel:paris-cohort`. Namespaces are free-form.
    pub queue: String,
    /// Body of the capture (markdown-friendly plain text).
    pub body: String,
    /// Free-form origin marker, e.g. `idea-skill`, `quick-pane`. Defaults
    /// to `mcp-capture` when omitted.
    #[serde(default)]
    pub source: Option<String>,
    /// Optional org alias (`themia.pro`, `equanimi.tech`). When set AND
    /// the queue handle starts with `channel:`, the capture lands inside
    /// that org's channel tree. Omit (or set null) for personal captures.
    #[serde(default)]
    pub org: Option<String>,
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
    /// directly under `~/.secretariat/inbox/` — not in a subdir.
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
    /// Absolute path to the draft to stamp. Typically lives under
    /// `~/.secretariat/outbox/<recipient-did>/`.
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VerifyParams {
    pub file_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct VerifyOutput {
    pub outcome: String,
    pub signer: Option<String>,
    pub stamped_at: Option<String>,
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
    /// Display name to give the inviter in the local contact book.
    /// Defaults to the host portion of their DID.
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
    /// Whether the inviter was added to the local contact book.
    pub contact_added: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListChannelsOutput {
    /// Channels with at least one envelope, sorted by latest activity
    /// (newest first).
    pub channels: Vec<ChannelSummaryDto>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ChannelSummaryDto {
    /// Canonical handle, e.g. `channel:secretariat:dev`.
    pub handle: String,
    /// Human-readable display name from `.channelDef` (empty if unset).
    pub name: String,
    /// Free-form description from `.channelDef` (empty if unset).
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
    /// Channel handle, e.g. `channel:secretariat:dev`. Must start with
    /// the `channel:` top namespace.
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
    /// Channel handle, e.g. `channel:product:data:baux-commerciaux`.
    /// Must start with `channel:`.
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
        description = "Compose a draft envelope to a recipient and write it to the outbox. \
        The principal stamps it separately (biometric-gated, never via this tool). \
        `to` accepts either a DID or a contact's display-name slug."
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

        let handle_str = params.handle.as_deref().unwrap_or("inbox:default");
        let handle = QueueHandle::parse(handle_str).map_err(|e| {
            invalid_request(format!("invalid `handle` `{handle_str}`: {e}"))
        })?;

        let req = ComposeRequest {
            from,
            recipient: Recipient::new(to, handle),
            depth,
            urgency,
            source: params.source.unwrap_or_else(|| "mcp".to_string()),
            cadence_hint: params.cadence_hint,
            body,
        };

        let path = compose_envelope(req, &self.paths.template, &self.paths.outbox, Utc::now())
            .map_err(|e| invalid_request(format!("compose failed: {e}")))?;

        info!(file = %path.display(), "composed envelope via MCP");

        Ok(Json(ComposeOutput {
            file_path: path.display().to_string(),
            note: "Draft written to outbox. Show the body to the principal, get explicit \
                   confirmation, then stamp via the `stamp` tool (biometric-gated). The daemon \
                   will deliver after stamping; on macOS the LaunchAgent polls every 15 minutes."
                .to_string(),
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
        envelopes whose owner is the principal themselves — same primitive as a \
        peer letter, but the routing rule keeps them on disk. Use for ideas, \
        journal entries, future-self notes, anything to surface again at the \
        next review session. Stamps are optional (tamper-evident self-attestation), \
        never required. \
        \
        The `queue` parameter is a `<namespace>:<slug>` handle; namespaces are \
        free-form (e.g. `inbox:triage`, `area:health`, `project:autonomous-enterprise`). \
        If you don't know which to pick, default to `inbox:triage`."
    )]
    async fn capture(
        &self,
        Parameters(params): Parameters<CaptureParams>,
    ) -> Result<Json<CaptureOutput>, ErrorData> {
        let from = load_principal_did(&self.paths)?;
        let queue = QueueHandle::parse(&params.queue).map_err(|e| {
            invalid_request(format!("invalid `queue` `{}`: {e}", params.queue))
        })?;

        let req = CaptureRequest {
            from,
            queue: queue.clone(),
            body: params.body,
            source: params.source.unwrap_or_else(|| "mcp-capture".to_string()),
        };

        let channel_tree = self.resolve_channels_root(params.org.as_deref())?;
        let roots = CaptureRoots {
            flat_queues: &self.paths.queues,
            channel_tree: &channel_tree,
        };
        let path = capture_to_queue(req, roots, Utc::now())
            .map_err(|e| invalid_request(format!("capture failed: {e}")))?;

        info!(file = %path.display(), queue = %queue.as_str(), "captured to local queue via MCP");

        // Fire-and-forget contextification pass on inbox:triage captures.
        // No-op when no cognition adapter is configured (default state);
        // safe even if the file moves before the principal sees this
        // response because list_review_queue resolves paths on read,
        // never holds them in long-lived state.
        if queue.as_str() == secretariat_core::application::ROUTABLE_QUEUE {
            let capture_path = path.clone();
            let queues_root = self.paths.queues.clone();
            let ledger_path = self.paths.contextification_log.clone();
            let cognition_config = self.paths.cognition_config.clone();
            tokio::spawn(async move {
                match try_contextify_after_capture(
                    &capture_path,
                    &queues_root,
                    &ledger_path,
                    &cognition_config,
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
        Channels are addressed by colon-pathed handles like `channel:secretariat:dev` \
        or `channel:dommage-corporel:paris-cohort`. The substrate creates a channel \
        implicitly on the first capture into it — there is no explicit create step yet."
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
        newest-first. The `handle` must start with `channel:` (e.g. \
        `channel:secretariat:dev`). `limit` defaults to 10. \
        \
        Returns each envelope's body, sender, captured-at timestamp, and metadata \
        flags (stamped/encrypted). Use this to descend into one channel after \
        `list_channels` shows you what's available."
    )]
    async fn read_channel(
        &self,
        Parameters(params): Parameters<ReadChannelParams>,
    ) -> Result<Json<ReadChannelOutput>, ErrorData> {
        let handle = QueueHandle::parse(&params.handle).map_err(|e| {
            invalid_request(format!("invalid `handle` `{}`: {e}", params.handle))
        })?;
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

        let outcome = stamp_document(&path, &signer, StampAct::Attest, params.force, Utc::now())
            .map_err(|e| match e {
                StampError::AlreadyStamped => invalid_request(
                    "file already has a stamp; pass `force: true` to re-stamp".into(),
                ),
                StampError::Signer(SignerError::BiometricRefused) => {
                    invalid_request("biometric refused or cancelled".into())
                }
                other => invalid_request(format!("stamp failed: {other}")),
            })?;

        info!(file = %outcome.stamped_path.display(), "stamped envelope via MCP");

        Ok(Json(StampOutput {
            stamped_path: outcome.stamped_path.display().to_string(),
            signer: outcome.stamp.signer.as_str().to_string(),
            stamped_at: outcome.stamp.stamped_at.to_rfc3339(),
            doc_hash: outcome.stamp.doc_hash.to_string(),
        }))
    }

    // Note: `list_inbox` and `list_outbox` were tools in 0.2.7-0.2.10.
    // Moved to resources (`secretariat://inbox`, `secretariat://outbox`)
    // in 0.2.11 — listing IS reading, so resource semantics fit; the
    // model fetches them via `resources/read` rather than `tools/call`.

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
        description = "Archive an inbox envelope — move it out of the active \
        inbox into `inbox/archived/`. Use during a review session when the \
        principal says 'handled' / 'ignore' / 'done with this'. Files stay on \
        disk for history; just out of the active queue. Idempotent."
    )]
    async fn archive(
        &self,
        Parameters(params): Parameters<InboxActionParams>,
    ) -> Result<Json<InboxActionOutput>, ErrorData> {
        let path = PathBuf::from(&params.file_path);
        let moved = archive_envelope(&path, &self.paths.inbox)
            .map_err(|e| invalid_request(format!("archive failed: {e}")))?;
        info!(file = %path.display(), to = %moved.display(), "archived envelope via MCP");
        Ok(Json(InboxActionOutput {
            moved_to: moved.display().to_string(),
            note: "Envelope archived. Out of the active queue; kept on disk for history."
                .to_string(),
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
        description = "Verify the cryptographic stamp on an envelope file against the \
        signer's resolved DID. Returns one of: Verified, Tampered, Unsigned, \
        SignerUnresolvable, SignatureInvalid."
    )]
    async fn verify(
        &self,
        Parameters(params): Parameters<VerifyParams>,
    ) -> Result<Json<VerifyOutput>, ErrorData> {
        let resolver =
            CompositeDidResolver::new(DidWebResolver::new(self.paths.peers_cache.clone()));
        let path = PathBuf::from(&params.file_path);
        let outcome = verify_document(&path, &resolver)
            .map_err(|e| invalid_request(format!("verify failed: {e}")))?;
        Ok(Json(verify_outcome_to_view(outcome)))
    }

    // Note: `list_contacts` was a tool in 0.2.7-0.2.9. Moved to a resource
    // (`secretariat://contacts`) in 0.2.10 — contacts is a thing-to-read,
    // not an action-to-perform. Resource semantics fit; tool semantics
    // don't.

    // Note: `add_contact` was a tool in 0.2.7-0.2.10. Dropped in 0.2.11
    // because the normal contact-add path is invite-driven (`invite` /
    // `accept_invite` both auto-add the peer to the local contact book —
    // this is the bidirectional-contact-add per
    // memory/project_invite_is_correspondence). The remaining case —
    // someone hands the principal a DID out of band with no claim URL —
    // is rare enough that CLI handles it.

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
        the local DID with the relay if not already registered, and adds the \
        inviter to the local contact book (the bidirectional contact-add IS the \
        relationship). Returns inviter DID + acceptance metadata. Pair with \
        `invite` on the inviter side."
    )]
    async fn accept_invite(
        &self,
        Parameters(params): Parameters<AcceptInviteParams>,
    ) -> Result<Json<AcceptInviteOutput>, ErrorData> {
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

        // Auto-add inviter as a contact, plus persist the relay endpoint.
        let endpoint_origin = relay_origin_from_claim_url(&params.claim_url)?;
        let display = match params.name.as_deref() {
            Some(s) => DisplayName::parse(s)
                .map_err(|e| invalid_request(format!("invalid name: {e}")))?,
            None => default_display_for_did(&claimed.inviter_did)?,
        };
        let endpoint = RelayEndpoint::parse(&endpoint_origin)
            .map_err(|e| invalid_request(format!("derived relay endpoint invalid: {e}")))?;
        let contact = Contact::new(claimed.inviter_did.clone(), display, Some(endpoint));
        let contact_added = add_contact(&self.paths.contacts, contact).is_ok();

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
            contact_added,
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
        Use `create_channel` to populate the org with channels. Each channel addressed \
        as `channel:foo:bar` becomes a nested directory inside the org's `channels/`."
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
                Did::parse(s)
                    .map_err(|e| invalid_request(format!("invalid did `{s}`: {e}")))?,
            ),
        };
        let name = params
            .name
            .unwrap_or_else(|| alias.as_str().to_string());
        let description = params.description.unwrap_or_default();
        let org = app_create_org(
            &self.paths.orgs_root,
            alias,
            did,
            name,
            description,
            Utc::now(),
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
        name = "create_channel",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        ),
        description = "Create a channel inside an org (or in the principal's personal \
        tree if `org` is omitted). Writes a `.channelDef` metadata file and pre-creates \
        the `envelopes/` directory so the channel is visible in `list_channels` even \
        before any captures land. \
        \
        The `handle` must start with `channel:` and may have any number of colon-pathed \
        segments (`channel:product:data:baux-commerciaux`). Use `name` and `description` \
        to provide human-readable metadata."
    )]
    async fn create_channel(
        &self,
        Parameters(params): Parameters<CreateChannelParams>,
    ) -> Result<Json<ChannelDefDto>, ErrorData> {
        let handle = QueueHandle::parse(&params.handle).map_err(|e| {
            invalid_request(format!("invalid handle `{}`: {e}", params.handle))
        })?;
        let root = self.resolve_channels_root(params.org.as_deref())?;
        let name = params
            .name
            .unwrap_or_else(|| handle.slug().to_string());
        let description = params.description.unwrap_or_default();
        let def = app_create_channel(&root, handle, name, description, Utc::now())
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
        description = "Hard-delete a channel's entire directory tree — `.channelDef`, \
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
        let handle = QueueHandle::parse(&params.handle).map_err(|e| {
            invalid_request(format!("invalid handle `{}`: {e}", params.handle))
        })?;
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
}

// ---------------------------------------------------------------------------
// Resource URIs — kept here so the prompt bodies and resource handlers
// stay in sync. Anything that fetches `secretariat://template` (e.g.
// `prompts/compose.md`) must match the uri string the handler advertises.
// ---------------------------------------------------------------------------

const RESOURCE_TEMPLATE_URI: &str = "secretariat://template";
const RESOURCE_ATTENTION_ENVELOPE_URI: &str = "secretariat://attention-envelope";
const RESOURCE_CONTACTS_URI: &str = "secretariat://contacts";
const RESOURCE_INBOX_URI: &str = "secretariat://inbox";
const RESOURCE_OUTBOX_URI: &str = "secretariat://outbox";

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

        if self.paths.template.exists() {
            resources.push(build_resource(
                RESOURCE_TEMPLATE_URI,
                "Envelope template",
                "The principal's customized attentional-granularity envelope \
                 template at ~/.secretariat/template.md. Source of truth for \
                 envelope shape (headline, context, substance, subtleties, \
                 asks). Fetch this before drafting envelopes via /compose.",
            ));
        }

        if self.paths.attention_envelope.exists() {
            resources.push(build_resource(
                RESOURCE_ATTENTION_ENVELOPE_URI,
                "Attention envelope",
                "The principal's declared bounds (depths, urgencies, cadence) \
                 at ~/.secretariat/attention-envelope.md. Check before \
                 outbound envelopes to avoid violating the principal's \
                 stated cadence.",
            ));
        }

        // Contacts always available (even if empty) — useful for the model
        // to discover whether the principal already has the peer they
        // want to compose to in their book.
        resources.push(build_resource(
            RESOURCE_CONTACTS_URI,
            "Contacts",
            "The principal's contact book — peers known to the substrate, \
             with display names, DIDs, and (for did:key contacts) relay \
             endpoints. Fetch before composing to a peer to confirm the \
             slug or DID.",
        ));

        // Inbox / outbox are listings — fetched only when explicitly
        // reviewing. The /review prompt instructs the model to fetch
        // these on principal request, never proactively.
        resources.push(build_resource(
            RESOURCE_INBOX_URI,
            "Inbox",
            "Verified inbound envelopes the principal has received but not \
             yet acted on. Fetch ONLY when the principal explicitly asks to \
             review their inbox — Secretariat is for low-cadence intentional \
             review, not constant inbox-checking.",
        ));
        resources.push(build_resource(
            RESOURCE_OUTBOX_URI,
            "Outbox",
            "Drafts in the principal's outbox (stamped + sent and unstamped \
             pending review). Fetch when the principal asks 'what drafts do \
             I have?' or wants to review pending stamps.",
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
            RESOURCE_TEMPLATE_URI => std::fs::read_to_string(&self.paths.template)
                .map_err(|e| internal_error(format!("read template: {e}")))?,
            RESOURCE_ATTENTION_ENVELOPE_URI => {
                std::fs::read_to_string(&self.paths.attention_envelope)
                    .map_err(|e| internal_error(format!("read attention-envelope: {e}")))?
            }
            RESOURCE_CONTACTS_URI => render_contacts(&self.paths.contacts)?,
            RESOURCE_INBOX_URI => render_envelope_listing(
                "Inbox",
                list_inbox_files(&self.paths.inbox)
                    .map_err(|e| internal_error(format!("list_inbox: {e}")))?,
            ),
            RESOURCE_OUTBOX_URI => render_envelope_listing(
                "Outbox",
                list_outbox_files(&self.paths.outbox)
                    .map_err(|e| internal_error(format!("list_outbox: {e}")))?,
            ),
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
Secretariat is cryptographically attested AI-mediated correspondence. The \
principal is the human; you are the scribe. The principal stamps; you never \
do. Drafts live under `~/.secretariat/outbox/<recipient-did>/` and become \
sent envelopes only after the principal authorizes the stamp via Touch ID.

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
`secretariat://inbox` / `secretariat://outbox` resources proactively or \
between unrelated requests — only when the principal explicitly asks \
(\"check my inbox\", \"any drafts pending?\"). Captures (`capture`) stay \
local and CANNOT be stamped — use them for ideas/journal entries the \
principal will revisit at the next review session. Always `verify` \
inbound envelopes before trusting their content.";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn internal_error(msg: String) -> ErrorData {
    ErrorData::new(ErrorCode::INTERNAL_ERROR, msg, None)
}

fn render_envelope_listing(title: &str, envelopes: Vec<ListedEnvelope>) -> String {
    if envelopes.is_empty() {
        return format!("# {title}\n\n_Empty._\n");
    }
    let mut out = format!("# {title}\n\n");
    for e in envelopes {
        out.push_str(&format!("- `{}`", e.file_path));
        if let Some(from) = &e.from {
            out.push_str(&format!(" · from `{}`", from.as_str()));
        }
        if let Some(to) = &e.to {
            out.push_str(&format!(" · to `{}`", to.as_str()));
        }
        if let Some(handle) = &e.queue {
            out.push_str(&format!(" · queue `{}`", handle.as_str()));
        }
        if e.stamped {
            out.push_str(" · stamped ✓");
        }
        if e.encrypted {
            out.push_str(" · encrypted");
        }
        out.push('\n');
    }
    out
}

fn render_contacts(path: &std::path::Path) -> Result<String, ErrorData> {
    let contacts =
        list_contacts(path).map_err(|e| internal_error(format!("list_contacts: {e}")))?;
    if contacts.is_empty() {
        return Ok("# Contacts\n\nNo contacts yet. Use `invite` (you invite a peer) or \
                  `accept_invite` (a peer invited you) to establish your first \
                  correspondence relationship — both auto-add the peer.\n"
            .to_string());
    }
    let mut out = String::from("# Contacts\n\n");
    for c in contacts {
        out.push_str(&format!(
            "- **{}** — `{}`",
            c.display_name.as_str(),
            c.did.as_str()
        ));
        if let Some(relay) = &c.relay_endpoint {
            out.push_str(&format!(" · relay: `{}`", relay.as_str()));
        }
        out.push('\n');
    }
    Ok(out)
}

fn invalid_request(msg: String) -> ErrorData {
    ErrorData::new(ErrorCode::INVALID_REQUEST, msg, None)
}

fn resolve_to_did(paths: &KeyPaths, to: &str) -> Result<Did, ErrorData> {
    if to.starts_with("did:") {
        Did::parse(to).map_err(|e| invalid_request(format!("invalid did: {e}")))
    } else {
        let contact = find_by_slug(&paths.contacts, to)
            .map_err(|e| invalid_request(format!("contact lookup failed: {e}")))?
            .ok_or_else(|| invalid_request(format!("no contact matches `{to}`")))?;
        Ok(contact.did)
    }
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
    let did_file = paths.root.join("did");
    let raw = std::fs::read_to_string(&did_file).map_err(|e| {
        invalid_request(format!(
            "could not read principal DID at {}: {e}",
            did_file.display()
        ))
    })?;
    Did::parse(raw.trim()).map_err(|e| invalid_request(format!("malformed did file: {e}")))
}

fn first_registered_relay(path: &std::path::Path) -> Result<String, ErrorData> {
    let state = RelayState::load(path)
        .map_err(|e| invalid_request(format!("loading relay-state: {e}")))?;
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

fn default_display_for_did(did: &Did) -> Result<DisplayName, ErrorData> {
    let s = did.as_str();
    let name = if let Some(rest) = s.strip_prefix("did:web:") {
        rest.split(':').next().unwrap_or(rest).to_string()
    } else if let Some(rest) = s.strip_prefix("did:key:") {
        format!("did-key-{}", &rest.chars().take(8).collect::<String>())
    } else {
        s.to_string()
    };
    DisplayName::parse(name)
        .map_err(|e| invalid_request(format!("default display name invalid: {e}")))
}

fn verify_outcome_to_view(outcome: VerifyOutcome) -> VerifyOutput {
    match outcome {
        VerifyOutcome::Verified { signer, stamped_at, .. } => VerifyOutput {
            outcome: "Verified".into(),
            signer: Some(signer.as_str().to_string()),
            stamped_at: Some(stamped_at.to_rfc3339()),
        },
        VerifyOutcome::Tampered { .. } => VerifyOutput {
            outcome: "Tampered".into(),
            signer: None,
            stamped_at: None,
        },
        VerifyOutcome::Unsigned => VerifyOutput {
            outcome: "Unsigned".into(),
            signer: None,
            stamped_at: None,
        },
        VerifyOutcome::SignerUnresolvable { signer, .. } => VerifyOutput {
            outcome: "SignerUnresolvable".into(),
            signer: Some(signer.as_str().to_string()),
            stamped_at: None,
        },
        VerifyOutcome::SignatureInvalid { signer } => VerifyOutput {
            outcome: "SignatureInvalid".into(),
            signer: Some(signer.as_str().to_string()),
            stamped_at: None,
        },
    }
}
