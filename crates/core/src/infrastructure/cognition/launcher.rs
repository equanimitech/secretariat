//! [`CognitionLaunching`] adapter — builds a [`LaunchPlan`] from the
//! principal's `CognitionPrefs::launch_*` fields.
//!
//! Today the only shipped substrate is Claude Code (`claude`), but the
//! adapter is substrate-agnostic by design: anything the principal can
//! launch as a CLI works, and routing to an alternative endpoint (LM
//! Studio's OpenAI-compatible server, a sovereign Ollama box) is
//! expressed entirely in config:
//!
//! ```toml
//! [cognition]
//! launch_command = "claude"
//! launch_args = ["--model", "openai/gpt-oss-20b"]
//!
//! [cognition.launch_env]
//! ANTHROPIC_BASE_URL = "http://localhost:1234"
//! ANTHROPIC_AUTH_TOKEN = "lmstudio"
//! ```
//!
//! That config block produces a plan equivalent to:
//!
//! ```text
//! ANTHROPIC_BASE_URL=http://localhost:1234 \
//! ANTHROPIC_AUTH_TOKEN=lmstudio \
//!   claude --model openai/gpt-oss-20b
//! ```
//!
//! See `docs/developer/launch.md` and
//! https://lmstudio.ai/docs/integrations/claude-code for the upstream
//! recipe.

use std::collections::BTreeMap;
use std::path::Path;

use crate::domain::ChannelBinding;
use crate::infrastructure::preferences::CognitionPrefs;
use crate::ports::{CognitionLaunching, LaunchPlan, LauncherError};

#[derive(Debug, Clone)]
pub struct PrefsLauncher {
    command: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

impl PrefsLauncher {
    /// Build from preferences alone — no channel override. Trims the
    /// command but otherwise leaves the caller's args/env unmolested.
    pub fn from_prefs(prefs: &CognitionPrefs) -> Self {
        Self {
            command: prefs.launch_command.trim().to_string(),
            args: prefs.launch_args.clone(),
            env: prefs.launch_env.clone(),
        }
    }

    /// Build from preferences with per-channel overrides layered on top.
    ///
    /// Resolution rules:
    /// - `launch_command`: binding wins when `Some`, else prefs.
    /// - `launch_args`: binding wins when non-empty (replaces, not
    ///   appends — args are a substrate contract, not additive).
    /// - `launch_env`: per-key merge, binding wins on conflicts. Keys
    ///   only in prefs survive (e.g. workspace-wide proxy var); keys
    ///   only in binding land as-is (e.g. journals' LM Studio routing).
    pub fn from_prefs_with_binding(prefs: &CognitionPrefs, binding: &ChannelBinding) -> Self {
        let command = match binding.launch_command.as_deref() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => prefs.launch_command.trim().to_string(),
        };
        let args = if binding.launch_args.is_empty() {
            prefs.launch_args.clone()
        } else {
            binding.launch_args.clone()
        };
        let mut env = prefs.launch_env.clone();
        for (k, v) in &binding.launch_env {
            env.insert(k.clone(), v.clone());
        }
        Self { command, args, env }
    }
}

