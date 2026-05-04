//! Secretariat MCP server — stdio transport, 8 tools.
//!
//! Tools exposed:
//!
//! | Tool | Purpose |
//! |---|---|
//! | `compose` | Write an envelope to the outbox (principal stamps separately) |
//! | `stamp` | Trigger biometric stamp on a draft (Touch ID gates regardless of caller) |
//! | `list_outbox` | Pending drafts (stamped + unstamped) |
//! | `list_inbox` | Verified inbound envelopes |
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
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{ErrorCode, ErrorData},
    tool, tool_handler, tool_router,
    ServerHandler,
};
use schemars::JsonSchema;
use secretariat_core::application::{
    add_contact, claim_invite, compose_envelope, create_invite, find_by_slug, list_contacts,
    list_inbox_files, list_outbox_files, read_envelope, stamp_document, verify_document,
    view_invite, ComposeRequest, ListedEnvelope, StampError, VerifyOutcome,
};
use secretariat_core::domain::{DidMethod, StampAct};
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
}

impl SecretariatServer {
    pub fn new(paths: KeyPaths) -> Self {
        Self {
            paths,
            tool_router: Self::tool_router(),
        }
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
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ComposeOutput {
    pub file_path: String,
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
    pub to: Option<String>,
    pub stamped: bool,
    pub encrypted: bool,
}

impl From<ListedEnvelope> for EnvelopeView {
    fn from(l: ListedEnvelope) -> Self {
        Self {
            file_path: l.file_path,
            from: l.from,
            to: l.to,
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

        let req = ComposeRequest {
            from,
            to: Some(to),
            depth,
            urgency,
            source: params.source.unwrap_or_else(|| "mcp".to_string()),
            cadence_hint: params.cadence_hint,
        };

        let path = compose_envelope(req, &self.paths.template, &self.paths.outbox, Utc::now())
            .map_err(|e| invalid_request(format!("compose failed: {e}")))?;

        // Body is currently sourced from the AG template. v0 doesn't yet wire
        // the user-supplied body into the file — it goes through the template
        // by design. Note this in the response so the principal knows.
        let _ = params.body;

        info!(file = %path.display(), "composed envelope via MCP");

        Ok(Json(ComposeOutput {
            file_path: path.display().to_string(),
            note: "Draft written to outbox via the AG template. Edit the file to insert the \
                   body, then stamp it manually (biometric-gated). The daemon will deliver \
                   after stamping."
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
impl ServerHandler for SecretariatServer {}

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
