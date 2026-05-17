//! Ports — traits the domain depends on. Implementations live in `infrastructure`.
//!
//! Step 2 of the implementation plan. Populated below.

use thiserror::Error;

use crate::domain::{Did, DocHash, QueueHandle, Signature};

// -- Signer -------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum SignerError {
    #[error("biometric authentication failed or was cancelled")]
    BiometricRefused,
    #[error("signing key not available: {0}")]
    KeyUnavailable(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("internal: {0}")]
    Internal(String),
}

/// Produces detached ed25519 signatures over a doc hash, gated by a humanness check.
pub trait Signer {
    fn signer_did(&self) -> &Did;
    fn sign(&self, doc_hash: &DocHash, reason: &str) -> Result<Signature, SignerError>;
}

// -- DidResolver --------------------------------------------------------------

/// Result of resolving a DID to a usable verifying key set.
#[derive(Debug, Clone)]
pub struct ResolvedDid {
    pub did: Did,
    /// One or more 32-byte ed25519 verifying keys listed under `assertionMethod`.
    pub stamp_public_keys: Vec<[u8; 32]>,
    /// The raw DID document JSON, retained for caching and future fields.
    pub raw_document: serde_json::Value,
}

#[derive(Debug, Clone, Error)]
pub enum DidResolutionError {
    #[error("did document not found at {url}")]
    NotFound { url: String },
    #[error("did document is malformed: {0}")]
    Malformed(String),
    #[error("no ed25519 verification method present in did document")]
    NoEd25519Key,
    #[error("network error: {0}")]
    Network(String),
}

/// Resolves a `Did` to its document. Implementations may cache.
pub trait DidResolver {
    fn resolve(&self, did: &Did) -> Result<ResolvedDid, DidResolutionError>;
}

// -- CognitionPort -----------------------------------------------------------
//
// Pluggable substrate for AI-driven enrichment of captures. Domain stays
// neutral: no Anthropic types, no model names, no SDK leakage. Adapters
// live in `infrastructure/cognition/`.
//
// First and only method today: `route_capture` — given a capture body
// and the principal's existing queue vocabulary, suggest which queue the
// thought belongs in. Default state of the system has no adapter wired,
// in which case `route_capture` returns `CognitionError::NotConfigured`
// and the contextification job is a no-op. Sovereignty over cognition is
// the architectural invariant; default-off is the threat-model default.

/// What a cognition adapter returns when it routes a capture body.
#[derive(Debug, Clone)]
pub struct RouteSuggestion {
    /// Where the adapter thinks this capture belongs.
    pub queue: QueueHandle,
    /// Self-rated, in `[0.0, 1.0]`. LLM self-confidence is theatrical at
    /// the calibration level — the contextification use case treats this
    /// as a coarse gate (above/below the principal's threshold), not a
    /// fine-grained signal.
    pub confidence: f32,
    /// Free-form sentence explaining the choice. Logged to the ledger so
    /// the principal can audit when calibration drifts.
    pub rationale: String,
    /// Adapter identifier (e.g. `"claude-opus-4-7"`, `"local-llama3-8b"`).
    /// Logged for retroactive reasoning when prompts/models change.
    pub model: String,
    /// Bumped by the adapter every time its prompt changes. Lets the
    /// ledger be replayed against a known prompt baseline.
    pub prompt_version: String,
}

#[derive(Debug, Error)]
pub enum CognitionError {
    /// No adapter wired, or adapter exists but missing required config
    /// (e.g. BYOK key file). Caller treats this as "feature off" — never
    /// surfaces as a hard failure; contextification simply skips.
    #[error("cognition adapter not configured")]
    NotConfigured,
    /// Adapter wired but the chosen substrate isn't reachable. Caller
    /// logs and skips the routing decision; capture stays in original
    /// queue.
    #[error("network error: {0}")]
    Network(String),
    #[error("rate limited by cognition substrate")]
    RateLimited,
    #[error("invalid response from cognition substrate: {0}")]
    InvalidResponse(String),
    /// Adapter signaled the suggestion was below the meaningful-signal
    /// threshold (model declined to commit). Caller treats as no-op.
    #[error("cognition substrate abstained")]
    Abstained,
    #[error("internal: {0}")]
    Internal(String),
}

/// Pluggable cognition substrate. The use case calls this to ask "where
/// should this capture body live?" and never sees the model behind the
/// answer. Concrete adapters wire Claude / local LLMs / etc.
///
/// Default state is **no adapter** — every capture stays in the queue
/// the principal explicitly chose. An adapter exists only if the
/// principal has opted in (currently by writing
/// `~/.secretariat/cognition.json`).
pub trait CognitionPort: Send + Sync {
    /// Suggest a queue for the body. `existing_queues` is the principal's
    /// current vocabulary — the adapter must constrain its suggestion to
    /// this set unless it returns very high confidence on a new handle
    /// the use case will validate.
    fn route_capture(
        &self,
        body: &str,
        existing_queues: &[QueueHandle],
    ) -> impl std::future::Future<Output = Result<RouteSuggestion, CognitionError>> + Send;
}

// -- CognitionLauncher --------------------------------------------------------
//
// Plans how to start the principal's chosen interactive cognition CLI
// inside a channel-bound cwd. Pure planning — the use case returns the
// plan and the host (CLI exec, future MCP `launch_channel`) decides
// whether to replace the process, spawn-detach, or hand it to a
// terminal. Substrate-agnostic by design: today's only adapter wraps
// Claude Code (`claude`), but the same shape covers a future LM Studio
// CLI, Ollama wrapper, or BYOK runner without touching application
// code. See `docs/developer/launch.md`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    /// Executable to start, resolved against `$PATH` or an absolute path.
    pub command: String,
    /// Args passed after `command`. Concatenated from the adapter's
    /// base args and any user-configured `launch_args`.
    pub args: Vec<String>,
    /// Working directory for the launched process — typically the
    /// channel-dir, possibly remapped via [`crate::domain::ChannelBinding`].
    pub cwd: std::path::PathBuf,
    /// Env overrides layered on top of the parent process env. Empty
    /// when running against the principal's default cognition (e.g.
    /// Claude API). Populated when routing to LM Studio or another
    /// OpenAI-compatible endpoint via `ANTHROPIC_BASE_URL` etc.
    pub env: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("launch_command is empty in preferences")]
    EmptyCommand,
}

pub trait CognitionLauncher: Send + Sync {
    /// Build a [`LaunchPlan`] describing how to start an interactive
    /// session rooted at `cwd`. Pure — no process is spawned here.
    fn plan_launch(&self, cwd: &std::path::Path) -> Result<LaunchPlan, LauncherError>;
}