impl CognitionLaunching for PrefsLauncher {
    fn plan_launch(&self, cwd: &Path) -> Result<LaunchPlan, LauncherError> {
        if self.command.is_empty() {
            return Err(LauncherError::EmptyCommand);
        }
        Ok(LaunchPlan {
            command: self.command.clone(),
            args: self.args.clone(),
            cwd: cwd.to_path_buf(),
            env: self.env.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_prefs_launch_with_claude() {
        let prefs = CognitionPrefs::default();
        let l = PrefsLauncher::from_prefs(&prefs);
        let tmp = TempDir::new().unwrap();
        let plan = l.plan_launch(tmp.path()).unwrap();
        assert_eq!(plan.command, "claude");
        assert!(plan.args.is_empty());
        assert!(plan.env.is_empty());
        assert_eq!(plan.cwd, tmp.path());
    }

    #[test]
    fn lm_studio_recipe_produces_full_env_and_model_args() {
        let prefs = CognitionPrefs {
            launch_args: vec!["--model".into(), "openai/gpt-oss-20b".into()],
            launch_env: [
                (
                    "ANTHROPIC_BASE_URL".to_string(),
                    "http://localhost:1234".to_string(),
                ),
                ("ANTHROPIC_AUTH_TOKEN".to_string(), "lmstudio".to_string()),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let l = PrefsLauncher::from_prefs(&prefs);
        let tmp = TempDir::new().unwrap();
        let plan = l.plan_launch(tmp.path()).unwrap();
        assert_eq!(plan.command, "claude");
        assert_eq!(plan.args, vec!["--model", "openai/gpt-oss-20b"]);
        assert_eq!(
            plan.env.get("ANTHROPIC_BASE_URL").map(String::as_str),
            Some("http://localhost:1234")
        );
        assert_eq!(
            plan.env.get("ANTHROPIC_AUTH_TOKEN").map(String::as_str),
            Some("lmstudio")
        );
    }

    #[test]
    fn empty_command_errors() {
        let prefs = CognitionPrefs {
            launch_command: "".into(),
            ..Default::default()
        };
        let l = PrefsLauncher::from_prefs(&prefs);
        let tmp = TempDir::new().unwrap();
        assert!(matches!(
            l.plan_launch(tmp.path()),
            Err(LauncherError::EmptyCommand)
        ));
    }

    #[test]
    fn whitespace_only_command_errors() {
        let prefs = CognitionPrefs {
            launch_command: "   ".into(),
            ..Default::default()
        };
        let l = PrefsLauncher::from_prefs(&prefs);
        let tmp = TempDir::new().unwrap();
        assert!(matches!(
            l.plan_launch(tmp.path()),
            Err(LauncherError::EmptyCommand)
        ));
    }

    #[test]
    fn binding_command_overrides_prefs() {
        let prefs = CognitionPrefs {
            launch_command: "claude".into(),
            ..Default::default()
        };
        let binding = ChannelBinding {
            launch_command: Some("/usr/local/bin/journal-claude".into()),
            ..ChannelBinding::empty()
        };
        let l = PrefsLauncher::from_prefs_with_binding(&prefs, &binding);
        let tmp = TempDir::new().unwrap();
        let plan = l.plan_launch(tmp.path()).unwrap();
        assert_eq!(plan.command, "/usr/local/bin/journal-claude");
    }

    #[test]
    fn binding_args_replace_prefs_when_present() {
        let prefs = CognitionPrefs {
            launch_args: vec!["--default-flag".into()],
            ..Default::default()
        };
        let binding = ChannelBinding {
            launch_args: vec!["--model".into(), "openai/gpt-oss-20b".into()],
            ..ChannelBinding::empty()
        };
        let l = PrefsLauncher::from_prefs_with_binding(&prefs, &binding);
        let tmp = TempDir::new().unwrap();
        let plan = l.plan_launch(tmp.path()).unwrap();
        assert_eq!(plan.args, vec!["--model", "openai/gpt-oss-20b"]);
    }

    #[test]
    fn empty_binding_args_inherits_prefs() {
        let prefs = CognitionPrefs {
            launch_args: vec!["--default-flag".into()],
            ..Default::default()
        };
        let binding = ChannelBinding::empty();
        let l = PrefsLauncher::from_prefs_with_binding(&prefs, &binding);
        let tmp = TempDir::new().unwrap();
        let plan = l.plan_launch(tmp.path()).unwrap();
        assert_eq!(plan.args, vec!["--default-flag"]);
    }

    #[test]
    fn binding_env_merges_with_prefs_env() {
        let prefs = CognitionPrefs {
            launch_env: [
                ("HTTP_PROXY".to_string(), "x".to_string()),
                ("SHARED".to_string(), "from-prefs".to_string()),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        let binding = ChannelBinding {
            launch_env: [
                (
                    "ANTHROPIC_BASE_URL".to_string(),
                    "http://localhost:1234".to_string(),
                ),
                ("SHARED".to_string(), "from-binding".to_string()),
            ]
            .into_iter()
            .collect(),
            ..ChannelBinding::empty()
        };
        let l = PrefsLauncher::from_prefs_with_binding(&prefs, &binding);
        let tmp = TempDir::new().unwrap();
        let plan = l.plan_launch(tmp.path()).unwrap();
        assert_eq!(plan.env.get("HTTP_PROXY").map(String::as_str), Some("x"));
        assert_eq!(
            plan.env.get("ANTHROPIC_BASE_URL").map(String::as_str),
            Some("http://localhost:1234")
        );
        // Binding wins on collision.
        assert_eq!(plan.env.get("SHARED").map(String::as_str), Some("from-binding"));
    }
}
