//! `ClaudeCognitionAdapter` — BYOK Anthropic Messages API adapter.
//!
//! Built only when the cognition config selects `provider: "anthropic"`.
//! Calls `POST /v1/messages` with the capture body and parses the
//! returned JSON into a `RouteSuggestion`.
//!
//! Threat-model invariants enforced here, not at the use case:
//!
//! - The API call carries **only the capture body**. Adjacent captures,
//!   contact identifiers, queue contents, channel contracts — none of
//!   it leaves the device.
//! - The system prompt + queue list is logged via tracing at debug
//!   level so the principal can audit what shape of context the model
//!   sees. The body is **not** logged.
//! - Default model is a Haiku-class one — fast, cheap, right for
//!   classification.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::domain::QueueHandle;
use crate::ports::{AgFields, CognitionAg, CognitionError, CognitionRouting, RouteSuggestion};

use super::config::CognitionConfig;

const DEFAULT_API_BASE: &str = "https://api.anthropic.com";
const DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);

/// Anthropic models we offer in the picker. Anthropic does not expose
/// a public `/models` endpoint requiring no auth — this list is the
/// known-good set for routing-class use as of May 2026.
pub const KNOWN_ANTHROPIC_MODELS: &[&str] = &[
    "claude-haiku-4-5-20251001",
    "claude-sonnet-4-6",
    "claude-opus-4-7",
];

#[derive(Debug, Clone)]
pub struct ClaudeCognitionAdapter {
    config: CognitionConfig,
    /// Bumped here whenever the prompt template changes. Logged into
    /// the ledger so old decisions can be reasoned about retroactively.
    prompt_version: &'static str,
}

impl ClaudeCognitionAdapter {
    /// Build from a `CognitionConfig` whose `provider` is anthropic.
    /// Returns `None` when the config selects a different provider OR
    /// when the required `api_key` is missing.
    pub fn from_config(config: CognitionConfig) -> Option<Self> {
        if config.api_key.as_deref().unwrap_or("").is_empty() {
            return None;
        }
        Some(Self {
            config,
            prompt_version: "v1",
        })
    }

    pub fn config(&self) -> &CognitionConfig {
        &self.config
    }

    pub fn model_or_default(&self) -> &str {
        self.config.model.as_deref().unwrap_or(DEFAULT_MODEL)
    }

    pub fn api_base_or_default(&self) -> &str {
        self.config.api_base.as_deref().unwrap_or(DEFAULT_API_BASE)
    }
}

// ---------------------------------------------------------------------------
// Prompt + response shape — pulled out so they're testable without HTTP.
// ---------------------------------------------------------------------------

