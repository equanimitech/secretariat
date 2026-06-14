//! Secretariat MCP server — stdio transport.
//!
//! Tools exposed (post git-native teardown, cut B):
//!
//! | Tool | Purpose |
//! |---|---|
//! | `compose` | Write a doc into a registered repo: placed by convention, scribe-signed, committed |
//! | `stamp` | Trigger biometric stamp on a draft (Touch ID gates regardless of caller) |
//! | `read` | Decrypt + return body of an envelope |
//! | `verify` | Check a signed/stamped artifact (three-state layered verifier) |
//! | `agent_add` / `agent_list` / `agent_remove` / `agent_rotate` | Manage authorized agents |
//! | `repo_add` / `repo_list` / `repo_remove` | Manage the substrate manifest (`[[repos]]`) |
//!
//! On `stamp`: the call only *initiates* the ceremony; the platform
//! biometric gate (Touch ID via the Swift helper) blocks until the
//! principal physically authorizes. Claude cannot bypass that.
//!
//! The channels / orgs / contracts / compose / capture / invite tools and
//! the `secretariat://orgs` + `secretariat://compositions` resources were
//! removed in the git-native teardown (cut B) — that correspondence column
//! moved off the bespoke substrate onto git-native review.

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
    tool, tool_handler, tool_router, RoleServer, ServerHandler,
};
use schemars::JsonSchema;
use secretariat_core::application::compose_ops::ComposeRequest;
use secretariat_core::application::repo_ops::list_repos;
use secretariat_core::application::{
    add_agent as app_add_agent, compose_document, list_agents as app_list_agents, read_envelope,
    remove_agent as app_remove_agent, resolve_sole_scribe, rotate_agent as app_rotate_agent,
    stamp_document, verify_document_layered, DocType, LayeredVerifyOutcome, StampError,
    VerifyOutcome,
};
use secretariat_core::domain::StampAct;
use secretariat_core::infrastructure::biometric::build_signer;
use secretariat_core::infrastructure::composite_did_resolver::CompositeDidResolver;
use secretariat_core::infrastructure::did_web_resolver::DidWebResolver;
use secretariat_core::infrastructure::keys::{load_signing_key, KeyPaths};
use secretariat_core::infrastructure::open_in_secretariat;
use secretariat_core::ports::SignerError;
use secretariat_core::Did;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

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
// Prompts — the stamp ceremony surface. The capture / compose / onboard /
// review rituals were dropped with their backing tools in the git-native
// teardown (cut B).
// ---------------------------------------------------------------------------

