//! Tauri commands wrapping `secretariat-core` use cases.
//!
//! The CLI (`sec`) and the MCP server (`sec-mcp`) are unchanged surfaces;
//! these commands give the Tauri front-end the same primitives via IPC,
//! calling into `secretariat-core` directly (no subprocess, no sidecar).
//!
//! See `docs/milestones/2026-05-04-tauri-front-door.md` for why the Tauri
//! shell is becoming the principal-facing front door.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use secretariat_core::application::read_envelope as core_read_envelope;
use secretariat_core::domain::DisplayName;
use secretariat_core::infrastructure::keys::{
    generate_keypair, load_signing_key, save_signing_key, KeyPaths,
};
use secretariat_core::Did;

/// What `init_identity` reports back to the front-end.
#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct IdentityState {
    /// The principal's DID — `did:key:z…` for a fresh install.
    pub did: String,
    /// Whether this call generated a new identity (true) or surfaced an
    /// existing one (false). The UI uses this to switch between
    /// "Welcome — your identity is …" vs "Welcome back — you're …".
    pub created: bool,
}

/// Ensure the principal has an identity. Idempotent — generates a fresh
/// did:key on first call, returns the existing one thereafter.
///
/// Mirrors `sec init` (without the optional `--did did:web:...` flag, which
/// is a power-user case that can stay in the CLI for now).
#[tauri::command]
#[specta::specta]
pub async fn init_identity() -> Result<IdentityState, String> {
    use chrono::Utc;
    use secretariat_core::domain::DisplayName;
    use secretariat_core::infrastructure::identity_store::{
        load_identity, save_identity, PrincipalIdentity,
    };

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    paths
        .ensure_dirs()
        .map_err(|e| format!("creating directories under {}: {e}", paths.root.display()))?;

    // Already initialized? Surface the existing DID.
    if let Some(existing) =
        load_identity(&paths.identity_md).map_err(|e| format!("loading identity: {e}"))?
    {
        return Ok(IdentityState {
            did: existing.did.as_str().to_string(),
            created: false,
        });
    }

    // Refuse to clobber a partial install (key exists but no identity record).
    if paths.signing_key.exists() {
        return Err(format!(
            "signing key exists at {} but no identity record at {} — refusing to regenerate",
            paths.signing_key.display(),
            paths.identity_md.display()
        ));
    }

    // Generate fresh keypair + derive did:key.
    let key = generate_keypair();
    let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());

    save_signing_key(&paths.signing_key, &key).map_err(|e| {
        format!(
            "writing signing key to {}: {e}",
            paths.signing_key.display()
        )
    })?;

    let now = Utc::now();
    let display_name = DisplayName::parse("Principal")
        .map_err(|e| format!("default display name invalid: {e}"))?;
    let identity = PrincipalIdentity {
        did: did.clone(),
        did_method: "did:key".to_string(),
        display_name,
        full_name: None,
        key_path: "identity/key".to_string(),
        key_type: "ed25519".to_string(),
        key_created_at: now,
        key_rotations: Vec::new(),
        authorized_agents: Vec::new(),
        created_at: now,
        signature: None,
        body: String::new(),
    };
    save_identity(&paths.identity_md, &identity, &key)
        .map_err(|e| format!("writing identity.md: {e}"))?;

    log::info!("init_identity: generated new did:key for principal");
    Ok(IdentityState {
        did: did.as_str().to_string(),
        created: true,
    })
}

/// Surface the current identity without generating one. Returns `None`
/// (serialized as `null`) if no identity exists yet — the front-end can
/// use this to decide whether to show onboarding or the main UI.
#[tauri::command]
#[specta::specta]
pub async fn current_identity() -> Result<Option<IdentityState>, String> {
    use secretariat_core::infrastructure::identity_store::load_identity;

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    if !paths.signing_key.exists() {
        return Ok(None);
    }
    match load_identity(&paths.identity_md).map_err(|e| format!("loading identity: {e}"))? {
        Some(id) => Ok(Some(IdentityState {
            did: id.did.as_str().to_string(),
            created: false,
        })),
        None => Ok(None),
    }
}

/// Diagnostic — returns the absolute path to `~/.secretariat/`. Useful for
/// "open in Finder" buttons and for surfacing where keys live.
#[tauri::command]
#[specta::specta]
pub async fn secretariat_root() -> Result<String, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    Ok(paths.root.display().to_string())
}

