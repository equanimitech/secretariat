//! Use case: distill a markdown body into AG (attentional-granularity)
//! fields — `title`, `lede`, `summary`.
//!
//! Called at envelope-write time from `compose_envelope_with_ag` and
//! `capture_to_queue_with_ag` when the author left the AG triplet
//! empty. The scribe drafts; the receiver decides.
//!
//! Invariants:
//!
//! - Never overrides author-supplied fields. If the caller passes any
//!   one of title/lede/summary, the use case returns `None` and the
//!   author's intent stands.
//! - Never fires on encrypted bodies. The scribe can only see plaintext.
//! - Never fires on short bodies (`< AG_MIN_BODY_CHARS` and no
//!   paragraph break). The cost (one API call, latency, model drift)
//!   isn't worth it for a one-liner — the body already *is* the gross
//!   signal.
//! - Never crashes a write. Adapter not configured, network failures,
//!   malformed responses — all surface as `Ok(None)` so the envelope
//!   still writes. The principal's correspondence does not depend on
//!   the cognition substrate.
//!
//! Threat model: only the body bytes cross the wire to the cognition
//! substrate. No envelope metadata, recipient, channel name, or
//! contract terms. The adapter's own threat-model contract enforces
//! this; the use case never adds context to the call.

use crate::infrastructure::cognition::AnyCognitionAdapter;
use crate::infrastructure::preferences::CognitionPrefs;
use crate::ports::{AgFields, CognitionAg, CognitionError};

/// Minimum body length (bytes) below which AG extraction does not fire
/// unless the body contains a paragraph break (`\n\n`). Captures and
/// composes shorter than this are typically a sentence — the body *is*
/// the gross signal already.
pub const AG_MIN_BODY_CHARS: usize = 280;

/// Returns true when the body is substantive enough to warrant an AG
/// extraction pass. Either it's at least `AG_MIN_BODY_CHARS` long, or
/// it has a paragraph break (a multi-paragraph note even if short).
pub fn body_warrants_ag(body: &str) -> bool {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.chars().count() >= AG_MIN_BODY_CHARS || trimmed.contains("\n\n")
}

/// What the caller passes in for the existing AG fields (whatever the
/// author supplied, possibly all `None`).
#[derive(Debug, Clone, Default)]
pub struct AuthorAgFields {
    pub title: Option<String>,
    pub lede: Option<String>,
    pub summary: Option<String>,
}

impl AuthorAgFields {
    /// True when at least one of the three is set — the author has
    /// expressed AG intent, and the scribe must stand down.
    pub fn any_set(&self) -> bool {
        self.title.is_some() || self.lede.is_some() || self.summary.is_some()
    }
}

/// Outcome of a `try_extract_ag` call. The variants record *why* AG
/// fields are or aren't being applied so the caller can ledger or
/// surface the reason.
#[derive(Debug, Clone)]
pub enum AgExtractOutcome {
    /// Extraction succeeded; caller should populate the envelope and
    /// mark `ag_source = Ai`.
    Generated(AgFields),
    /// Author supplied at least one AG field — nothing to do.
    AuthorSupplied,
    /// Body too short / encrypted / empty — bypass.
    BelowThreshold,
    /// No cognition adapter configured (default state).
    AdapterNotConfigured,
    /// Adapter wired but the call failed (network, rate limit, bad
    /// response). Stringified for tracing; the envelope still writes
    /// without AG fields.
    AdapterError(String),
}

/// Best-effort entry point. Always returns `Ok`; failure modes are
/// represented as `AgExtractOutcome` variants so the caller never has
/// to branch on `Result` for a non-essential enrichment.
///
/// Inputs:
///
/// - `body` — plaintext markdown. Encrypted bodies are filtered out
///   by the caller (the use case doesn't see encryption state directly).
/// - `author` — what the caller already has from the principal.
/// - `cognition_prefs` — the `[cognition]` block from `preferences.toml`.
///   When unset, `AdapterNotConfigured` is returned.
pub async fn try_extract_ag(
    body: &str,
    author: &AuthorAgFields,
    cognition_prefs: &CognitionPrefs,
) -> AgExtractOutcome {
    if author.any_set() {
        return AgExtractOutcome::AuthorSupplied;
    }
    if !body_warrants_ag(body) {
        return AgExtractOutcome::BelowThreshold;
    }
    let Some(adapter) = AnyCognitionAdapter::from_prefs(cognition_prefs) else {
        return AgExtractOutcome::AdapterNotConfigured;
    };
    try_extract_with_adapter(body, &adapter).await
}

