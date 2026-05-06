//! Settings-pane Tauri commands. Backs the v0.3 settings panes:
//! Paths (reveal-in-finder), Relay (list / set), and Integrations (MCP
//! wiring status). Profile + Identity stay in `commands::secretariat`,
//! and the quick-pane shortcut stays in `commands::preferences` /
//! `commands::quick_pane`.

use std::path::PathBuf;
use std::process::Command;

use secretariat_core::infrastructure::keys::KeyPaths;
use secretariat_core::infrastructure::transport::RelayState;
use serde::{Deserialize, Serialize};
use specta::Type;

fn key_paths() -> Result<KeyPaths, String> {
    KeyPaths::discover().map_err(|e| format!("resolving secretariat root: {e}"))
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// Reveal a path in Finder (macOS) / file manager (other platforms).
///
/// Used by the Paths pane's "Reveal in Finder" button so the principal
/// can poke around in `~/.secretariat/` without us having to render a
/// file tree inside the app.
#[tauri::command]
#[specta::specta]
pub async fn reveal_in_finder(path: String) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("Reveal in Finder is macOS-only for now".to_string());
    }
    Command::new("open")
        .arg("-R")
        .arg(&path)
        .status()
        .map_err(|e| format!("invoking open: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Relay
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RelayInfo {
    pub endpoint: String,
    pub registered: bool,
}

/// List all relays from `~/.secretariat/relay-state.json`. Returns an
/// empty vec if the file doesn't exist (pre-onboarding).
#[tauri::command]
#[specta::specta]
pub async fn list_relays() -> Result<Vec<RelayInfo>, String> {
    let paths = key_paths()?;
    let state = RelayState::load(&paths.relay_state)
        .map_err(|e| format!("loading relay-state.json: {e}"))?;
    Ok(state
        .iter()
        .map(|e| RelayInfo {
            endpoint: e.endpoint.clone(),
            registered: e.registered,
        })
        .collect())
}

/// Add (or upsert) a relay endpoint. Does NOT register the principal's
/// DID with the relay — that happens automatically the first time
/// `invite` or `accept_invite` runs against this endpoint, or
/// (in CLI flows) via `sec relay register`.
#[tauri::command]
#[specta::specta]
pub async fn add_relay(endpoint: String) -> Result<(), String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err("endpoint must not be empty".to_string());
    }
    if !trimmed.starts_with("https://") && !trimmed.starts_with("http://") {
        return Err("endpoint must start with `https://` or `http://`".to_string());
    }
    let paths = key_paths()?;
    let mut state =
        RelayState::load(&paths.relay_state).map_err(|e| format!("loading: {e}"))?;
    state.entry_mut(trimmed); // upserts an entry if missing
    state
        .save(&paths.relay_state)
        .map_err(|e| format!("saving: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Integrations (MCP)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IntegrationStatus {
    /// Whether the integration is currently wired (server entry exists
    /// in the client's config and points at the bundled `sec-mcp`).
    pub wired: bool,
    /// The path the integration's config currently points at, if any.
    /// Useful for diagnostics — mismatch between this and the bundled
    /// path means the silent-wire is stale.
    pub binary_path: Option<String>,
    /// Where the integration stores its config (for surfacing in the UI).
    pub config_location: Option<String>,
    /// Whether the client itself was detected at all (e.g. Claude Code
    /// CLI installed, Claude Desktop app present).
    pub client_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IntegrationsStatus {
    pub claude_code: IntegrationStatus,
    pub claude_desktop: IntegrationStatus,
    /// The path to the bundled `sec-mcp` we'd wire into clients.
    /// When integrations show a different `binary_path`, the principal
    /// can re-wire from the UI to bring them into sync.
    pub bundled_binary: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_integrations_status() -> Result<IntegrationsStatus, String> {
    let bundled_binary = bundled_sec_mcp_path();
    Ok(IntegrationsStatus {
        claude_code: claude_code_status(),
        claude_desktop: claude_desktop_status(),
        bundled_binary,
    })
}

/// Re-run `sec mcp install` to re-wire Claude Code + Claude Desktop with
/// the current bundled binary. The principal hits this from the
/// Integrations pane when they see a path mismatch.
#[tauri::command]
#[specta::specta]
pub async fn rewire_mcp_integrations() -> Result<(), String> {
    let sec = bundled_sec_path()
        .ok_or_else(|| "bundled `sec` binary not found next to app".to_string())?;
    let sec_mcp =
        bundled_sec_mcp_path().ok_or_else(|| "bundled `sec-mcp` not found".to_string())?;
    let status = Command::new(&sec)
        .args(["mcp", "install", "--binary", &sec_mcp])
        .status()
        .map_err(|e| format!("invoking sec: {e}"))?;
    if !status.success() {
        return Err(format!("`sec mcp install` exited with {status}"));
    }
    Ok(())
}

fn bundled_sec_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidate = dir.join("sec");
    if candidate.exists() {
        Some(candidate.to_string_lossy().into_owned())
    } else {
        None
    }
}

fn bundled_sec_mcp_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let candidate = dir.join("sec-mcp");
    if candidate.exists() {
        Some(candidate.to_string_lossy().into_owned())
    } else {
        None
    }
}

fn claude_code_status() -> IntegrationStatus {
    let claude = which_or_known("claude");
    let client_detected = claude.is_some();
    let mut wired = false;
    let mut binary_path = None;
    if let Some(c) = claude.as_ref() {
        if let Ok(out) = Command::new(c).args(["mcp", "get", "secretariat"]).output() {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                wired = true;
                // Output looks like:
                //   secretariat:
                //     Scope: User config (available in all your projects)
                //     ...
                //     Command: /Applications/Secretariat.app/Contents/MacOS/sec-mcp
                for line in stdout.lines() {
                    let trimmed = line.trim();
                    if let Some(rest) = trimmed.strip_prefix("Command:") {
                        binary_path = Some(rest.trim().to_string());
                        break;
                    }
                }
            }
        }
    }
    IntegrationStatus {
        wired,
        binary_path,
        config_location: dirs::home_dir()
            .map(|h| h.join(".claude.json").display().to_string()),
        client_detected,
    }
}

fn claude_desktop_status() -> IntegrationStatus {
    let config_path = dirs::home_dir().map(|h| {
        h.join("Library/Application Support/Claude/claude_desktop_config.json")
    });
    let client_detected = PathBuf::from("/Applications/Claude.app").exists();
    let mut wired = false;
    let mut binary_path = None;
    if let Some(p) = &config_path {
        if let Ok(text) = std::fs::read_to_string(p) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(cmd) = v
                    .get("mcpServers")
                    .and_then(|m| m.get("secretariat"))
                    .and_then(|s| s.get("command"))
                    .and_then(|c| c.as_str())
                {
                    wired = true;
                    binary_path = Some(cmd.to_string());
                }
            }
        }
    }
    IntegrationStatus {
        wired,
        binary_path,
        config_location: config_path.map(|p| p.display().to_string()),
        client_detected,
    }
}

fn which_or_known(name: &str) -> Option<PathBuf> {
    if let Ok(out) = Command::new("which").arg(name).output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    let home = dirs::home_dir()?;
    [
        home.join(".local/bin").join(name),
        PathBuf::from("/opt/homebrew/bin").join(name),
        PathBuf::from("/usr/local/bin").join(name),
    ]
    .into_iter()
    .find(|p| p.exists())
}