// ---------------------------------------------------------------------------
// Review surface — envelope read
// ---------------------------------------------------------------------------
//
// The cross-queue inbox / review-queue listing surfaces were removed in the
// git-native teardown (cut B). What the Tauri shell keeps is the read +
// stamp path: open a file the explorer surfaced, decrypt it, stamp it.

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct EnvelopeRead {
    pub body: String,
    pub from: Option<String>,
    /// DID of the queue *owner* (recipient).
    pub to: Option<String>,
    /// Queue handle on the owner's machine (`<namespace>:<slug>`).
    pub queue: Option<String>,
    pub was_encrypted: bool,
}

/// Decrypt + return the body of an envelope file. Plaintext envelopes
/// pass through unchanged; encrypted envelopes are decrypted using the
/// local signing key (key never leaves the device).
#[tauri::command]
#[specta::specta]
pub async fn read_envelope(file_path: String) -> Result<EnvelopeRead, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let path = PathBuf::from(file_path);
    let res =
        core_read_envelope(&path, &paths.signing_key).map_err(|e| format!("read_envelope: {e}"))?;
    Ok(EnvelopeRead {
        body: res.body,
        from: res.envelope_from.map(|d| d.as_str().to_string()),
        to: res.envelope_to.map(|d| d.as_str().to_string()),
        queue: res.envelope_queue.map(|h| h.as_str().to_string()),
        was_encrypted: res.was_encrypted,
    })
}

/// Stamp a draft. Touch ID fires from the app's window context. The
/// stamp's atomic rename promotes the file into the canonical
/// `envelopes/YYYY/MM/DD/` day-shard, which is the daemon's wire-send
/// signal — federation runs in the daemon (substrate-for-themia,
/// Move 5).
#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct StampReport {
    pub stamped_path: String,
    pub doc_hash: String,
    pub stamped_at: String,
    /// Kept for backward compatibility with the frontend; always
    /// `false` now that federation is daemon-only.
    pub delivered: bool,
    /// Kept for backward compatibility with the frontend; always
    /// `None` now.
    pub relay_assigned_id: Option<String>,
    /// Kept for backward compatibility with the frontend; always
    /// `None` now.
    pub delivery_warning: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn stamp_envelope(file_path: String) -> Result<StampReport, String> {
    use secretariat_core::application::{stamp_document, StampError};
    use secretariat_core::domain::StampAct;
    use secretariat_core::infrastructure::biometric::build_signer;
    use secretariat_core::ports::SignerError;

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let did = load_self_did(&paths)?;
    let key =
        load_signing_key(&paths.signing_key).map_err(|e| format!("loading signing key: {e}"))?;

    let path = std::path::PathBuf::from(file_path);

    // Stamp lives in a blocking call (the Touch ID gate is sync via the
    // touchid-prompt helper). spawn_blocking keeps the runtime healthy.
    let path_for_stamp = path.clone();
    let did_for_stamp = did.clone();
    let key_for_stamp = key.clone();
    let stamp_result = tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
        let signer = build_signer(did_for_stamp, key_for_stamp, false)
            .map_err(|e| format!("biometric gate setup: {e}"))?;
        match stamp_document(
            &path_for_stamp,
            &signer,
            StampAct::Attest,
            false,
            chrono::Utc::now(),
        ) {
            Ok(out) => Ok(out),
            Err(StampError::AlreadyStamped) => Err("file is already stamped".to_string()),
            Err(StampError::Signer(SignerError::BiometricRefused)) => {
                Err("Touch ID refused or cancelled".to_string())
            }
            Err(e) => Err(format!("stamp failed: {e}")),
        }
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    Ok(StampReport {
        stamped_path: stamp_result.stamped_path.display().to_string(),
        doc_hash: stamp_result.stamp.doc_hash.to_string(),
        stamped_at: stamp_result.stamp.stamped_at.to_rfc3339(),
        delivered: false,
        relay_assigned_id: None,
        delivery_warning: None,
    })
}

// ---------------------------------------------------------------------------
// Principal profile — display name (presence, not identity)
// ---------------------------------------------------------------------------
//
// The DID is identity; the profile is presence. The principal sets a
// display name during onboarding (and can edit later). Stored locally
// only — never sent over the wire.

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct Profile {
    pub display_name: String,
}