/// Build the system prompt that frames the routing task. The list of
/// existing queues is inlined so the model is constrained to vocabulary
/// the principal already uses.
pub(crate) fn build_system_prompt(existing_queues: &[QueueHandle]) -> String {
    let list = if existing_queues.is_empty() {
        "(no queues exist yet — only `inbox:triage` is in scope)".to_string()
    } else {
        existing_queues
            .iter()
            .map(|q| format!("- {}", q.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "You are a queue router for a personal capture system. \
The principal has just captured a thought (provided in the user message). \
Pick the queue that best fits the capture from this list of queues the principal already uses:\n\n\
{list}\n\n\
Respond ONLY with JSON of this exact shape, nothing before or after:\n\
{{\"queue\":\"<namespace>:<slug>\",\"confidence\":<float between 0.0 and 1.0>,\"rationale\":\"<one sentence>\"}}\n\n\
Rules:\n\
- Stay within the list above unless none could plausibly fit. If you must invent a new handle, follow the `<namespace>:<slug>` form (lowercase letters, digits, hyphens) and only do so when confidence is at least 0.9.\n\
- If the capture is too ambiguous to confidently route, return queue=\"inbox:triage\" and confidence=0.0.\n\
- The rationale must be one short sentence. No bullet lists, no markdown.\n\
- The rationale must not quote the body verbatim — describe the type of thought instead."
    )
}

/// Anthropic Messages-API request body (a tiny subset — only the
/// fields we set).
#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize, Debug)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentBlock {
    Text {
        text: String,
    },
    /// Catch-all so the deserializer doesn't fail on tool-use /
    /// thinking blocks if the model emits them.
    #[serde(other)]
    Other,
}

#[derive(Deserialize, Debug)]
pub(crate) struct RoutingPayload {
    pub queue: String,
    pub confidence: f32,
    pub rationale: String,
}

/// Pull the JSON payload out of an Anthropic response and parse it
/// into a `RouteSuggestion`.
pub(crate) fn parse_response(
    raw: &str,
    model: &str,
    prompt_version: &str,
) -> Result<RouteSuggestion, CognitionError> {
    let parsed: AnthropicResponse = serde_json::from_str(raw)
        .map_err(|e| CognitionError::InvalidResponse(format!("envelope parse: {e}")))?;
    let text = parsed
        .content
        .into_iter()
        .find_map(|block| match block {
            AnthropicContentBlock::Text { text } => Some(text),
            AnthropicContentBlock::Other => None,
        })
        .ok_or_else(|| CognitionError::InvalidResponse("no text block in response".into()))?;
    routing_payload_to_suggestion(&text, model, prompt_version)
}

/// Validate a `{queue, confidence, rationale}` blob and lift it into a
/// `RouteSuggestion`. Shared between the Anthropic + OpenAI-compat
/// adapters because both ask the model for the same JSON shape.
pub(crate) fn routing_payload_to_suggestion(
    text: &str,
    model: &str,
    prompt_version: &str,
) -> Result<RouteSuggestion, CognitionError> {
    let trimmed = text.trim();
    let payload: RoutingPayload = serde_json::from_str(trimmed).map_err(|e| {
        CognitionError::InvalidResponse(format!("payload parse: {e} from `{trimmed}`"))
    })?;

    let queue = QueueHandle::parse(&payload.queue).map_err(|e| {
        CognitionError::InvalidResponse(format!(
            "model returned invalid queue handle `{}`: {e}",
            payload.queue
        ))
    })?;

    if !(0.0..=1.0).contains(&payload.confidence) {
        return Err(CognitionError::InvalidResponse(format!(
            "confidence out of range: {}",
            payload.confidence
        )));
    }

    Ok(RouteSuggestion {
        queue,
        confidence: payload.confidence,
        rationale: payload.rationale,
        model: model.to_string(),
        prompt_version: prompt_version.to_string(),
    })
}

// ---------------------------------------------------------------------------
// AG extraction — title / lede / summary distillation.
//
// Shape mirrors the routing path: a system prompt frames the task, the user
// message carries the body, the model replies with strict JSON, and a shared
// validator lifts the payload into the port-level value object. Same threat
// model: only the body crosses the wire; no envelope metadata, no recipient.
// ---------------------------------------------------------------------------

/// Build the AG extraction system prompt. Intentionally tiny + deterministic
/// — the smaller the surface, the less drift between providers and prompt
/// versions. Kept in one place so future siblings (OpenAI-compat, local
/// models) ride the same shape.
pub(crate) fn build_ag_system_prompt() -> &'static str {
    "You generate three AG (attentional-granularity) fields for a markdown body. \
The fields form a gross→subtle deepening pathway:\n\
- title: 2–6 words, the gross signal — what the body is *about* at a glance.\n\
- lede: one sentence, the sharper signal — what the body *says* in a line.\n\
- summary: 2–4 sentences, the full subtle signal — what the body *covers*.\n\n\
Rules:\n\
- Plain text only. No markdown, no quoting the body verbatim.\n\
- Reply with ONLY JSON of this exact shape, nothing before or after:\n\
{\"title\":\"...\",\"lede\":\"...\",\"summary\":\"...\"}\n"
}

#[derive(Deserialize, Debug)]
pub(crate) struct AgPayload {
    pub title: String,
    pub lede: String,
    pub summary: String,
}

/// Validate a `{title, lede, summary}` blob and lift it into `AgFields`.
/// Shared between Anthropic + OpenAI-compat adapters because both ask
/// the model for the same JSON shape.
pub(crate) fn ag_payload_to_fields(
    text: &str,
    model: &str,
    prompt_version: &str,
) -> Result<AgFields, CognitionError> {
    let trimmed = text.trim();
    let payload: AgPayload = serde_json::from_str(trimmed).map_err(|e| {
        CognitionError::InvalidResponse(format!("ag payload parse: {e} from `{trimmed}`"))
    })?;
    if payload.title.trim().is_empty()
        || payload.lede.trim().is_empty()
        || payload.summary.trim().is_empty()
    {
        return Err(CognitionError::InvalidResponse(
            "ag payload has empty title/lede/summary".into(),
        ));
    }
    Ok(AgFields {
        title: payload.title.trim().to_string(),
        lede: payload.lede.trim().to_string(),
        summary: payload.summary.trim().to_string(),
        model: model.to_string(),
        prompt_version: prompt_version.to_string(),
    })
}

