//! `sec launch` — open the principal's chosen interactive cognition CLI
//! (typically Claude Code) inside a channel's bound cwd.
//!
//! Resolves the channel's `root_path` from `<channel-dir>/contract.local.md`
//! when set, otherwise falls back to the default substrate path
//! (`~/.secretariat/<alias>/channel/<segments>/`). Then replaces the
//! current process with the command configured in
//! `~/.secretariat/preferences.toml` under `[cognition]`:
//! `launch_command`, `launch_args`, `launch_env`. Default command is
//! `claude` — see `docs/developer/launch.md` for the LM Studio recipe.
//!
//! On Unix the process is *replaced* via the POSIX `execvp` syscall
//! (`std::os::unix::process::CommandExt::exec`) so the principal's shell
//! hosts Claude Code directly with no `sec` in the middle. On other
//! platforms we fall back to spawn-and-wait. `--print-plan` is the
//! no-spawn escape hatch: emits the resolved plan as JSON for
//! inspection, scripting, and tests.

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::path::PathBuf;

use secretariat_core::application::launch_channel_with_binding as do_launch;
use secretariat_core::application::show_org;
use secretariat_core::domain::{OrgAlias, QueueHandle};
use secretariat_core::infrastructure::org_store::org_channels_root;
use secretariat_core::infrastructure::{load_or_migrate_preferences, PrefsLauncher};

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    /// Channel handle, e.g. `dev:secretariat`.
    pub handle: String,

    /// Org alias scoping the channel. Omit for personal channels under
    /// `~/.secretariat/_self/channels/`.
    #[arg(long)]
    pub org: Option<String>,

    /// Print the resolved launch plan as JSON to stdout and exit
    /// without launching. Useful for scripts inspecting what
    /// `sec launch` *would* do.
    #[arg(long)]
    pub print_plan: bool,
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;

    let handle = QueueHandle::parse(&args.handle)
        .map_err(|e| anyhow!("invalid handle `{}`: {e}", args.handle))?;

    let channels_root = match args.org.as_deref() {
        None => paths.personal_channels_root(),
        Some(s) => {
            let alias = OrgAlias::parse(s)
                .map_err(|e| anyhow!("invalid org alias `{s}`: {e}"))?;
            if show_org(&paths.orgs_root, &alias)
                .context("looking up org")?
                .is_none()
            {
                return Err(anyhow!(
                    "org `{}` does not exist — create it with `sec orgs create {}` first",
                    alias.as_str(),
                    alias.as_str()
                ));
            }
            org_channels_root(&paths.orgs_root, &alias)
        }
    };

    let prefs = load_or_migrate_preferences(
        &paths.preferences,
        &paths.legacy_cognition_config,
        &paths.legacy_cadence,
    )
    .context("loading preferences")?;

    // First pass: resolve the binding via a temporary base-prefs launcher
    // so we can read per-channel overrides off the contract.local.md.
    // Second pass: layer overrides onto a binding-aware launcher and
    // rebuild the plan. Two passes is the cost of keeping the use case
    // launcher-agnostic; cheap (one file read).
    let base_launcher = PrefsLauncher::from_prefs(&prefs.cognition);
    let (_first_plan, binding) =
        do_launch(&channels_root, &handle, &base_launcher).map_err(|e| anyhow!(e))?;
    let launcher = PrefsLauncher::from_prefs_with_binding(&prefs.cognition, &binding);
    let (plan, _binding) =
        do_launch(&channels_root, &handle, &launcher).map_err(|e| anyhow!(e))?;

    if args.print_plan {
        let json = serde_json::json!({
            "command": plan.command,
            "args": plan.args,
            "cwd": plan.cwd,
            "env": plan.env,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    launch_process(plan.command, plan.args, plan.cwd, plan.env)
}

#[cfg(unix)]
fn launch_process(
    command: String,
    args: Vec<String>,
    cwd: PathBuf,
    env: std::collections::BTreeMap<String, String>,
) -> Result<()> {
    use std::os::unix::process::CommandExt;

    let mut cmd = std::process::Command::new(&command);
    cmd.args(&args).current_dir(&cwd);
    for (k, v) in &env {
        cmd.env(k, v);
    }
    // execvp replaces this process; on success the call never returns.
    Err(anyhow!("could not launch `{command}`: {}", cmd.exec()))
}

#[cfg(not(unix))]
fn launch_process(
    command: String,
    args: Vec<String>,
    cwd: PathBuf,
    env: std::collections::BTreeMap<String, String>,
) -> Result<()> {
    let mut cmd = std::process::Command::new(&command);
    cmd.args(&args).current_dir(&cwd);
    for (k, v) in &env {
        cmd.env(k, v);
    }
    let status = cmd
        .status()
        .with_context(|| format!("spawning {command}"))?;
    if !status.success() {
        return Err(anyhow!("{command} exited with {status}"));
    }
    Ok(())
}
