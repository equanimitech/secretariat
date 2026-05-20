//! One-shot cognition: classification, enrichment, route-suggestion.
//!
//! Today the only call is `route_capture` (contextify-capture asks the
//! adapter where this body belongs). Future siblings under the same
//! port: summary-on-capture, tag suggestion, digest-distill — anything
//! shaped as "one prompt in, one structured answer out."

use thiserror::Error;

use crate::domain::QueueHandle;

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

/// Pluggable one-shot cognition. The use case calls this to ask "where
/// should this capture body live?" and never sees the model behind the
/// answer. Concrete adapters wire Claude / local LLMs / etc.
///
/// Default state is **no adapter** — every capture stays in the queue
/// the principal explicitly chose. An adapter exists only if the
/// principal has opted in.
pub trait CognitionRouting: Send + Sync {
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
