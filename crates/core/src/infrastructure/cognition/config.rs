//! Cognition configuration — single source of truth for which substrate
//! is wired and how. Loaded from `~/.secretariat/cognition.json`.
//!
//! Default-off discipline: missing file = no adapter = no API calls.
//! The `provider` field selects which adapter handles the call. Other
//! fields are interpreted per-provider:
//!
//! Provider semantics:
//!
//! - `anthropic` (default): `api_key` + optional `model`/`api_base`, used against the Anthropic Messages API.
//! - `openai-compat`: `api_base` (required) + optional `api_key`/`model`, used against any OpenAI Chat-Completions endpoint (Ollama, OpenRouter, OpenAI, Together, Groq, …).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CognitionConfigError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed cognition config: {0}")]
    Malformed(#[from] serde_json::Error),
}

/// Which cognition substrate is wired. The variant determines which
/// adapter handles `route_capture` calls.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Provider {
    /// Anthropic Messages API (BYOK).
    #[serde(rename = "anthropic")]
    Anthropic,
    /// Any OpenAI Chat-Completions compatible endpoint — Ollama,
    /// OpenRouter, OpenAI itself, Together, Groq, …
    #[serde(
        rename = "openai-compat",
        alias = "openai-compatible",
        alias = "openai"
    )]
    OpenAiCompat,
}

impl Default for Provider {
    fn default() -> Self {
        Self::Anthropic
    }
}

/// Persisted shape of `~/.secretariat/cognition.json`. All fields are
/// optional except where the chosen provider demands one (e.g.
/// openai-compat needs `api_base`); the adapter validates that at
/// construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitionConfig {
    /// Selected substrate. Defaults to `anthropic` when the field is
    /// missing — keeps existing `~/.secretariat/cognition.json` files
    /// (anthropic-shaped) working without migration.
    #[serde(default)]
    pub provider: Provider,

    /// API key. Required by anthropic + openai (hosted) endpoints,
    /// optional for Ollama.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// API base URL. Required for openai-compat (no sensible default
    /// across Ollama / OpenRouter / OpenAI). Optional for anthropic
    /// (defaults to `https://api.anthropic.com`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,

    /// Model identifier passed to the substrate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Confidence threshold below which routing decisions don't apply.
    /// Defaults to 0.7 when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_threshold: Option<f32>,
}

impl CognitionConfig {
    pub fn threshold_or_default(&self) -> f32 {
        self.route_threshold.unwrap_or(0.7)
    }
}

/// Load the config file. Returns `Ok(None)` when the file does not
/// exist — that is the explicit "feature off" state and never an error.
pub fn load_config(path: &Path) -> Result<Option<CognitionConfig>, CognitionConfigError> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path).map_err(|e| CognitionConfigError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let cfg: CognitionConfig = serde_json::from_str(&raw)?;
    Ok(Some(cfg))
}

/// Persist the config. Creates the parent directory on first save.
/// Writes pretty-printed JSON so the principal can hand-edit comfortably.
pub fn save_config(path: &Path, config: &CognitionConfig) -> Result<(), CognitionConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CognitionConfigError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let raw = serde_json::to_string_pretty(config)?;
    std::fs::write(path, raw).map_err(|e| CognitionConfigError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_file_yields_none() {
        let dir = TempDir::new().unwrap();
        assert!(load_config(&dir.path().join("nope.json"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn provider_defaults_to_anthropic_on_missing_field() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("c.json");
        std::fs::write(&path, r#"{"api_key":"sk-test"}"#).unwrap();
        let cfg = load_config(&path).unwrap().unwrap();
        assert_eq!(cfg.provider, Provider::Anthropic);
        assert_eq!(cfg.api_key.as_deref(), Some("sk-test"));
    }

    #[test]
    fn openai_compat_provider_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("c.json");
        let original = CognitionConfig {
            provider: Provider::OpenAiCompat,
            api_key: None,
            api_base: Some("http://localhost:11434/v1".into()),
            model: Some("llama3.1:8b".into()),
            route_threshold: Some(0.65),
        };
        save_config(&path, &original).unwrap();
        let loaded = load_config(&path).unwrap().unwrap();
        assert_eq!(loaded.provider, Provider::OpenAiCompat);
        assert_eq!(
            loaded.api_base.as_deref(),
            Some("http://localhost:11434/v1")
        );
        assert_eq!(loaded.model.as_deref(), Some("llama3.1:8b"));
        assert!((loaded.threshold_or_default() - 0.65).abs() < 0.001);
    }

    #[test]
    fn malformed_config_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("c.json");
        std::fs::write(&path, "{not json}").unwrap();
        assert!(matches!(
            load_config(&path),
            Err(CognitionConfigError::Malformed(_))
        ));
    }

    #[test]
    fn save_creates_parent_dir() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a/b/cognition.json");
        save_config(
            &path,
            &CognitionConfig {
                provider: Provider::Anthropic,
                api_key: Some("k".into()),
                api_base: None,
                model: None,
                route_threshold: None,
            },
        )
        .unwrap();
        assert!(path.exists());
    }

    #[test]
    fn alias_spellings_for_provider() {
        // Don't care which spelling the principal types — they all map
        // to the same variant.
        for raw in [
            r#"{"provider":"openai-compat"}"#,
            r#"{"provider":"openai-compatible"}"#,
            r#"{"provider":"openai"}"#,
        ] {
            let cfg: CognitionConfig = serde_json::from_str(raw).unwrap();
            assert_eq!(cfg.provider, Provider::OpenAiCompat);
        }
    }
}