/// Pull the JSON payload out of an Anthropic response and parse it into
/// `AgFields`. Mirrors `parse_response` for the routing path.
pub(crate) fn parse_ag_response(
    raw: &str,
    model: &str,
    prompt_version: &str,
) -> Result<AgFields, CognitionError> {
    let parsed: AnthropicResponse = serde_json::from_str(raw)
        .map_err(|e| CognitionError::InvalidResponse(format!("envelope parse: {e}")))?;
    let text = parsed
        .content
        .into_iter()
        .find_map(|block| match block {
            AnthropicContentBlock::Text { text } => Some(text),
            AnthropicContentBlock::Other => None,
        })
        .ok_or_else(|| CognitionError::InvalidResponse("no text block in response".into()))?;
    ag_payload_to_fields(&text, model, prompt_version)
}

impl CognitionAg for ClaudeCognitionAdapter {
    async fn extract_ag(&self, body: &str) -> Result<AgFields, CognitionError> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or(CognitionError::NotConfigured)?;
        let system = build_ag_system_prompt().to_string();
        debug!(
            model = self.model_or_default(),
            prompt_version = self.prompt_version,
            body_len = body.len(),
            "calling Anthropic Messages API for AG extraction"
        );

        let request = AnthropicRequest {
            model: self.model_or_default(),
            max_tokens: 512,
            system,
            messages: vec![AnthropicMessage {
                role: "user",
                content: body,
            }],
        };

        let client = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| CognitionError::Network(e.to_string()))?;

        let url = format!("{}/v1/messages", self.api_base_or_default());
        let resp = client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| CognitionError::Network(e.to_string()))?;

        let status = resp.status();
        if status.as_u16() == 429 {
            return Err(CognitionError::RateLimited);
        }
        let text = resp
            .text()
            .await
            .map_err(|e| CognitionError::Network(format!("reading response body: {e}")))?;
        if !status.is_success() {
            return Err(CognitionError::Network(format!("HTTP {status}: {text}")));
        }
        parse_ag_response(&text, self.model_or_default(), self.prompt_version)
    }
}

impl CognitionRouting for ClaudeCognitionAdapter {
    async fn route_capture(
        &self,
        body: &str,
        existing_queues: &[QueueHandle],
    ) -> Result<RouteSuggestion, CognitionError> {
        let api_key = self
            .config
            .api_key
            .as_deref()
            .ok_or(CognitionError::NotConfigured)?;
        let system = build_system_prompt(existing_queues);
        debug!(
            queues = existing_queues.len(),
            model = self.model_or_default(),
            prompt_version = self.prompt_version,
            "calling Anthropic Messages API for capture routing"
        );

        let request = AnthropicRequest {
            model: self.model_or_default(),
            max_tokens: 256,
            system,
            messages: vec![AnthropicMessage {
                role: "user",
                content: body,
            }],
        };

        let client = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| CognitionError::Network(e.to_string()))?;

