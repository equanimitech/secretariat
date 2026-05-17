//! Cognition adapters + audit ledger.
//!
//! Implements `ports::CognitionPort` for concrete substrates and stores
//! the per-decision audit trail. Domain + application import only the
//! port; everything substrate-specific lives here so swapping the brain
//! is a one-file change.
//!
//! Today we ship two adapters:
//! - [`ClaudeCognitionAdapter`] — Anthropic Messages API, BYOK.
//! - [`OpenAICompatibleAdapter`] — anything speaking the OpenAI
//!   Chat-Completions shape (Ollama for sovereign / on-device, plus
//!   OpenRouter / OpenAI / Together / Groq for cloud variants).
//!
//! [`AnyCognitionAdapter`] dispatches between them based on the
//! `provider` field in the config file.

pub mod claude;
pub mod config;
pub mod launcher;
pub mod ledger;
pub mod openai_compat;

pub use claude::{ClaudeCognitionAdapter, KNOWN_ANTHROPIC_MODELS};
pub use config::{
    load_config, save_config, CognitionConfig, CognitionConfigError, Provider,
};
pub use launcher::PrefsLauncher;
pub use ledger::{append_entry, read_entries, LedgerEntry, LedgerError};
pub use openai_compat::OpenAICompatibleAdapter;

use std::path::Path;

use crate::domain::QueueHandle;
use crate::infrastructure::preferences::{CognitionPrefs, CognitionProvider};
use crate::ports::{CognitionError, CognitionPort, RouteSuggestion};

/// Enum-dispatched cognition adapter. Built by `try_load`, picks the
/// concrete implementation based on `CognitionConfig::provider`.
///
/// Enum dispatch (rather than `Box<dyn CognitionPort>`) because the
/// trait uses native `async fn`, which is not dyn-compatible without
/// `async-trait`. Two variants is fine; if we add a third we revisit.
#[derive(Debug, Clone)]
pub enum AnyCognitionAdapter {
    Anthropic(ClaudeCognitionAdapter),
    OpenAiCompat(OpenAICompatibleAdapter),
}

impl AnyCognitionAdapter {
    /// Build from a freshly-loaded config. Returns `None` when the
    /// chosen provider's required fields are missing — caller treats
    /// as "feature off."
    pub fn from_config(config: CognitionConfig) -> Option<Self> {
        match config.provider {
            Provider::Anthropic => ClaudeCognitionAdapter::from_config(config).map(Self::Anthropic),
            Provider::OpenAiCompat => {
                OpenAICompatibleAdapter::from_config(config).map(Self::OpenAiCompat)
            }
        }
    }

    /// Convenience: load the config and build the adapter in one step.
    /// Returns `Ok(None)` for both "no config file" and "config file
    /// present but required fields missing" — both mean feature off.
    pub fn try_load(config_path: &Path) -> Result<Option<Self>, CognitionConfigError> {
        let Some(config) = load_config(config_path)? else {
            return Ok(None);
        };
        Ok(Self::from_config(config))
    }

    /// Build from the unified `CognitionPrefs` (the new `preferences.toml`
    /// section). Returns `None` when the provider's required fields are
    /// missing — caller treats as "feature off."
    pub fn from_prefs(prefs: &CognitionPrefs) -> Option<Self> {
        let config = CognitionConfig {
            provider: match prefs.provider {
                CognitionProvider::Anthropic => Provider::Anthropic,
                CognitionProvider::OpenaiCompat => Provider::OpenAiCompat,
            },
            api_key: prefs.api_key.clone(),
            api_base: prefs.api_base.clone(),
            model: prefs.model.clone(),
            route_threshold: prefs.route_threshold,
        };
        Self::from_config(config)
    }

    /// Threshold the use case applies to suggestions before re-filing.
    /// Reads from the underlying config so the picker can change it
    /// without the use case re-loading.
    pub fn threshold(&self) -> f32 {
        match self {
            Self::Anthropic(a) => a.config().threshold_or_default(),
            Self::OpenAiCompat(a) => a.config().threshold_or_default(),
        }
    }

    /// Identifier of the underlying model — purely for surfaces that
    /// want to label the adapter (settings pane, ledger).
    pub fn model_id(&self) -> &str {
        match self {
            Self::Anthropic(a) => a.model_or_default(),
            Self::OpenAiCompat(a) => a.model_or_default(),
        }
    }

    /// Catalog of model identifiers the principal can pick from.
    /// Anthropic returns a hand-curated list (no public no-auth
    /// endpoint); OpenAI-compat hits `/models` on the configured base.
    pub async fn list_models(&self) -> Result<Vec<String>, CognitionError> {
        match self {
            Self::Anthropic(_) => {
                Ok(KNOWN_ANTHROPIC_MODELS.iter().map(|s| s.to_string()).collect())
            }
            Self::OpenAiCompat(a) => a.list_models().await,
        }
    }
}

impl CognitionPort for AnyCognitionAdapter {
    async fn route_capture(
        &self,
        body: &str,
        existing_queues: &[QueueHandle],
    ) -> Result<RouteSuggestion, CognitionError> {
        match self {
            Self::Anthropic(a) => a.route_capture(body, existing_queues).await,
            Self::OpenAiCompat(a) => a.route_capture(body, existing_queues).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn anthropic_config_builds_anthropic_adapter() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("c.json");
        std::fs::write(&path, r#"{"provider":"anthropic","api_key":"sk-ant"}"#).unwrap();
        let adapter = AnyCognitionAdapter::try_load(&path).unwrap().unwrap();
        assert!(matches!(adapter, AnyCognitionAdapter::Anthropic(_)));
    }

    #[test]
    fn openai_compat_config_builds_openai_adapter() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("c.json");
        std::fs::write(
            &path,
            r#"{"provider":"openai-compat","api_base":"http://localhost:11434/v1","model":"llama3.1"}"#,
        )
        .unwrap();
        let adapter = AnyCognitionAdapter::try_load(&path).unwrap().unwrap();
        assert!(matches!(adapter, AnyCognitionAdapter::OpenAiCompat(_)));
        assert_eq!(adapter.model_id(), "llama3.1");
    }

    #[test]
    fn missing_required_field_yields_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("c.json");
        // anthropic without api_key
        std::fs::write(&path, r#"{"provider":"anthropic"}"#).unwrap();
        assert!(AnyCognitionAdapter::try_load(&path).unwrap().is_none());

        // openai-compat without api_base
        std::fs::write(&path, r#"{"provider":"openai-compat"}"#).unwrap();
        assert!(AnyCognitionAdapter::try_load(&path).unwrap().is_none());
    }
}