/// Lower-level variant for tests + callers that have already built an
/// adapter (or want to inject a scripted one). Same outcome shape.
pub async fn try_extract_with_adapter<A: CognitionAg>(
    body: &str,
    adapter: &A,
) -> AgExtractOutcome {
    match adapter.extract_ag(body).await {
        Ok(fields) => AgExtractOutcome::Generated(fields),
        Err(CognitionError::NotConfigured) => AgExtractOutcome::AdapterNotConfigured,
        Err(other) => {
            tracing::warn!(error = %other, "AG extraction failed; envelope writes without AG fields");
            AgExtractOutcome::AdapterError(other.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::AgFields;

    struct ScriptedAdapter {
        answer: Result<AgFields, CognitionError>,
    }

    impl CognitionAg for ScriptedAdapter {
        async fn extract_ag(&self, _body: &str) -> Result<AgFields, CognitionError> {
            match &self.answer {
                Ok(f) => Ok(f.clone()),
                Err(CognitionError::NotConfigured) => Err(CognitionError::NotConfigured),
                Err(other) => Err(CognitionError::Internal(other.to_string())),
            }
        }
    }

    #[test]
    fn body_warrants_ag_short_oneliner_skips() {
        assert!(!body_warrants_ag("ping dad about the book"));
    }

    #[test]
    fn body_warrants_ag_long_singleparagraph_fires() {
        let s = "a".repeat(AG_MIN_BODY_CHARS + 1);
        assert!(body_warrants_ag(&s));
    }

    #[test]
    fn body_warrants_ag_short_multiparagraph_fires() {
        assert!(body_warrants_ag("first thought.\n\nsecond thought."));
    }

    #[test]
    fn body_warrants_ag_empty_skips() {
        assert!(!body_warrants_ag(""));
        assert!(!body_warrants_ag("   \n\t  "));
    }

    #[tokio::test]
    async fn author_supplied_short_circuits() {
        let body = "x".repeat(AG_MIN_BODY_CHARS + 50);
        let author = AuthorAgFields {
            title: Some("already chosen".into()),
            ..AuthorAgFields::default()
        };
        let prefs = CognitionPrefs::default(); // no adapter
        let out = try_extract_ag(&body, &author, &prefs).await;
        assert!(matches!(out, AgExtractOutcome::AuthorSupplied));
    }

    #[tokio::test]
    async fn no_adapter_skips() {
        let body = "x".repeat(AG_MIN_BODY_CHARS + 1);
        let prefs = CognitionPrefs::default();
        let out = try_extract_ag(&body, &AuthorAgFields::default(), &prefs).await;
        assert!(matches!(out, AgExtractOutcome::AdapterNotConfigured));
    }

    #[tokio::test]
    async fn short_body_skips_even_when_adapter_present() {
        let adapter = ScriptedAdapter {
            answer: Ok(AgFields {
                title: "t".into(),
                lede: "l".into(),
                summary: "s".into(),
                model: "m".into(),
                prompt_version: "v1".into(),
            }),
        };
        // Adapter ignored — body gates before we reach it.
        let body = "ping dad";
        // We can still test the path that goes through the adapter
        // explicitly when we want to:
        let out =
            super::try_extract_ag(body, &AuthorAgFields::default(), &CognitionPrefs::default())
                .await;
        assert!(matches!(out, AgExtractOutcome::BelowThreshold));
        // Sanity-check the adapter side too.
        let direct = try_extract_with_adapter(body, &adapter).await;
        assert!(matches!(direct, AgExtractOutcome::Generated(_)));
    }

    #[tokio::test]
    async fn adapter_error_returns_outcome_variant() {
        let adapter = ScriptedAdapter {
            answer: Err(CognitionError::Network("boom".into())),
        };
        let body = "x".repeat(AG_MIN_BODY_CHARS + 1);
        let out = try_extract_with_adapter(&body, &adapter).await;
        assert!(matches!(out, AgExtractOutcome::AdapterError(_)));
    }

    #[tokio::test]
    async fn adapter_success_returns_generated() {
        let adapter = ScriptedAdapter {
            answer: Ok(AgFields {
                title: "Book chapter 3".into(),
                lede: "Marcelo wants more pressure in the third chapter.".into(),
                summary: "Notes on chapter 3 revisions.".into(),
                model: "claude-haiku-4-5".into(),
                prompt_version: "v1".into(),
            }),
        };
        let body = "x".repeat(AG_MIN_BODY_CHARS + 1);
        let out = try_extract_with_adapter(&body, &adapter).await;
        match out {
            AgExtractOutcome::Generated(f) => {
                assert_eq!(f.title, "Book chapter 3");
                assert!(!f.lede.is_empty());
            }
            other => panic!("expected Generated, got {other:?}"),
        }
    }
}
