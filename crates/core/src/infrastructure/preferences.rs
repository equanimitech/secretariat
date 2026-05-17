//! Principal preferences — a single `~/.secretariat/preferences.toml` that
//! consolidates composition, cognition, and delivery settings.
//!
//! ## Migration
//!
//! Earlier versions stored these in separate files:
//! - `~/.secretariat/cognition.json` — cognition substrate config
//! - `~/.secretariat/cadence.toml`   — delivery cadence
//!
//! On first load, if either legacy file is present and `preferences.toml`
//! does not exist yet, we read the legacy files, merge into a new
//! `preferences.toml`, and delete the legacy files. After migration the
//! legacy files are gone; callers should use this module exclusively.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum PreferencesError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed preferences.toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("malformed legacy cognition.json: {0}")]
    LegacyCognitionJson(#[from] serde_json::Error),
    #[error("malformed legacy cadence.toml: {0}")]
    LegacyCadenceToml(toml::de::Error),
    #[error("serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("poll_interval_minutes = {got} is below the minimum 15")]
    PollIntervalBelowMinimum { got: u32 },
}

// ---------------------------------------------------------------------------
// Sub-sections
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CompositionPrefs {
    /// The line appended at the end of every envelope body. Empty = none.
    /// AGENTS.md rule #9: the principal owns the closing line; Claude never
    /// auto-appends.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub closing_line: String,

    /// Free-form style guidance Claude reads when composing. Tone, language,
    /// persona hints. Empty = no extra guidance.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style_notes: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CognitionProvider {
    #[default]
    Anthropic,
    /// Any OpenAI Chat-Completions compatible endpoint (Ollama, OpenRouter, …).
    #[serde(alias = "openai-compatible", alias = "openai")]
    OpenaiCompat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CognitionPrefs {
    /// Selected substrate. Defaults to `anthropic` when missing.
    #[serde(default)]
    pub provider: CognitionProvider,

    /// API key. Required for anthropic + hosted openai endpoints; optional
    /// for local endpoints (Ollama).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// API base URL. Required for openai-compat. Optional for anthropic
    /// (defaults to `https://api.anthropic.com`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,

    /// Model identifier passed to the substrate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Confidence threshold below which contextification routing decisions
    /// are not applied. Defaults to 0.7 when missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_threshold: Option<f32>,

    /// Command resolved by `sec launch` to start the principal's chosen
    /// interactive cognition CLI in a channel-bound cwd. Defaults to
    /// `claude` (Claude Code). Override to swap substrates without
    /// touching the use case — e.g. a wrapper script that points
    /// `claude` at a local LM Studio endpoint, or a different CLI
    /// entirely.
    #[serde(default = "default_launch_command")]
    pub launch_command: String,

    /// Args appended after `launch_command`. Examples:
    /// - `["--model", "openai/gpt-oss-20b"]` for LM Studio routing.
    /// - `["--dangerously-skip-permissions"]` for sandboxed sub-orgs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub launch_args: Vec<String>,

    /// Env vars layered on top of the parent process env at launch
    /// time. Canonical use: point Claude Code at an OpenAI-compatible
    /// endpoint hosted by LM Studio:
    ///
    /// ```toml
    /// [cognition.launch_env]
    /// ANTHROPIC_BASE_URL = "http://localhost:1234"
    /// ANTHROPIC_AUTH_TOKEN = "lmstudio"
    /// ```
    ///
    /// See `docs/developer/launch.md` for the full LM Studio recipe.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub launch_env: BTreeMap<String, String>,
}

fn default_launch_command() -> String {
    "claude".to_string()
}

impl Default for CognitionPrefs {
    fn default() -> Self {
        Self {
            provider: CognitionProvider::default(),
            api_key: None,
            api_base: None,
            model: None,
            route_threshold: None,
            launch_command: default_launch_command(),
            launch_args: Vec::new(),
            launch_env: BTreeMap::new(),
        }
    }
}

impl CognitionPrefs {
    pub fn threshold_or_default(&self) -> f32 {
        self.route_threshold.unwrap_or(0.7)
    }

    pub fn is_configured(&self) -> bool {
        self.api_key.is_some() || self.api_base.is_some()
    }
}

const DEFAULT_POLL_INTERVAL_MINUTES: u32 = 15;
const MIN_POLL_INTERVAL_MINUTES: u32 = 15;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliveryPrefs {
    /// Minutes between relay polls. Floored at 15 (anti-compulsion default).
    #[serde(default = "default_poll_interval")]
    pub poll_interval_minutes: u32,
}

fn default_poll_interval() -> u32 {
    DEFAULT_POLL_INTERVAL_MINUTES
}

impl Default for DeliveryPrefs {
    fn default() -> Self {
        Self {
            poll_interval_minutes: DEFAULT_POLL_INTERVAL_MINUTES,
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Preferences {
    #[serde(default)]
    pub composition: CompositionPrefs,
    #[serde(default)]
    pub cognition: CognitionPrefs,
    #[serde(default)]
    pub delivery: DeliveryPrefs,
}

impl Preferences {
    /// Load from `preferences.toml`. If the file is absent, returns defaults.
    /// Does NOT perform migration — call `load_or_migrate` for that.
    pub fn load(path: &Path) -> Result<Self, PreferencesError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path).map_err(|e| PreferencesError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let prefs: Self = toml::from_str(&raw)?;
        prefs.validate()?;
        Ok(prefs)
    }

    pub fn save(&self, path: &Path) -> Result<(), PreferencesError> {
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(path, raw).map_err(|e| PreferencesError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        Ok(())
    }

    fn validate(&self) -> Result<(), PreferencesError> {
        if self.delivery.poll_interval_minutes < MIN_POLL_INTERVAL_MINUTES {
            return Err(PreferencesError::PollIntervalBelowMinimum {
                got: self.delivery.poll_interval_minutes,
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Migration
// ---------------------------------------------------------------------------

/// Load preferences, migrating from legacy `cognition.json` + `cadence.toml`
/// if `preferences.toml` doesn't exist yet. After migration the legacy files
/// are deleted.
pub fn load_or_migrate(
    preferences_path: &Path,
    legacy_cognition: &Path,
    legacy_cadence: &Path,
) -> Result<Preferences, PreferencesError> {
    if preferences_path.exists() {
        return Preferences::load(preferences_path);
    }

    let mut prefs = Preferences::default();

    // Migrate cognition.json.
    if legacy_cognition.exists() {
        let raw = std::fs::read_to_string(legacy_cognition).map_err(|e| PreferencesError::Io {
            path: legacy_cognition.to_path_buf(),
            source: e,
        })?;
        #[derive(serde::Deserialize)]
        struct LegacyCognition {
            #[serde(default)]
            provider: Option<String>,
            api_key: Option<String>,
            api_base: Option<String>,
            model: Option<String>,
            route_threshold: Option<f32>,
        }
        let legacy: LegacyCognition = serde_json::from_str(&raw)?;
        prefs.cognition.provider = match legacy.provider.as_deref() {
            Some("openai-compat") | Some("openai-compatible") | Some("openai") => {
                CognitionProvider::OpenaiCompat
            }
            _ => CognitionProvider::Anthropic,
        };
        prefs.cognition.api_key = legacy.api_key;
        prefs.cognition.api_base = legacy.api_base;
        prefs.cognition.model = legacy.model;
        prefs.cognition.route_threshold = legacy.route_threshold;
    }

    // Migrate cadence.toml.
    if legacy_cadence.exists() {
        let raw = std::fs::read_to_string(legacy_cadence).map_err(|e| PreferencesError::Io {
            path: legacy_cadence.to_path_buf(),
            source: e,
        })?;
        #[derive(serde::Deserialize)]
        struct LegacyCadence {
            poll_interval_minutes: Option<u32>,
        }
        let legacy: LegacyCadence =
            toml::from_str(&raw).map_err(PreferencesError::LegacyCadenceToml)?;
        if let Some(n) = legacy.poll_interval_minutes {
            prefs.delivery.poll_interval_minutes = n;
        }
    }

    prefs.validate()?;
    prefs.save(preferences_path)?;

    // Delete legacy files — best effort (don't fail if already gone).
    let _ = std::fs::remove_file(legacy_cognition);
    let _ = std::fs::remove_file(legacy_cadence);

    Ok(prefs)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn missing_file_yields_defaults() {
        let d = dir();
        let p = Preferences::load(&d.path().join("preferences.toml")).unwrap();
        assert_eq!(p, Preferences::default());
    }

    #[test]
    fn launch_command_defaults_to_claude() {
        let p = Preferences::default();
        assert_eq!(p.cognition.launch_command, "claude");
        assert!(p.cognition.launch_args.is_empty());
        assert!(p.cognition.launch_env.is_empty());
    }

    #[test]
    fn launch_fields_round_trip_via_toml() {
        let d = dir();
        let path = d.path().join("preferences.toml");
        let mut prefs = Preferences::default();
        prefs.cognition.launch_command = "/usr/local/bin/claude".into();
        prefs.cognition.launch_args = vec!["--model".into(), "openai/gpt-oss-20b".into()];
        prefs
            .cognition
            .launch_env
            .insert("ANTHROPIC_BASE_URL".into(), "http://localhost:1234".into());
        prefs
            .cognition
            .launch_env
            .insert("ANTHROPIC_AUTH_TOKEN".into(), "lmstudio".into());
        prefs.save(&path).unwrap();
        let loaded = Preferences::load(&path).unwrap();
        assert_eq!(loaded, prefs);
    }

    #[test]
    fn omitted_launch_fields_deserialize_to_defaults() {
        let d = dir();
        let path = d.path().join("preferences.toml");
        // Older preferences.toml with no [cognition] launch_* keys.
        std::fs::write(&path, "[cognition]\napi_key = \"sk-test\"\n").unwrap();
        let loaded = Preferences::load(&path).unwrap();
        assert_eq!(loaded.cognition.launch_command, "claude");
        assert!(loaded.cognition.launch_args.is_empty());
        assert!(loaded.cognition.launch_env.is_empty());
    }

    #[test]
    fn roundtrip_save_load() {
        let d = dir();
        let path = d.path().join("preferences.toml");
        let mut prefs = Preferences::default();
        prefs.composition.closing_line = "_Drafted by AI._".into();
        prefs.composition.style_notes = "Formal.".into();
        prefs.cognition.api_key = Some("sk-test".into());
        prefs.delivery.poll_interval_minutes = 30;
        prefs.save(&path).unwrap();
        let loaded = Preferences::load(&path).unwrap();
        assert_eq!(prefs, loaded);
    }

    #[test]
    fn rejects_poll_interval_below_minimum() {
        let d = dir();
        let path = d.path().join("preferences.toml");
        std::fs::write(&path, "[delivery]\npoll_interval_minutes = 5\n").unwrap();
        assert!(matches!(
            Preferences::load(&path),
            Err(PreferencesError::PollIntervalBelowMinimum { got: 5 })
        ));
    }

    #[test]
    fn migration_reads_cognition_json_and_cadence_toml() {
        let d = dir();
        let prefs_path = d.path().join("preferences.toml");
        let cog_path = d.path().join("cognition.json");
        let cad_path = d.path().join("cadence.toml");

        std::fs::write(
            &cog_path,
            r#"{"provider":"openai-compat","api_base":"http://localhost:11434/v1","model":"llama3"}"#,
        ).unwrap();
        std::fs::write(&cad_path, "poll_interval_minutes = 60\n").unwrap();

        let prefs = load_or_migrate(&prefs_path, &cog_path, &cad_path).unwrap();

        assert_eq!(prefs.cognition.provider, CognitionProvider::OpenaiCompat);
        assert_eq!(
            prefs.cognition.api_base.as_deref(),
            Some("http://localhost:11434/v1")
        );
        assert_eq!(prefs.cognition.model.as_deref(), Some("llama3"));
        assert_eq!(prefs.delivery.poll_interval_minutes, 60);

        // Legacy files deleted.
        assert!(!cog_path.exists());
        assert!(!cad_path.exists());

        // preferences.toml written.
        assert!(prefs_path.exists());
    }

    #[test]
    fn migration_skips_missing_legacy_files() {
        let d = dir();
        let prefs_path = d.path().join("preferences.toml");
        let prefs =
            load_or_migrate(&prefs_path, &d.path().join("nope.json"), &d.path().join("nope.toml"))
                .unwrap();
        assert_eq!(prefs, Preferences::default());
    }

    #[test]
    fn load_or_migrate_uses_existing_prefs_file() {
        let d = dir();
        let prefs_path = d.path().join("preferences.toml");
        std::fs::write(&prefs_path, "[composition]\nclosing_line = \"hi\"\n").unwrap();
        // Even if legacy files also present, existing prefs wins.
        let cog = d.path().join("cognition.json");
        std::fs::write(&cog, r#"{"api_key":"sk-other"}"#).unwrap();
        let prefs =
            load_or_migrate(&prefs_path, &cog, &d.path().join("cadence.toml")).unwrap();
        assert_eq!(prefs.composition.closing_line, "hi");
        assert!(prefs.cognition.api_key.is_none());
        // Legacy file NOT deleted (we didn't migrate).
        assert!(cog.exists());
    }
}
