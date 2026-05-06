//! `OpenAICompatibleAdapter` — covers any provider speaking the OpenAI
//! Chat-Completions shape.
//!
//! Tested call paths:
//! - **Ollama** (`http://localhost:11434/v1`) — fully sovereign,
//!   no network exfiltration, no API key. The strategically important
//!   case for this adapter.
//! - **OpenRouter** (`https://openrouter.ai/api/v1`) — multi-provider
//!   gateway, BYOK.
//! - Vanilla OpenAI / Together / Groq follow the same shape.
//!
//! Same threat-model invariants as the Anthropic adapter:
//! - The HTTP body carries **only the capture body**. Adjacent
//!   captures, contact identifiers, queue contents — none of it leaves
//!   the device.
//! - Body is not logged. Queue count + model + base URL are debug-logged
//!   so the principal can audit shape, not content.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::domain::QueueHandle;
use crate::ports::{CognitionError, CognitionPort, RouteSuggestion};

use super::claude::{build_system_prompt, routing_payload_to_suggestion};
use super::config::CognitionConfig;

const HTTP_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct OpenAICompatibleAdapter {
    config: CognitionConfig,
    prompt_version: &'static str,
}

impl OpenAICompatibleAdapter {
    /// Build from a `CognitionConfig` whose `provider` is openai-compat.
    /// Returns `None` when the required `api_base` is missing — there
    /// is no sensible default across Ollama / OpenRouter / OpenAI, so
    /// the principal must declare which.
    pub fn from_config(config: CognitionConfig) -> Option<Self> {
        if config.api_base.as_deref().unwrap_or("").is_empty() {
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

    /// Default model when omitted. We can't pick a sensible cross-
    /// provider default (a Llama tag is wrong on OpenRouter, a GPT id
    /// is wrong on Ollama), so this is intentionally a placeholder
    /// the caller is expected to override. Routing falls back to
    /// `gpt-4o-mini`-style naming because that's the most likely
    /// hosted-provider answer; Ollama users will have set a model.
    pub fn model_or_default(&self) -> &str {
        self.config.model.as_deref().unwrap_or("gpt-4o-mini")
    }

    pub fn api_base(&self) -> Option<&str> {
        self.config.api_base.as_deref()
    }

    /// Fetch the model catalog from the provider's `/models` endpoint.
    /// Used by the settings pane to populate the picker. Returns
    /// `Network` errors verbatim so the UI can surface a sensible
    /// message ("is Ollama running?", "wrong base URL", etc).
    pub async fn list_models(&self) -> Result<Vec<String>, CognitionError> {
        let base = self
            .api_base()
            .ok_or(CognitionError::NotConfigured)?
            .trim_end_matches('/');
        let url = format!("{base}/models");

        let mut req = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| CognitionError::Network(e.to_string()))?
            .get(&url);
        if let Some(key) = self.config.api_key.as_deref() {
            if !key.is_empty() {
                req = req.bearer_auth(key);
            }
        }
        let resp = req
            .send()
            .await
            .map_err(|e| CognitionError::Network(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| CognitionError::Network(format!("reading models body: {e}")))?;
        if !status.is_success() {
            return Err(CognitionError::Network(format!("HTTP {status}: {text}")));
        }
        let parsed: ModelsResponse = serde_json::from_str(&text)
            .map_err(|e| CognitionError::InvalidResponse(format!("models list parse: {e}")))?;
        let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
        ids.sort();
        Ok(ids)
    }
}

#[derive(Deserialize, Debug)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize, Debug)]
struct ModelEntry {
    id: String,
}

// ---------------------------------------------------------------------------
// Chat-Completions request + response shape.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    max_tokens: u32,
    /// Many OpenAI-compat servers honor this; Ollama ignores unknown
    /// fields without erroring, so it's safe to always send.
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize, Debug)]
struct ChatChoice {
    message: ChatMessageOut,
}

#[derive(Deserialize, Debug)]
struct ChatMessageOut {
    content: String,
}

/// Pull the assistant text out of a Chat-Completions response. Some
/// providers emit a leading code-fence (`json ... `); strip it before
/// handing to the shared payload validator.
pub(crate) fn parse_chat_response(
    raw: &str,
    model: &str,
    prompt_version: &str,
) -> Result<RouteSuggestion, CognitionError> {
    let parsed: ChatResponse = serde_json::from_str(raw)
        .map_err(|e| CognitionError::InvalidResponse(format!("envelope parse: {e}")))?;
    let content = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| CognitionError::InvalidResponse("no choices in response".into()))?
        .message
        .content;
    let unfenced = strip_code_fence(&content);
    routing_payload_to_suggestion(&unfenced, model, prompt_version)
}

