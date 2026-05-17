//! [`CognitionLauncher`] adapter — builds a [`LaunchPlan`] from the
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

use crate::infrastructure::preferences::CognitionPrefs;
use crate::ports::{CognitionLauncher, LaunchPlan, LauncherError};

#[derive(Debug, Clone)]
pub struct PrefsLauncher {
    command: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

impl PrefsLauncher {
    /// Build from preferences. Trims the command but otherwise leaves
    /// the caller's args/env unmolested — preferences are the source
    /// of truth.
    pub fn from_prefs(prefs: &CognitionPrefs) -> Self {
        Self {
            command: prefs.launch_command.trim().to_string(),
            args: prefs.launch_args.clone(),
            env: prefs.launch_env.clone(),
        }
    }
}

impl CognitionLauncher for PrefsLauncher {
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
        let mut prefs = CognitionPrefs::default();
        prefs.launch_args = vec!["--model".into(), "openai/gpt-oss-20b".into()];
        prefs
            .launch_env
            .insert("ANTHROPIC_BASE_URL".into(), "http://localhost:1234".into());
        prefs
            .launch_env
            .insert("ANTHROPIC_AUTH_TOKEN".into(), "lmstudio".into());
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
        let mut prefs = CognitionPrefs::default();
        prefs.launch_command = "".into();
        let l = PrefsLauncher::from_prefs(&prefs);
        let tmp = TempDir::new().unwrap();
        assert!(matches!(
            l.plan_launch(tmp.path()),
            Err(LauncherError::EmptyCommand)
        ));
    }

    #[test]
    fn whitespace_only_command_errors() {
        let mut prefs = CognitionPrefs::default();
        prefs.launch_command = "   ".into();
        let l = PrefsLauncher::from_prefs(&prefs);
        let tmp = TempDir::new().unwrap();
        assert!(matches!(
            l.plan_launch(tmp.path()),
            Err(LauncherError::EmptyCommand)
        ));
    }
}