#[prompt_router]
impl SecretariatServer {
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
    /// Absolute path to the envelope to stamp. Stamping embeds the
    /// `$attestation` block in place; no rename.
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
pub struct ComposeParams {
    /// Absolute path to the target repo. Must be enrolled in the substrate
    /// manifest (`repo_add`).
    pub repo: String,
    /// Doc type: `idea` | `pain` | `decision` | `pitch` | `note`. Drives the
    /// bucket directory (`docs/ideas/`, `docs/pain/`, …) and the frontmatter
    /// `type:` facet.
    pub doc_type: String,
    /// Title — drives the `<date>-<slug>.md` filename and the commit message.
    pub title: String,
    /// Full markdown body. May carry leading editorial frontmatter (lifted
    /// into the canonical block); the cryptographic keys (`$envelope` /
    /// `$signature` / `$attestation`) are reserved and rejected.
    pub body: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ComposeOutput {
    pub path: String,
    pub signer: String,
    pub committed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_skipped: Option<String>,
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

// ---------------------------------------------------------------------------
// Repo-registry parameter / output schemas
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RepoDto {
    /// Absolute path to the repo.
    pub path: String,
    /// `project` or `home`.
    pub role: String,
    /// Free-form grouping labels (e.g. `themia`, `equanimitech`).
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepoAddParams {
    /// Path to the repo (must be a git repo). Absolute preferred.
    pub path: String,
    /// `project` (default) or `home`.
    #[serde(default)]
    pub role: Option<String>,
    /// Free-form grouping tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepoListParams {
    /// Only repos carrying this tag.
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepoRemoveParams {
    /// Path to the repo to unenroll.
    pub path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RepoListOutput {
    pub repos: Vec<RepoDto>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RepoRemoveOutput {
    pub removed: bool,
}

// ---------------------------------------------------------------------------
// Timeline parameter / output schemas
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TimelineParams {
    /// Date window. One of: `today`, `Nd` (last N days, e.g. `7d` / `30d`),
    /// `YYYY-MM` (whole month), `YYYY-MM-DD`, or `YYYY-MM-DD..YYYY-MM-DD`.
    /// Default `7d`.
    #[serde(default)]
    pub range: Option<String>,
    /// Grouping granularity: `day` | `week` | `month`. At `month` only the
    /// per-day histogram is returned (no per-doc entries) to stay compact.
    /// Default `day`.
    #[serde(default)]
    pub zoom: Option<String>,
    /// Restrict to repos carrying this tag (e.g. `equanimitech`, `themia`).
    #[serde(default)]
    pub tag: Option<String>,
    /// Restrict to a doc state: `stamped` | `signed` | `raw`.
    #[serde(default)]
    pub state: Option<String>,
    /// Restrict to a doc bucket (top-level dir under `docs/`, e.g. `decisions`).
    #[serde(default)]
    pub bucket: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TimelineDayDto {
    pub date: String,
    pub stamped: usize,
    pub signed: usize,
    pub raw: usize,
    pub total: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TimelineEntryDto {
    pub date: String,
    /// `stamped` | `signed` | `raw`.
    pub state: String,
    /// Top-level dir under `docs/`, or null if the doc sits directly in `docs/`.
    pub bucket: Option<String>,
    pub slug: String,
    /// First markdown heading in the body, if any.
    pub title: Option<String>,
    pub repo_tags: Vec<String>,
    pub rel_path: String,
    /// Absolute path — hand directly to `read` to open the doc.
    pub abs_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TimelineOutput {
    pub from: String,
    pub to: String,
    pub zoom: String,
    pub total: usize,
    pub by_day: Vec<TimelineDayDto>,
    /// Per-doc entries. Empty when `zoom = month`.
    pub entries: Vec<TimelineEntryDto>,
}

// ---------------------------------------------------------------------------
// Tool router
// ---------------------------------------------------------------------------

#[tool_router]
impl SecretariatServer {
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

        info!(file = %outcome.stamped_path.display(), "stamped envelope via MCP");

        Ok(Json(StampOutput {
            stamped_path: outcome.stamped_path.display().to_string(),
            signer: outcome.stamp.signer.as_str().to_string(),
            stamped_at: outcome.stamp.stamped_at.to_rfc3339(),
            doc_hash: outcome.stamp.doc_hash.to_string(),
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
        name = "compose",
        annotations(
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        ),
        description = "Compose a markdown doc into a registered repo through the substrate \
        — the canonical way to write any doc (idea, pain, decision, pitch, note). Places it \
        by convention (`docs/<bucket>/<date>-<slug>.md`), signs the body at birth with the \
        scribe's `$signature` (the author-signature tier of the trust model — NOT a stamp), \
        and commits exactly that one path (message `docs(<type>): <title>`); co-mingled \
        working-tree state is never staged. Never overwrites: an existing target path is an \
        error. Render the body to the principal before composing. Stamping remains a \
        separate, principal-only act."
    )]
    async fn compose(
        &self,
        Parameters(params): Parameters<ComposeParams>,
    ) -> Result<Json<ComposeOutput>, ErrorData> {
        let doc_type = DocType::parse(&params.doc_type)
            .map_err(|e| invalid_request(format!("invalid doc_type: {e}")))?;
        let (scribe_did, scribe_key) = resolve_sole_scribe(&self.paths)
            .map_err(|e| invalid_request(format!("resolving scribe: {e}")))?;
        let registry = list_repos(&self.paths.preferences, None)
            .map_err(|e| invalid_request(format!("loading repo registry: {e}")))?;
        let outcome = compose_document(ComposeRequest {
            registry: &registry,
            repo_path: &PathBuf::from(&params.repo),
            doc_type,
            title: &params.title,
            body: &params.body,
            signer: scribe_did.clone(),
            signing_key: &scribe_key,
            now: Utc::now(),
        })
        .map_err(|e| invalid_request(format!("compose failed: {e}")))?;
        info!(path = %outcome.path.display(), committed = outcome.committed, "doc composed via MCP");
        // Surface the fresh doc in the desktop app. Best-effort: a missing
        // GUI session must not fail the compose call.
        if let Err(e) = open_in_secretariat(&outcome.path) {
            warn!(error = %e, "composed but could not open in the Secretariat app");
        }
        Ok(Json(ComposeOutput {
            path: outcome.path.display().to_string(),
            signer: scribe_did.as_str().to_string(),
            committed: outcome.committed,
            commit_skipped: outcome.commit_skipped,
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
        principal's scribe."
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
        no agents have been provisioned."
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
        audit trail. The principal's identity record is re-signed to remove the entry."
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
        Use when key compromise is suspected, or as part of routine key hygiene."
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
        name = "repo_add",
        annotations(
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Enroll (or update) a git repo in the substrate manifest. `path` is \
        the identity — calling again with the same path updates its role/tags (upsert, no \
        duplicate). `role` is `project` (default) or `home` (private cross-cutting PKM). \
        `tags` are free-form grouping labels (e.g. themia, equanimitech). Fails if `path` \
        is not a git repo."
    )]
    async fn repo_add(
        &self,
        Parameters(params): Parameters<RepoAddParams>,
    ) -> Result<Json<RepoDto>, ErrorData> {
        use secretariat_core::application::repo_ops::register_repo;
        use secretariat_core::infrastructure::RepoRole;
        let role = RepoRole::parse(params.role.as_deref().unwrap_or("project"))
            .map_err(|e| invalid_request(format!("invalid role: {e}")))?;
        let entry = register_repo(
            &self.paths.preferences,
            std::path::Path::new(&params.path),
            role,
            params.tags,
        )
        .map_err(|e| invalid_request(format!("repo_add failed: {e}")))?;
        info!(path = %entry.path.display(), "repo enrolled via MCP");
        Ok(Json(repo_to_dto(entry)))
    }

    #[tool(
        name = "repo_list",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false),
        description = "List repos enrolled in the substrate manifest, optionally filtered \
        to those carrying a given tag. Each entry: absolute path, role (project|home), tags."
    )]
    async fn repo_list(
        &self,
        Parameters(params): Parameters<RepoListParams>,
    ) -> Result<Json<RepoListOutput>, ErrorData> {
        use secretariat_core::application::repo_ops::list_repos;
        let repos = list_repos(&self.paths.preferences, params.tag.as_deref())
            .map_err(|e| invalid_request(format!("repo_list failed: {e}")))?;
        Ok(Json(RepoListOutput {
            repos: repos.into_iter().map(repo_to_dto).collect(),
        }))
    }

    #[tool(
        name = "repo_remove",
        annotations(
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        ),
        description = "Unenroll a repo from the substrate manifest by path. Returns \
        `removed: false` if the path was not enrolled. Does not touch the repo's files — \
        only the manifest entry."
    )]
    async fn repo_remove(
        &self,
        Parameters(params): Parameters<RepoRemoveParams>,
    ) -> Result<Json<RepoRemoveOutput>, ErrorData> {
        use secretariat_core::application::repo_ops::unregister_repo;
        let removed = unregister_repo(&self.paths.preferences, std::path::Path::new(&params.path))
            .map_err(|e| invalid_request(format!("repo_remove failed: {e}")))?;
        Ok(Json(RepoRemoveOutput { removed }))
    }

    #[tool(
        name = "timeline",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false),
        description = "Chronological view of docs across all registered repos, grouped by \
        date and badged by state (stamped / signed / raw). Answers 'what did I create \
        today / over the last days / last month'. \
        \
        `range`: today | Nd (e.g. 7d, 30d) | YYYY-MM | YYYY-MM-DD | YYYY-MM-DD..YYYY-MM-DD \
        (default 7d). `zoom`: day | week | month — at `month`, only the per-day histogram \
        is returned (no per-doc entries) to stay compact. Optional filters: `tag` (repo \
        group, e.g. equanimitech), `state` (stamped|signed|raw), `bucket` (e.g. decisions). \
        \
        Read-only and never decrypts — state is derived from frontmatter, dates from the \
        `<date>-<slug>.md` filename. Hand an entry's `abs_path` to `read` to open it. \
        Distinguish signed-only (informational) from stamped (authoritative)."
    )]
    async fn timeline(
        &self,
        Parameters(params): Parameters<TimelineParams>,
    ) -> Result<Json<TimelineOutput>, ErrorData> {
        use secretariat_core::application::timeline_ops::{
            build_timeline, DocState, TimelineFilter,
        };

        let range = params.range.as_deref().unwrap_or("7d");
        let zoom = params.zoom.as_deref().unwrap_or("day").to_lowercase();
        if !matches!(zoom.as_str(), "day" | "week" | "month") {
            return Err(invalid_request(format!(
                "invalid zoom `{zoom}` (expected day|week|month)"
            )));
        }
        let state = match params.state.as_deref() {
            None => None,
            Some(s) => Some(DocState::parse(s).ok_or_else(|| {
                invalid_request(format!("invalid state `{s}` (expected stamped|signed|raw)"))
            })?),
        };
        let filter = TimelineFilter {
            tag: params.tag.clone(),
            state,
            bucket: params.bucket.clone(),
        };
        let today = Utc::now().date_naive();
        let tl = build_timeline(&self.paths.preferences, today, range, &filter)
            .map_err(|e| invalid_request(format!("timeline failed: {e}")))?;

        let entries = if zoom == "month" {
            Vec::new()
        } else {
            tl.entries
                .iter()
                .map(|e| TimelineEntryDto {
                    date: e.date.to_string(),
                    state: e.state.as_str().to_string(),
                    bucket: e.bucket.clone(),
                    slug: e.slug.clone(),
                    title: e.title.clone(),
                    repo_tags: e.repo_tags.clone(),
                    rel_path: e.rel_path.display().to_string(),
                    abs_path: e.abs_path.display().to_string(),
                })
                .collect()
        };

        Ok(Json(TimelineOutput {
            from: tl.from.to_string(),
            to: tl.to.to_string(),
            zoom,
            total: tl.entries.len(),
            by_day: tl
                .by_day
                .iter()
                .map(|d| TimelineDayDto {
                    date: d.date.to_string(),
                    stamped: d.stamped,
                    signed: d.signed,
                    raw: d.raw,
                    total: d.total(),
                })
                .collect(),
            entries,
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
/// once at session start.
const SERVER_INSTRUCTIONS: &str = "\
Secretariat is ambient context for AI, stamped by humans. You read and verify \
envelopes; the principal enters to stamp the moments that count. You are the \
scribe; the principal stamps, you never do. Stamping embeds an `$attestation` \
block in place — no rename, no path change. Stamping is gated by Touch ID.

Writing docs: use `compose`, not a generic file write. Compose places the doc \
by convention (`docs/<bucket>/<date>-<slug>.md` in a registered repo), signs \
the body with your scribe `$signature` at birth, and commits exactly that one \
path. Render the full body to the principal before composing. A composed doc \
is signed-only (informational) until the principal stamps it.

Stamp ceremony (mandatory before calling `stamp`):
  1. Call `read` on the same `file_path`.
  2. Render the FULL decrypted body verbatim — code block or quoted region, \
never a summary.
  3. Wait for explicit consent in the same turn (e.g. \"stamp it\"). Implicit \
consent from a prior turn does not count if the file changed.
  4. Only then call `stamp`. The Touch ID dialog reason carries the \
document's first-line headline + a short hash prefix; if it differs from what \
you displayed, abort.

Always `verify` inbound envelopes before trusting their content.";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn invalid_request(msg: String) -> ErrorData {
    ErrorData::new(ErrorCode::INVALID_REQUEST, msg, None)
}

fn repo_to_dto(e: secretariat_core::infrastructure::RepoEntry) -> RepoDto {
    RepoDto {
        path: e.path.display().to_string(),
        role: e.role.as_str().to_string(),
        tags: e.tags,
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

#[cfg(test)]
mod repo_tool_tests {
    use secretariat_core::application::repo_ops::{list_repos, register_repo, unregister_repo};
    use secretariat_core::infrastructure::RepoRole;
    use tempfile::TempDir;

    #[test]
    fn repo_ops_roundtrip_under_temp_prefs() {
        let d = TempDir::new().unwrap();
        let repo = d.path().join("themia");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let prefs = d.path().join("preferences.toml");

        register_repo(&prefs, &repo, RepoRole::Home, vec!["themia".into()]).unwrap();
        let listed = list_repos(&prefs, Some("themia")).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].role, RepoRole::Home);

        assert!(unregister_repo(&prefs, &repo).unwrap());
        assert!(list_repos(&prefs, None).unwrap().is_empty());
    }
}