/// Strip a leading ```json fence and trailing ``` if present. Handles
/// the common case where smaller models can't help themselves; passes
/// through unchanged when no fence is present.
fn strip_code_fence(s: &str) -> String {
    let trimmed = s.trim();
    let without_open = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    let without_close = without_open
        .trim_start_matches('\n')
        .trim_end()
        .strip_suffix("```")
        .unwrap_or_else(|| without_open.trim_start_matches('\n').trim_end());
    without_close.trim().to_string()
}

impl CognitionPort for OpenAICompatibleAdapter {
    async fn route_capture(
        &self,
        body: &str,
        existing_queues: &[QueueHandle],
    ) -> Result<RouteSuggestion, CognitionError> {
        let base = self
            .api_base()
            .ok_or(CognitionError::NotConfigured)?
            .trim_end_matches('/');
        let system = build_system_prompt(existing_queues);
        debug!(
            queues = existing_queues.len(),
            model = self.model_or_default(),
            api_base = base,
            prompt_version = self.prompt_version,
            "calling OpenAI-compat Chat Completions for capture routing"
        );

        let request = ChatRequest {
            model: self.model_or_default(),
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system.as_str(),
                },
                ChatMessage {
                    role: "user",
                    content: body,
                },
            ],
            max_tokens: 256,
            temperature: 0.0,
        };

        let mut req = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| CognitionError::Network(e.to_string()))?
            .post(format!("{base}/chat/completions"))
            .header("content-type", "application/json")
            .json(&request);
        if let Some(key) = self.config.api_key.as_deref() {
            if !key.is_empty() {
                req = req.bearer_auth(key);
            }
        }
        let resp = req
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
        parse_chat_response(&text, self.model_or_default(), self.prompt_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::cognition::config::Provider;

    fn ollama_cfg() -> CognitionConfig {
        CognitionConfig {
            provider: Provider::OpenAiCompat,
            api_key: None,
            api_base: Some("http://localhost:11434/v1".into()),
            model: Some("llama3.1:8b".into()),
            route_threshold: None,
        }
    }

    #[test]
    fn requires_api_base() {
        let cfg = CognitionConfig {
            provider: Provider::OpenAiCompat,
            api_key: None,
            api_base: None,
            model: None,
            route_threshold: None,
        };
        assert!(OpenAICompatibleAdapter::from_config(cfg).is_none());
    }

    #[test]
    fn ollama_config_yields_adapter() {
        let adapter = OpenAICompatibleAdapter::from_config(ollama_cfg()).unwrap();
        assert_eq!(adapter.model_or_default(), "llama3.1:8b");
        assert_eq!(adapter.api_base(), Some("http://localhost:11434/v1"));
    }

    #[test]
    fn parse_chat_response_happy_path() {
        let raw = r#"{
            "id":"chatcmpl-1",
            "choices":[{"index":0,"message":{"role":"assistant","content":"{\"queue\":\"inbox:pain\",\"confidence\":0.82,\"rationale\":\"x\"}"}}]
        }"#;
        let suggestion = parse_chat_response(raw, "llama3.1:8b", "v1").unwrap();
        assert_eq!(suggestion.queue.as_str(), "inbox:pain");
        assert!((suggestion.confidence - 0.82).abs() < 0.001);
        assert_eq!(suggestion.model, "llama3.1:8b");
    }

    #[test]
    fn parse_chat_response_strips_json_fence() {
        let raw = r#"{
            "choices":[{"message":{"role":"assistant","content":"```json\n{\"queue\":\"inbox:idea\",\"confidence\":0.9,\"rationale\":\"x\"}\n```"}}]
        }"#;
        let suggestion = parse_chat_response(raw, "m", "v1").unwrap();
        assert_eq!(suggestion.queue.as_str(), "inbox:idea");
    }

    #[test]
    fn parse_chat_response_strips_bare_fence() {
        let raw = r#"{
            "choices":[{"message":{"role":"assistant","content":"```\n{\"queue\":\"inbox:idea\",\"confidence\":0.9,\"rationale\":\"x\"}\n```"}}]
        }"#;
        let suggestion = parse_chat_response(raw, "m", "v1").unwrap();
        assert_eq!(suggestion.queue.as_str(), "inbox:idea");
    }

    #[test]
    fn parse_chat_response_rejects_no_choices() {
        let raw = r#"{"choices":[]}"#;
        let err = parse_chat_response(raw, "m", "v1").unwrap_err();
        assert!(matches!(err, CognitionError::InvalidResponse(_)));
    }

    #[test]
    fn strip_code_fence_idempotent_on_plain_json() {
        let plain = r#"{"queue":"inbox:triage","confidence":0.0,"rationale":"x"}"#;
        assert_eq!(strip_code_fence(plain), plain);
    }
}
