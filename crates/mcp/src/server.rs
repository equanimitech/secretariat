//! Secretariat MCP server — stdio transport, 7 tools.
//!
//! Tools exposed:
//!
//! | Tool | Purpose |
//! |---|---|
//! | `compose` | Write an envelope to the outbox (principal stamps separately) |
//! | `list_outbox` | Pending drafts (stamped + unstamped) |
//! | `list_inbox` | Verified inbound envelopes |
//! | `read` | Decrypt + return body of an envelope |
//! | `verify` | Check a stamped artifact |
//! | `list_contacts` | Known peers |
//! | `add_contact` | Manual contact entry |
//!
//! Tools deliberately **not** exposed:
//!
//! - `stamp` — principal-only via the menubar (or `sec stamp` CLI). Rule 4
//!   in `AGENTS.md`: only the principal stamps. If Claude could stamp, the
//!   primitive collapses to forgery.
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
    add_contact, compose_envelope, find_by_slug, list_contacts, list_inbox_files,
    list_outbox_files, read_envelope, verify_document, ComposeRequest, ListedEnvelope,
    VerifyOutcome,
};
use secretariat_core::domain::DidMethod;
use secretariat_core::infrastructure::composite_did_resolver::CompositeDidResolver;
use secretariat_core::infrastructure::did_web_resolver::DidWebResolver;
use secretariat_core::infrastructure::keys::KeyPaths;
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

fn verify_outcome_to_view(outcome: VerifyOutcome) -> VerifyOutput {
    match outcome {
        VerifyOutcome::Verified { signer, stamped_at } => VerifyOutput {
            outcome: "Verified".into(),
            signer: Some(signer.as_str().to_string()),
            stamped_at: Some(stamped_at.to_rfc3339()),
        },
        VerifyOutcome::Tampered { signer, stamped_at } => VerifyOutput {
            outcome: "Tampered".into(),
            signer: Some(signer.as_str().to_string()),
            stamped_at: Some(stamped_at.to_rfc3339()),
        },
        VerifyOutcome::Unsigned => VerifyOutput {
            outcome: "Unsigned".into(),
            signer: None,
            stamped_at: None,
        },
        VerifyOutcome::SignerUnresolvable { signer } => VerifyOutput {
            outcome: "SignerUnresolvable".into(),
            signer: Some(signer.as_str().to_string()),
            stamped_at: None,
        },
        VerifyOutcome::SignatureInvalid { signer, stamped_at } => VerifyOutput {
            outcome: "SignatureInvalid".into(),
            signer: Some(signer.as_str().to_string()),
            stamped_at: Some(stamped_at.to_rfc3339()),
        },
    }
}