        let url = format!("{}/v1/messages", self.api_base_or_default());
        let resp = client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| CognitionError::Network(e.to_string()))?;

        let status = resp.status();
        if status.as_u16() == 429 {
            return Err(CognitionError::RateLimited);
        }
        let text = resp
            .text()
            .await
            .map_err(|e| CognitionError::Network(format!("reading response body: {e}")))?;
        if !status.is_success() {
            return Err(CognitionError::Network(format!("HTTP {status}: {text}")));
        }
        parse_response(&text, self.model_or_default(), self.prompt_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::cognition::config::Provider;

    fn cfg_with_key() -> CognitionConfig {
        CognitionConfig {
            provider: Provider::Anthropic,
            api_key: Some("sk-ant-test".into()),
            api_base: None,
            model: None,
            route_threshold: None,
        }
    }

    #[test]
    fn requires_api_key() {
        let cfg = CognitionConfig {
            provider: Provider::Anthropic,
            api_key: None,
            ..cfg_with_key()
        };
        assert!(ClaudeCognitionAdapter::from_config(cfg).is_none());
    }

    #[test]
    fn defaults_apply_when_omitted() {
        let adapter = ClaudeCognitionAdapter::from_config(cfg_with_key()).unwrap();
        assert!(adapter.model_or_default().starts_with("claude-"));
        assert_eq!(adapter.api_base_or_default(), DEFAULT_API_BASE);
    }

    #[test]
    fn system_prompt_lists_existing_queues() {
        let queues = vec![
            QueueHandle::parse("inbox:triage").unwrap(),
            QueueHandle::parse("inbox:pain").unwrap(),
            QueueHandle::parse("area:health").unwrap(),
        ];
        let prompt = build_system_prompt(&queues);
        assert!(prompt.contains("- inbox:triage"));
        assert!(prompt.contains("- inbox:pain"));
        assert!(prompt.contains("- area:health"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn system_prompt_handles_empty_vocabulary() {
        let prompt = build_system_prompt(&[]);
        assert!(prompt.contains("inbox:triage"));
    }

    #[test]
    fn parse_response_happy_path() {
        let raw = r#"{
            "id":"msg_1",
            "type":"message",
            "role":"assistant",
            "content":[{"type":"text","text":"{\"queue\":\"inbox:pain\",\"confidence\":0.83,\"rationale\":\"complaint about onboarding\"}"}],
            "stop_reason":"end_turn"
        }"#;
        let suggestion = parse_response(raw, "claude-haiku-4-5", "v1").unwrap();
        assert_eq!(suggestion.queue.as_str(), "inbox:pain");
        assert!((suggestion.confidence - 0.83).abs() < 0.001);
        assert_eq!(suggestion.rationale, "complaint about onboarding");
        assert_eq!(suggestion.model, "claude-haiku-4-5");
        assert_eq!(suggestion.prompt_version, "v1");
    }

    #[test]
    fn parse_response_tolerates_whitespace_around_json() {
        let raw = r#"{
            "content":[{"type":"text","text":"\n  {\"queue\":\"inbox:idea\",\"confidence\":0.91,\"rationale\":\"product idea\"}  \n"}]
        }"#;
        let suggestion = parse_response(raw, "m", "v1").unwrap();
        assert_eq!(suggestion.queue.as_str(), "inbox:idea");
    }

    #[test]
    fn parse_response_skips_non_text_blocks() {
        let raw = r#"{
            "content":[
                {"type":"thinking","thinking":"..."},
                {"type":"text","text":"{\"queue\":\"inbox:triage\",\"confidence\":0.0,\"rationale\":\"unsure\"}"}
            ]
        }"#;
        let suggestion = parse_response(raw, "m", "v1").unwrap();
        assert_eq!(suggestion.queue.as_str(), "inbox:triage");
    }

    #[test]
    fn parse_response_rejects_no_text_block() {
        let raw = r#"{"content":[{"type":"thinking","thinking":"..."}]}"#;
        let err = parse_response(raw, "m", "v1").unwrap_err();
        assert!(matches!(err, CognitionError::InvalidResponse(_)));
    }

    #[test]
    fn parse_response_rejects_invalid_queue_handle() {
        let raw = r#"{
            "content":[{"type":"text","text":"{\"queue\":\"NOT VALID\",\"confidence\":0.9,\"rationale\":\"x\"}"}]
        }"#;
        let err = parse_response(raw, "m", "v1").unwrap_err();
        assert!(matches!(err, CognitionError::InvalidResponse(_)));
    }

    #[test]
    fn parse_ag_response_happy_path() {
        let raw = r#"{
            "id":"msg_1",
            "content":[{"type":"text","text":"{\"title\":\"Chapter 3 pressure\",\"lede\":\"Marcelo wants more pressure in chapter 3.\",\"summary\":\"Notes on chapter 3 revisions. Marcelo flagged tension as the weak spot. Action: re-read draft tonight.\"}"}]
        }"#;
        let fields = parse_ag_response(raw, "claude-haiku-4-5", "v1").unwrap();
        assert_eq!(fields.title, "Chapter 3 pressure");
        assert!(fields.lede.contains("pressure"));
        assert!(fields.summary.contains("Marcelo"));
        assert_eq!(fields.model, "claude-haiku-4-5");
        assert_eq!(fields.prompt_version, "v1");
    }

    #[test]
    fn parse_ag_response_rejects_empty_field() {
        let raw = r#"{
            "content":[{"type":"text","text":"{\"title\":\"\",\"lede\":\"x\",\"summary\":\"y\"}"}]
        }"#;
        let err = parse_ag_response(raw, "m", "v1").unwrap_err();
        assert!(matches!(err, CognitionError::InvalidResponse(_)));
    }

    #[test]
    fn parse_ag_response_trims_whitespace() {
        let raw = r#"{
            "content":[{"type":"text","text":"{\"title\":\"  T  \",\"lede\":\"L\",\"summary\":\"S\"}"}]
        }"#;
        let fields = parse_ag_response(raw, "m", "v1").unwrap();
        assert_eq!(fields.title, "T");
    }

    #[test]
    fn build_ag_system_prompt_mentions_each_field() {
        let prompt = build_ag_system_prompt();
        assert!(prompt.contains("title"));
        assert!(prompt.contains("lede"));
        assert!(prompt.contains("summary"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn parse_response_rejects_confidence_out_of_range() {
        let raw = r#"{
            "content":[{"type":"text","text":"{\"queue\":\"inbox:triage\",\"confidence\":1.5,\"rationale\":\"x\"}"}]
        }"#;
        let err = parse_response(raw, "m", "v1").unwrap_err();
        assert!(matches!(err, CognitionError::InvalidResponse(_)));
    }
}
