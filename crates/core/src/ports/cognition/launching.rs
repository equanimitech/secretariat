//! External-terminal launching cognition.
//!
//! Plans how to start the principal's chosen interactive cognition CLI
//! inside a channel-bound cwd. Pure planning — the use case returns the
//! plan and the host (CLI exec, MCP `launch_channel`) decides whether
//! to replace the process, spawn-detach, or hand it to a terminal.
//! Substrate-agnostic by design: today's only adapter wraps Claude Code
//! (`claude`), but the same shape covers a future LM Studio CLI, Ollama
//! wrapper, or BYOK runner without touching application code. See
//! `docs/developer/launch.md`.
//!
//! Sibling port [`CognitionSession`](super::session::CognitionSession)
//! covers the *in-process* streaming-chat surface (Tauri tab-strip);
//! this port covers the *external-terminal* launch surface (`sec launch`).
//! Different lifecycles, different responsibilities, different ports.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    /// Executable to start, resolved against `$PATH` or an absolute path.
    pub command: String,
    /// Args passed after `command`. Concatenated from the adapter's
    /// base args and any user-configured `launch_args`.
    pub args: Vec<String>,
    /// Working directory for the launched process — typically the
    /// channel-dir, possibly remapped via [`crate::domain::ChannelBinding`].
    pub cwd: PathBuf,
    /// Env overrides layered on top of the parent process env. Empty
    /// when running against the principal's default cognition (e.g.
    /// Claude API). Populated when routing to LM Studio or another
    /// OpenAI-compatible endpoint via `ANTHROPIC_BASE_URL` etc.
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("launch_command is empty in preferences")]
    EmptyCommand,
}

pub trait CognitionLaunching: Send + Sync {
    /// Build a [`LaunchPlan`] describing how to start an interactive
    /// session rooted at `cwd`. Pure — no process is spawned here.
    fn plan_launch(&self, cwd: &Path) -> Result<LaunchPlan, LauncherError>;
}
