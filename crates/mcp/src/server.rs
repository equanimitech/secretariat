//! Secretariat MCP server — stdio transport.
//!
//! Tools exposed:
//!
//! | Tool | Purpose |
//! |---|---|
//! | `compose` | Write a peer-addressed envelope to the outbox (principal stamps separately) |
//! | `capture` | Drop a body into a local queue (substrate v0.3 — never sent, never stamped without consent) |
//! | `stamp` | Trigger biometric stamp on a draft (Touch ID gates regardless of caller) |
//! | `list_outbox` | Pending drafts (stamped + unstamped) |
//! | `list_inbox` | Verified inbound envelopes |
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
        ErrorCode, ErrorData, GetPromptRequestParam, GetPromptResult, Implementation,
        ListPromptsResult, PaginatedRequestParam, PromptMessage, PromptMessageRole,
        ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    prompt, prompt_handler, prompt_router,
    service::RequestContext,
    tool, tool_handler, tool_router,
    RoleServer, ServerHandler,
};
use schemars::JsonSchema;
use secretariat_core::application::{
    add_contact, archive_envelope, capture_to_queue, claim_invite, compose_envelope,
    create_invite, defer_envelope, find_by_slug, list_contacts, list_inbox_files,
    list_outbox_files, read_envelope, stamp_document, verify_document, view_invite,
    CaptureRequest, ComposeRequest, ListedEnvelope, StampError, VerifyOutcome,
};
use secretariat_core::domain::{DidMethod, QueueHandle, Recipient, StampAct};
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
}

