//! `launch_channel` — resolve a channel-dir's bound cwd and build a
//! [`LaunchPlan`] the CLI / MCP host then executes.
//!
//! Pure orchestration: no subprocess is spawned here. The CLI uses
//! `exec` (process replacement); a future MCP `launch_channel` tool
//! returns the plan so the host can open a terminal or spawn-detach
//! according to its own conventions.
//!
//! See `docs/developer/launch.md` and
//! `docs/pitches/2026-05-13-launch-dispatch-root-path.md`.

use std::path::Path;

use thiserror::Error;

use crate::domain::{ChannelBinding, QueueHandle};
use crate::infrastructure::binding_store::BindingStoreError;
use crate::infrastructure::channel_def_store::channel_dir;
use crate::infrastructure::{
    channel_def_path, load_channel_binding, resolve_channel_path, ChannelDefStoreError,
};
use crate::ports::{CognitionLauncher, LaunchPlan, LauncherError};

#[derive(Debug, Error)]
pub enum LaunchChannelError {
    #[error("handle `{0}` is not a channel handle (must start with `channel:`)")]
    NotAChannelHandle(String),
    #[error(
        "channel `{0}` does not exist — create it with `sec channels create {0}` (or check --org)"
    )]
    ChannelNotFound(String),
    #[error(transparent)]
    Binding(#[from] BindingStoreError),
    #[error(transparent)]
    ChannelDef(#[from] ChannelDefStoreError),
    #[error(transparent)]
    Launcher(#[from] LauncherError),
}

/// Plan an interactive launch in a channel's cwd. Honors `root_path`
/// from `<channel-dir>/contract.local.md` when set; falls back to the
/// substrate's default channel-dir otherwise. Also returns the loaded
/// [`ChannelBinding`] so callers can apply per-channel cognition
/// overrides (`launch_command` / `launch_args` / `launch_env`) when
/// constructing the launcher — see
/// [`crate::infrastructure::PrefsLauncher::from_prefs_with_binding`].
///
/// `channels_root` is the principal's channels root for the relevant
/// scope: `paths.channels` for personal channels, or
/// `org_channels_root(paths.orgs_root, alias)` for an org channel.
///
/// Errors when `handle` isn't a `channel:...` handle or when the
/// channel doesn't have a `channel.md` (or legacy `.channelDef`) at the
/// resolved path — same existence gate as capture/contract verbs.
pub fn launch_channel(
    channels_root: &Path,
    handle: &QueueHandle,
    launcher: &dyn CognitionLauncher,
) -> Result<LaunchPlan, LaunchChannelError> {
    Ok(launch_channel_with_binding(channels_root, handle, launcher)?.0)
}

/// Variant that also returns the resolved [`ChannelBinding`]. Use this
/// from the CLI / MCP host when you need to apply per-channel overrides
/// to a launcher built from the base preferences.
pub fn launch_channel_with_binding(
    channels_root: &Path,
    handle: &QueueHandle,
    launcher: &dyn CognitionLauncher,
) -> Result<(LaunchPlan, ChannelBinding), LaunchChannelError> {
    if handle.top_namespace() != "channel" {
        return Err(LaunchChannelError::NotAChannelHandle(
            handle.as_str().to_string(),
        ));
    }

    let default = channel_dir(channels_root, handle);
    let channel_def_file = channel_def_path(channels_root, handle);
    if !channel_def_file.is_file() {
        return Err(LaunchChannelError::ChannelNotFound(
            handle.as_str().to_string(),
        ));
    }

    let binding = load_channel_binding(&default)?;
    let cwd = binding
        .root_path
        .clone()
        .unwrap_or_else(|| default.clone());
    // Sanity: if root_path was set, prefer it even if resolve_channel_path
    // ever diverges from binding.root_path (it shouldn't — keep aligned).
    debug_assert_eq!(cwd, resolve_channel_path(&default)?);
    let plan = launcher.plan_launch(&cwd)?;
    Ok((plan, binding))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::contract_store::CONTRACT_FILENAME;
    use crate::infrastructure::PrefsLauncher;
    use crate::infrastructure::preferences::CognitionPrefs;
    use chrono::Utc;
    use tempfile::TempDir;

    fn make_channel(channels_root: &Path, handle: &QueueHandle) -> std::path::PathBuf {
        let dir = channel_dir(channels_root, handle);
        std::fs::create_dir_all(&dir).unwrap();
        // Minimal valid channelDef — just the file's presence is the
        // existence gate; field validation lives in channel_def_store.
        let def = crate::domain::ChannelDef {
            handle: handle.clone(),
            name: "test".to_string(),
            description: String::new(),
            created_at: Utc::now(),
        };
        crate::infrastructure::save_channel_def(channels_root, &def, false).unwrap();
        dir
    }

    fn launcher() -> PrefsLauncher {
        PrefsLauncher::from_prefs(&CognitionPrefs::default())
    }

    #[test]
    fn unbound_channel_plans_at_default_dir() {
        let tmp = TempDir::new().unwrap();
        let handle = QueueHandle::parse("channel:dev:secretariat").unwrap();
        let default = make_channel(tmp.path(), &handle);
        let plan = launch_channel(tmp.path(), &handle, &launcher()).unwrap();
        assert_eq!(plan.cwd, default);
        assert_eq!(plan.command, "claude");
    }

    #[test]
    fn root_path_redirects_cwd_to_bound_dir() {
        let tmp = TempDir::new().unwrap();
        let handle = QueueHandle::parse("channel:dev:secretariat").unwrap();
        let default = make_channel(tmp.path(), &handle);
        let bound = tmp.path().join("repo");
        std::fs::create_dir_all(&bound).unwrap();
        std::fs::write(
            default.join(CONTRACT_FILENAME),
            format!("---\nroot_path: {}\n---\n", bound.display()),
        )
        .unwrap();
        let plan = launch_channel(tmp.path(), &handle, &launcher()).unwrap();
        assert_eq!(plan.cwd, bound);
    }

    #[test]
    fn non_channel_handle_errors() {
        let tmp = TempDir::new().unwrap();
        let handle = QueueHandle::parse("inbox:triage").unwrap();
        let err = launch_channel(tmp.path(), &handle, &launcher()).unwrap_err();
        assert!(matches!(err, LaunchChannelError::NotAChannelHandle(_)));
    }

    #[test]
    fn missing_channel_errors() {
        let tmp = TempDir::new().unwrap();
        let handle = QueueHandle::parse("channel:never:existed").unwrap();
        let err = launch_channel(tmp.path(), &handle, &launcher()).unwrap_err();
        assert!(matches!(err, LaunchChannelError::ChannelNotFound(_)));
    }
}