/// Read the principal's profile. Returns null when no identity is set
/// yet (fresh install pre-onboarding). Backed by `identity.md`
/// frontmatter (v0.7+).
#[tauri::command]
#[specta::specta]
pub async fn get_profile() -> Result<Option<Profile>, String> {
    use secretariat_core::infrastructure::identity_store::load_identity;

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let identity = load_identity(&paths.identity_md).map_err(|e| format!("load_identity: {e}"))?;
    Ok(identity.map(|id| Profile {
        display_name: id.display_name.to_string(),
    }))
}

/// Set the principal's display name. Idempotent — overwrites whatever
/// was there. The DisplayName parser enforces validity (non-empty,
/// reasonable length, etc.).
#[tauri::command]
#[specta::specta]
pub async fn set_profile(display_name: String) -> Result<Profile, String> {
    use secretariat_core::infrastructure::identity_store::{load_identity, save_identity};
    use secretariat_core::infrastructure::keys::load_signing_key;

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    paths
        .ensure_dirs()
        .map_err(|e| format!("creating directories: {e}"))?;
    let parsed = DisplayName::parse(&display_name).map_err(|e| format!("invalid name: {e}"))?;
    let mut identity = load_identity(&paths.identity_md)
        .map_err(|e| format!("load_identity: {e}"))?
        .ok_or_else(|| "no identity yet — initialize first".to_string())?;
    identity.display_name = parsed.clone();
    let signing_key = load_signing_key(&paths.signing_key)
        .map_err(|e| format!("load_signing_key for identity re-sign: {e}"))?;
    save_identity(&paths.identity_md, &identity, &signing_key)
        .map_err(|e| format!("save_identity: {e}"))?;
    Ok(Profile {
        display_name: parsed.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Assistant launcher — open Claude (or any CLI assistant) in Terminal.app
// ---------------------------------------------------------------------------
//
// Click on a home-screen blob spawns the principal's chosen assistant. The
// MCP server is already wired to whichever assistant the principal runs, so
// the launcher just needs to open a terminal and start the binary. No
// clipboard prompt, no prefilled instruction — the assistant picks up
// context naturally via the MCP tools.
//
// Configurability is deferred: hardcoded to `claude` in `Terminal.app` for
// the v0.3 cut. Settings entry lands in a follow-up.

/// Where the home-screen blob launcher points the principal. CLI clients
/// (Claude Code, Gemini CLI, aider) need a terminal to live in; Claude
/// Desktop launches as a regular macOS app and picks up MCP from its own
/// config. Unknown values fall back to Terminal.app.
///
/// macOS-only today. Windows + Linux variants are a separate slice once
/// the GUI ships there (see AGENTS.md "Mac-only Day 1").
#[derive(Debug, Clone, Copy)]
enum AssistantTarget {
    Terminal,
    ITerm,
    Ghostty,
    WezTerm,
    Alacritty,
    ClaudeDesktop,
}

impl AssistantTarget {
    fn from_pref(s: Option<&str>) -> Self {
        match s.map(|x| x.trim().to_ascii_lowercase()).as_deref() {
            Some("iterm") | Some("iterm2") => Self::ITerm,
            Some("ghostty") => Self::Ghostty,
            Some("wezterm") => Self::WezTerm,
            Some("alacritty") => Self::Alacritty,
            Some("claude") | Some("claude-desktop") => Self::ClaudeDesktop,
            _ => Self::Terminal,
        }
    }
}

#[cfg(target_os = "macos")]
fn launch_macos(target: AssistantTarget, command: &str) -> Result<(), String> {
    launch_macos_in(target, command, None)
}

#[cfg(target_os = "macos")]
fn launch_macos_in(
    target: AssistantTarget,
    command: &str,
    cwd: Option<&std::path::Path>,
) -> Result<(), String> {
    // ClaudeDesktop is a direct app open — no terminal, no command. The
    // command + cwd are ignored for this target.
    if matches!(target, AssistantTarget::ClaudeDesktop) {
        let status = std::process::Command::new("open")
            .args(["-a", "Claude"])
            .status()
            .map_err(|e| format!("spawning `open`: {e}"))?;
        if !status.success() {
            return Err("could not open Claude.app — is it installed?".to_string());
        }
        return Ok(());
    }

    // Prepend a `cd "<cwd>" && ` when caller passed a working directory.
    // Quote-escape the path so spaces are safe; the surrounding script
    // already escapes the resulting `"` for osascript below.
    let full_command = match cwd {
        Some(dir) => {
            let dir_str = dir.to_string_lossy().replace('"', "\\\"");
            format!("cd \"{dir_str}\" && {command}")
        }
        None => command.to_string(),
    };

    // WezTerm + Alacritty don't expose a stable AppleScript do-script
    // bridge, so we spawn their CLIs directly and let macOS' app launcher
    // pick up the bundle. `bash -lc` keeps PATH + login profile so
    // `claude` resolves the way the principal expects in their shell.
    match target {
        AssistantTarget::WezTerm => {
            let status = std::process::Command::new("open")
                .args([
                    "-na",
                    "WezTerm",
                    "--args",
                    "start",
                    "--",
                    "bash",
                    "-lc",
                    &full_command,
                ])
                .status()
                .map_err(|e| format!("spawning `open` for WezTerm: {e}"))?;
            if !status.success() {
                return Err("could not open WezTerm — is it installed?".to_string());
            }
            return Ok(());
        }
        AssistantTarget::Alacritty => {
            let status = std::process::Command::new("open")
                .args([
                    "-na",
                    "Alacritty",
                    "--args",
                    "-e",
                    "bash",
                    "-lc",
                    &full_command,
                ])
                .status()
                .map_err(|e| format!("spawning `open` for Alacritty: {e}"))?;
            if !status.success() {
                return Err("could not open Alacritty — is it installed?".to_string());
            }
            return Ok(());
        }
        _ => {}
    }

    let escaped = full_command.replace('"', "\\\"");
    let script = match target {
        AssistantTarget::Terminal => format!(
            "tell application \"Terminal\"\n    activate\n    do script \"{escaped}\"\nend tell"
        ),
        AssistantTarget::ITerm => format!(
            "tell application \"iTerm\"\n    activate\n    create window with default profile\n    tell current session of current window\n        write text \"{escaped}\"\n    end tell\nend tell"
        ),
        AssistantTarget::Ghostty => format!(
            "do shell script \"open -na Ghostty --args -e '{}'\"",
            escaped.replace('\'', "'\\''")
        ),
        AssistantTarget::WezTerm
        | AssistantTarget::Alacritty
        | AssistantTarget::ClaudeDesktop => unreachable!(),
    };
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("spawning osascript: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("osascript failed: {stderr}"));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn launch_macos(_target: AssistantTarget, _command: &str) -> Result<(), String> {
    Err("assistant launcher: only macOS is supported in this build".to_string())
}

#[cfg(not(target_os = "macos"))]
fn launch_macos_in(
    _target: AssistantTarget,
    _command: &str,
    _cwd: Option<&std::path::Path>,
) -> Result<(), String> {
    Err("assistant launcher: only macOS is supported in this build".to_string())
}

/// Spawn the principal's assistant in their preferred environment. Reads
/// `AppPreferences::assistant_terminal` + `assistant_command` from the
/// caller; defaults to Terminal.app + `claude`.
#[tauri::command]
#[specta::specta]
pub async fn launch_assistant(
    terminal: Option<String>,
    command: Option<String>,
) -> Result<(), String> {
    let target = AssistantTarget::from_pref(terminal.as_deref());
    let cmd = command.as_deref().unwrap_or("claude");
    launch_macos(target, cmd)
}

// ---------------------------------------------------------------------------
// Quick-pane launcher commands
// ---------------------------------------------------------------------------

/// Launch Claude at the channel-dir enclosing the given path. Walks up
/// until it finds the nearest `channel.md`; derives handle + org from
/// the path; then calls `launch_channel_from_pane` semantics.
#[tauri::command]
#[specta::specta]
pub async fn launch_claude_at(path: String, terminal: Option<String>) -> Result<(), String> {
    let start = std::path::PathBuf::from(&path);
    let channel_dir = find_enclosing_channel_dir(&start)
        .ok_or_else(|| format!("no enclosing channel.md found for `{path}`"))?;

    // Walk up from channel_dir to find the `channels` segment.
    // Under the Move 3c layout the parent of `channels` is either
    // `<root>/orgs/<alias>` (org channel) or `<root>` itself
    // (self channel).
    let (org, handle) = derive_org_and_handle(&channel_dir)
        .ok_or_else(|| format!("could not derive handle from `{}`", channel_dir.display()))?;

    launch_channel_from_pane(handle, org, terminal).await
}

fn find_enclosing_channel_dir(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let start = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };
    let mut cur = start.as_path();
    loop {
        if cur.join("channel.md").is_file() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

fn derive_org_and_handle(channel_dir: &std::path::Path) -> Option<(Option<String>, String)> {
    let mut segments: Vec<String> = Vec::new();
    let mut cur = channel_dir;
    loop {
        let name = cur.file_name()?.to_string_lossy().into_owned();
        if name == "channels" {
            segments.reverse();
            let handle = segments.join(":");
            // Move 3c layout: org channels live under `<root>/orgs/<alias>/channels/`;
            // self channels live under `<root>/channels/` directly. So the
            // parent of `channels/` is either `<alias>` (whose parent is `orgs`)
            // or the vault root.
            let alias_dir = cur.parent()?;
            let grandparent = alias_dir.parent();
            let org = match grandparent.and_then(|g| g.file_name()) {
                Some(g) if g == std::ffi::OsStr::new("orgs") => {
                    Some(alias_dir.file_name()?.to_string_lossy().into_owned())
                }
                _ => None,
            };
            return Some((org, handle));
        }
        segments.push(name);
        cur = cur.parent()?;
    }
}

/// Launch a channel from the quick-pane via `sec launch` semantics
/// (binding-aware cwd + per-channel cognition overrides applied).
#[tauri::command]
#[specta::specta]
pub async fn launch_channel_from_pane(
    handle: String,
    org: Option<String>,
    terminal: Option<String>,
) -> Result<(), String> {
    use secretariat_core::application::launch_channel_with_binding;
    use secretariat_core::domain::{OrgAlias, QueueHandle};
    use secretariat_core::infrastructure::{
        load_or_migrate_preferences, org_store::org_channels_root, PrefsLauncher,
    };

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let parsed_handle =
        QueueHandle::parse(&handle).map_err(|e| format!("invalid handle `{handle}`: {e}"))?;
    let channels_root = match org.as_deref() {
        None => paths.personal_channels_root(),
        Some(s) => {
            let alias = OrgAlias::parse(s).map_err(|e| format!("invalid org alias `{s}`: {e}"))?;
            org_channels_root(&paths.orgs_root, &alias)
        }
    };
    let prefs = load_or_migrate_preferences(
        &paths.preferences,
        &paths.legacy_cognition_config,
        &paths.legacy_cadence,
    )
    .map_err(|e| format!("loading preferences: {e}"))?;

    let base = PrefsLauncher::from_prefs(&prefs.cognition);
    let (_p, binding) = launch_channel_with_binding(&channels_root, &parsed_handle, &base)
        .map_err(|e| format!("{e}"))?;
    let launcher = PrefsLauncher::from_prefs_with_binding(&prefs.cognition, &binding);
    let (plan, _b) = launch_channel_with_binding(&channels_root, &parsed_handle, &launcher)
        .map_err(|e| format!("{e}"))?;

    let mut shell = String::new();
    for (k, v) in &plan.env {
        let escaped = v.replace('"', "\\\"");
        shell.push_str(&format!("{k}=\"{escaped}\" "));
    }
    shell.push_str(&plan.command);
    for a in &plan.args {
        let escaped = a.replace('"', "\\\"");
        shell.push_str(&format!(" \"{escaped}\""));
    }
    let target = AssistantTarget::from_pref(terminal.as_deref());
    launch_macos_in(target, &shell, Some(&plan.cwd))
}

fn load_self_did(paths: &KeyPaths) -> Result<Did, String> {
    use secretariat_core::infrastructure::identity_store::load_identity;

    let identity =
        load_identity(&paths.identity_md).map_err(|e| format!("loading identity: {e}"))?;
    identity
        .map(|id| id.did)
        .ok_or_else(|| "no identity — run `sec init` first".to_string())
}

// Re-export for the bindings module so it can register these commands.
#[allow(dead_code)]
pub fn _types_used_in_bindings() -> (PathBuf,) {
    (PathBuf::new(),)
}
