//! Cognition port — extract AG (attentional-granularity) fields from a
//! body of markdown text.
//!
//! Sibling to [`CognitionRouting`](super::routing::CognitionRouting):
//! same "one prompt in, one structured answer out" shape, different
//! payload. Where routing classifies a body into a queue, AG extraction
//! distills the body into the three AG layers (title, lede, summary).
//!
//! The fields are author-populated when present; this port exists so
//! the scribe can fill them when the author left them empty. Receivers
//! see `ag_source = "ai"` in that case so the provenance is never
//! invisible.
//!
//! Default state — `NotConfigured` — produces no AG fields. The
//! envelope still writes successfully without them; renderers fall
//! back to the body's first heading + first lines.

use crate::ports::CognitionError;

/// What an AG extraction adapter returns for one body.
///
/// All three fields are populated when the call succeeds. If the
/// adapter can't honor that contract it returns
/// [`CognitionError::InvalidResponse`] instead — partial AG triplets
/// would leak into the envelope and confuse the timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgFields {
    /// Gross signal — 2–6 words, plain text. Used as card title.
    pub title: String,
    /// Subtle signal — one sentence, plain text. Used as preview line.
    pub lede: String,
    /// Deepening pathway — 2–4 sentences, plain text. Used in expanded
    /// detail views.
    pub summary: String,
    /// Adapter identifier — logged so the principal can audit which
    /// model wrote which envelope's AG.
    pub model: String,
    /// Bumped whenever the AG prompt template changes. Lets retroactive
    /// reasoning compare envelopes against a known prompt baseline.
    pub prompt_version: String,
}

/// Pluggable AG extraction. Concrete adapters wire Claude / OpenAI-compat
/// / local models / etc. Like routing, the adapter sees **only the body** —
/// no envelope metadata, no recipient, no context beyond the markdown
/// the author typed.
///
/// Default state is **no adapter** — every envelope writes with
/// whatever AG fields the author supplied (possibly none). An adapter
/// exists only when the principal opts in.
pub trait CognitionAg: Send + Sync {
    fn extract_ag(
        &self,
        body: &str,
    ) -> impl std::future::Future<Output = Result<AgFields, CognitionError>> + Send;
}