// ---------------------------------------------------------------------------
// Prompts — substrate vocabulary (`/idea`, `/pain`, `/shaping`, `/share`,
// `/roundtable`). Each prompt is a static markdown body shipped alongside
// the binary. They surface as slash commands in MCP-aware clients (Claude
// Code, Claude Desktop) and route, by convention, to the `capture` /
// `compose` / file-write tools as appropriate. See pitch
// `docs/pitches/2026-05-05-mcp-prompts-as-substrate-vocabulary.md`.
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

    // Note: `/share`, `/shaping`, `/roundtable` are intentionally NOT
    // shipped here. They are Rafa-personal product-management /
    // sharing vocabulary, not Secretariat correspondence vocabulary.
    // Secretariat-native prompts (`/review`, `/compose`, `/onboard`,
    // `/stamp`) are tracked as a follow-up pitch — they wrap the
    // server's correspondence tools rather than orthogonal workflows.
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
    /// Target queue handle, of the form `<namespace>:<slug>` — e.g.
    /// `inbox:triage`, `area:health`, `project:autonomous-enterprise`.
    /// Namespaces are free-form lowercase letters.
    pub queue: String,
    /// Body of the capture (markdown-friendly plain text).
    pub body: String,
    /// Free-form origin marker, e.g. `idea-skill`, `quick-pane`. Defaults
    /// to `mcp-capture` when omitted.
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CaptureOutput {
    pub file_path: String,
    pub queue: String,
    pub note: String,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct EmptyParams {}

#[derive(Debug, Serialize, JsonSchema)]
pub struct EnvelopeListing {
    pub envelopes: Vec<EnvelopeView>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct EnvelopeView {
    pub file_path: String,
    pub from: Option<String>,
    /// DID of the queue *owner* (recipient). Always set on well-formed
    /// envelopes. Compare to the principal's own DID to discriminate
    /// local capture (`to == self`) from peer/channel post (`to != self`).
    pub to: Option<String>,
    /// Queue handle on the owner's machine (`<namespace>:<slug>`).
    /// Always set on well-formed envelopes alongside `to`. Direct
    /// messages conventionally use `inbox:default`.
    pub queue: Option<String>,
    pub stamped: bool,
    pub encrypted: bool,
}

impl From<ListedEnvelope> for EnvelopeView {
    fn from(l: ListedEnvelope) -> Self {
        Self {
            file_path: l.file_path,
            from: l.from,
            to: l.to,
            queue: l.queue,
            stamped: l.stamped,
            encrypted: l.encrypted,
        }
    }
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

#[derive(Debug, Serialize, JsonSchema)]
pub struct ContactListing {
    pub contacts: Vec<ContactView>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ContactView {
    pub did: String,
    pub display_name: String,
    pub relay_endpoint: Option<String>,
}

impl From<Contact> for ContactView {
    fn from(c: Contact) -> Self {
        Self {
            did: c.did.as_str().to_string(),
            display_name: c.display_name.as_str().to_string(),
            relay_endpoint: c.relay_endpoint.as_ref().map(|r| r.as_str().to_string()),
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddContactParams {
    pub did: String,
    pub display_name: String,
    /// Required for `did:key` peers (no live discovery channel).
    /// Omit for `did:web` peers — daemon resolves from their DID document.
    #[serde(default)]
    pub relay_endpoint: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AddContactOutput {
    pub did: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InviteCreateParams {
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
pub struct InviteCreateOutput {
    pub token: String,
    pub claim_url: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InviteClaimParams {
    /// Claim URL the inviter shared
    /// (e.g. `https://secretariat.equanimi.tech/v0/invite/<token>`).
    pub claim_url: String,
    /// Display name to give the inviter in the local contact book.
    /// Defaults to the host portion of their DID.
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InviteClaimOutput {
    pub inviter_did: String,
    pub claimant_did: String,
    pub claimed_at: String,
    /// Whether the relay registered the claimant's DID during this call.
    pub registered: bool,
    /// Whether the inviter was added to the local contact book.
    pub contact_added: bool,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct InitParams {
    /// Optional did:web override; e.g. `did:web:rafa.equanimi.tech`. Omit
    /// to derive a `did:key` from the freshly-generated public key (zero
    /// hosting needed).
    #[serde(default)]
    pub did: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InitOutput {
    pub did: String,
    pub root: String,
    pub message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DaemonInstallOutput {
    pub plist_path: String,
    pub message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DaemonStatusOutput {
    pub installed: bool,
    pub loaded: bool,
    pub raw_output: String,
}

// ---------------------------------------------------------------------------
// Tool router
// ---------------------------------------------------------------------------

#[tool_router]
impl SecretariatServer {
    #[tool(
        name = "compose",
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
        description = "Drop a body of text into a local queue (substrate v0.3). \
        Captures are envelopes addressed to `Recipient::LocalQueue(handle)` — they \
        never leave the principal's machine and CANNOT be stamped (the domain \
        invariant rejects it). Use for ideas, journal entries, future-self notes, \
        anything to surface again at the next review session. \
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

        let path = capture_to_queue(req, &self.paths.queues, Utc::now())
            .map_err(|e| invalid_request(format!("capture failed: {e}")))?;

        info!(file = %path.display(), queue = %queue.as_str(), "captured to local queue via MCP");

        Ok(Json(CaptureOutput {
            file_path: path.display().to_string(),
            queue: queue.as_str().to_string(),
            note: "Capture written to local queue. It stays on this device and \
                   surfaces again at the next review session — never sent, never stamped."
                .to_string(),
        }))
    }

    #[tool(
        name = "stamp",
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

    #[tool(
        name = "list_outbox",
        description = "List drafts in the principal's outbox (stamped and unstamped)."
    )]
    async fn list_outbox(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<Json<EnvelopeListing>, ErrorData> {
        let envelopes = list_outbox_files(&self.paths.outbox)
            .map_err(|e| invalid_request(format!("list_outbox failed: {e}")))?;
        Ok(Json(EnvelopeListing {
            envelopes: envelopes.into_iter().map(EnvelopeView::from).collect(),
        }))
    }

    #[tool(
        name = "list_inbox",
        description = "List verified inbound envelopes from the principal's inbox. \
        IMPORTANT: only call this tool when the user has explicitly asked for their inbox \
        (e.g., 'check my inbox', 'any new messages from Marcelo?'). Do not call proactively \
        at the start of conversations or between unrelated requests — Secretariat is designed \
        for low-cadence, intentional review, not constant inbox-checking."
    )]
    async fn list_inbox(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<Json<EnvelopeListing>, ErrorData> {
        let envelopes = list_inbox_files(&self.paths.inbox)
            .map_err(|e| invalid_request(format!("list_inbox failed: {e}")))?;
        Ok(Json(EnvelopeListing {
            envelopes: envelopes.into_iter().map(EnvelopeView::from).collect(),
        }))
    }

    #[tool(
        name = "defer",
        description = "Defer an inbox envelope — move it out of the active inbox into \
        `inbox/deferred/`. Use during a review session when the principal says 'remind me \
        later' / 'come back to this' / 'not now'. Idempotent against the destination. \
        Future bubble-up logic will re-surface deferred envelopes; v1 just stages them."
    )]
    async fn defer(
        &self,
        Parameters(params): Parameters<InboxActionParams>,
    ) -> Result<Json<InboxActionOutput>, ErrorData> {
        let path = PathBuf::from(&params.file_path);
        let moved = defer_envelope(&path, &self.paths.inbox)
            .map_err(|e| invalid_request(format!("defer failed: {e}")))?;
        info!(file = %path.display(), to = %moved.display(), "deferred envelope via MCP");
        Ok(Json(InboxActionOutput {
            moved_to: moved.display().to_string(),
            note: "Envelope deferred. It is no longer in the active inbox; the principal \
                   can re-surface it later."
                .to_string(),
        }))
    }

    #[tool(
        name = "archive",
        description = "Archive an inbox envelope — move it out of the active inbox into \
        `inbox/archived/`. Use during a review session when the principal says 'handled' / \
        'ignore' / 'done with this'. Files stay on disk for history; just out of the queue."
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

    #[tool(
        name = "list_contacts",
        description = "List all known peers in the principal's contact book."
    )]
    async fn list_contacts(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<Json<ContactListing>, ErrorData> {
        let contacts = list_contacts(&self.paths.contacts)
            .map_err(|e| invalid_request(format!("list_contacts failed: {e}")))?;
        Ok(Json(ContactListing {
            contacts: contacts.into_iter().map(ContactView::from).collect(),
        }))
    }

    #[tool(
        name = "add_contact",
        description = "Add a peer to the principal's contact book. \
        For `did:key` peers, `relay_endpoint` is required (no live discovery channel). \
        For `did:web` peers, omit `relay_endpoint` — daemon resolves from their DID document."
    )]
    async fn add_contact(
        &self,
        Parameters(params): Parameters<AddContactParams>,
    ) -> Result<Json<AddContactOutput>, ErrorData> {
        let did = Did::parse(&params.did)
            .map_err(|e| invalid_request(format!("invalid did: {e}")))?;
        let name = DisplayName::parse(&params.display_name)
            .map_err(|e| invalid_request(format!("invalid display_name: {e}")))?;
        let relay = match params.relay_endpoint.as_deref() {
            Some(s) => Some(
                RelayEndpoint::parse(s)
                    .map_err(|e| invalid_request(format!("invalid relay_endpoint: {e}")))?,
            ),
            None => None,
        };
        if did.method() == DidMethod::Key && relay.is_none() {
            return Err(invalid_request(
                "did:key contacts require a relay_endpoint (no live discovery channel)"
                    .to_string(),
            ));
        }
        let display_str = name.as_str().to_string();
        let contact = Contact::new(did.clone(), name, relay);
        add_contact(&self.paths.contacts, contact)
            .map_err(|e| invalid_request(format!("add_contact failed: {e}")))?;
        Ok(Json(AddContactOutput {
            did: did.as_str().to_string(),
            display_name: display_str,
        }))
    }

    #[tool(
        name = "invite_create",
        description = "Create a one-shot invite token at the relay. The principal \
        must already be a registered tenant of `endpoint` (or the first registered \
        relay in relay-state.json). Returns a claim URL the principal can share \
        with a peer. Default TTL is 168 hours (7 days). Pair with `invite_claim` \
        on the recipient side."
    )]
    async fn invite_create(
        &self,
        Parameters(params): Parameters<InviteCreateParams>,
    ) -> Result<Json<InviteCreateOutput>, ErrorData> {
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

        Ok(Json(InviteCreateOutput {
            token: invite.token,
            claim_url: invite.claim_url,
            expires_at: invite.expires_at.to_rfc3339(),
        }))
    }

    #[tool(
        name = "init",
        description = "Generate the principal's ed25519 keypair + DID, seed \
        ~/.secretariat/template.md and attention-envelope.md, and write the \
        principal's `did` file. Idempotent-safe: refuses if a key already \
        exists (won't overwrite). Pair with `invite_claim` to onboard against \
        a relay in one round trip. Default DID method is `did:key` (zero \
        hosting); pass `did` to opt into `did:web` if the principal owns a \
        domain to host the DID document at."
    )]
    async fn init(
        &self,
        Parameters(params): Parameters<InitParams>,
    ) -> Result<Json<InitOutput>, ErrorData> {
        let mut cmd = std::process::Command::new("sec");
        cmd.arg("init");
        if let Some(did) = params.did.as_deref() {
            cmd.arg("--did").arg(did);
        }
        let output = cmd
            .output()
            .map_err(|e| invalid_request(format!("invoking `sec init`: {e}")))?;
        if !output.status.success() {
            return Err(invalid_request(format!(
                "sec init failed (exit {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let did = stderr
            .lines()
            .find(|l| l.contains("did") && l.contains("did:"))
            .and_then(|l| l.split_whitespace().last())
            .map(|s| s.to_string())
            .unwrap_or_default();

        info!(did = %did, "sec init via MCP");

        Ok(Json(InitOutput {
            did,
            root: self.paths.root.display().to_string(),
            message: stderr,
        }))
    }

    #[tool(
        name = "daemon_install",
        description = "Install the daemon as a macOS LaunchAgent so it runs \
        in the background, survives reboot, and auto-restarts on crash. \
        Idempotent — re-running after upgrades replaces the plist with the \
        current binary path. macOS only."
    )]
    async fn daemon_install(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<Json<DaemonInstallOutput>, ErrorData> {
        let output = std::process::Command::new("sec")
            .args(["daemon", "install"])
            .output()
            .map_err(|e| invalid_request(format!("invoking `sec daemon install`: {e}")))?;
        if !output.status.success() {
            return Err(invalid_request(format!(
                "sec daemon install failed (exit {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let plist_path = home_relative_plist();

        info!("daemon LaunchAgent installed via MCP");

        Ok(Json(DaemonInstallOutput {
            plist_path,
            message: stderr,
        }))
    }

    #[tool(
        name = "daemon_status",
        description = "Report whether the daemon LaunchAgent is installed + \
        loaded + (when launchctl reports it) the PID/exit-status. Useful for \
        verifying the background process is running after `daemon_install`."
    )]
    async fn daemon_status(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<Json<DaemonStatusOutput>, ErrorData> {
        let output = std::process::Command::new("sec")
            .args(["daemon", "status"])
            .output()
            .map_err(|e| invalid_request(format!("invoking `sec daemon status`: {e}")))?;
        let raw = String::from_utf8_lossy(&output.stdout).into_owned();
        let installed = raw.contains("plist installed:      true");
        let loaded = raw.contains("loaded (launchctl):   true");

        Ok(Json(DaemonStatusOutput {
            installed,
            loaded,
            raw_output: raw,
        }))
    }

    #[tool(
        name = "invite_claim",
        description = "Claim an invite issued by another principal. Auto-registers \
        the local DID with the relay if not already registered, and adds the \
        inviter to the local contact book. Returns inviter DID + claim metadata. \
        Pair with `invite_create` on the inviter side."
    )]
    async fn invite_claim(
        &self,
        Parameters(params): Parameters<InviteClaimParams>,
    ) -> Result<Json<InviteClaimOutput>, ErrorData> {
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

        Ok(Json(InviteClaimOutput {
            inviter_did: claimed.inviter_did.as_str().to_string(),
            claimant_did: claimed.claimant_did.as_str().to_string(),
            claimed_at: claimed.claimed_at.to_rfc3339(),
            registered: claimed.registered,
            contact_added,
        }))
    }
}

#[tool_handler]
#[prompt_handler]
impl ServerHandler for SecretariatServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
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

Cadence: Secretariat is for low-cadence, intentional review. Do not call \
`list_inbox` / `list_outbox` proactively or between unrelated requests — \
only when the principal explicitly asks (\"check my inbox\", \"any drafts \
pending?\"). Captures (`capture`) stay local and CANNOT be stamped — use \
them for ideas/journal entries the principal will revisit at the next \
review session. Always `verify` inbound envelopes before trusting their \
content.";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn home_relative_plist() -> String {
    if let Some(home) = dirs::home_dir() {
        home.join("Library/LaunchAgents/tech.equanimi.secretariat.daemon.plist")
            .display()
            .to_string()
    } else {
        "~/Library/LaunchAgents/tech.equanimi.secretariat.daemon.plist".to_string()
    }
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
