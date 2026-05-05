//! `sec mcp install` — auto-wire `sec-mcp` into Claude Desktop + Claude Code.
//!
//! Replaces the manual JSON edit at
//! `~/Library/Application Support/Claude/claude_desktop_config.json`.
//! Detects an existing config, merges the `secretariat` entry into
//! `mcpServers`, writes back atomically. For Claude Code the project-scope
//! `.mcp.json` lives in the repo (committed); user-scope is wired via the
//! `claude mcp add` CLI when present.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{Map, Value};
use tempfile::NamedTempFile;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Install `sec-mcp` into Claude Desktop and (when present) Claude Code.
    /// Idempotent — safe to re-run after upgrades.
    Install {
        /// Override the path to the `sec-mcp` binary. By default, resolved
        /// in this order: sibling of the running `sec` exe (Tauri sidecar
        /// case), `which sec-mcp`, `~/.cargo/bin/sec-mcp`,
        /// `~/.local/bin/sec-mcp`.
        #[arg(long)]
        binary: Option<PathBuf>,

        /// Skip Claude Desktop config (e.g. you don't use it).
        #[arg(long, default_value_t = false)]
        skip_desktop: bool,

        /// Skip Claude Code wiring.
        #[arg(long, default_value_t = false)]
        skip_code: bool,
    },
}

pub fn run(args: Args) -> Result<()> {
    match args.cmd {
        Cmd::Install {
            binary,
            skip_desktop,
            skip_code,
        } => install(binary, skip_desktop, skip_code),
    }
}

fn install(binary: Option<PathBuf>, skip_desktop: bool, skip_code: bool) -> Result<()> {
    let binary_path = resolve_binary(binary)?;
    eprintln!("[sec] using sec-mcp binary: {}", binary_path.display());

    let mut wired_anywhere = false;

    if !skip_desktop {
        match wire_claude_desktop(&binary_path) {
            Ok(path) => {
                eprintln!(
                    "[sec] wired Claude Desktop config: {}\n[sec]   restart Claude Desktop for the secretariat tools to appear.",
                    path.display()
                );
                wired_anywhere = true;
            }
            Err(e) => {
                eprintln!("[sec] (skipped Claude Desktop) {e}");
            }
        }
    }

    if !skip_code {
        match wire_claude_code(&binary_path) {
            Ok(()) => {
                eprintln!(
                    "[sec] wired Claude Code (user scope) — `claude mcp list` should show `secretariat`."
                );
                wired_anywhere = true;
            }
            Err(e) => {
                eprintln!("[sec] (skipped Claude Code) {e}");
            }
        }
    }

    if !wired_anywhere {
        return Err(anyhow!(
            "neither Claude Desktop nor Claude Code was wired. Pass --binary if sec-mcp is installed elsewhere, or wire manually."
        ));
    }
    Ok(())
}

fn resolve_binary(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if !p.exists() {
            return Err(anyhow!(
                "--binary path does not exist: {}",
                p.display()
            ));
        }
        return Ok(p);
    }

    // Sibling of the running `sec` binary (e.g. inside
    // Secretariat.app/Contents/MacOS/ when invoked from the Tauri-bundled
    // sidecar). Checked first so a bundled .app wins over a stray
    // `~/.cargo/bin/sec-mcp` left over from dev installs.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join("sec-mcp");
            if sibling.exists() && sibling != exe {
                return Ok(sibling);
            }
        }
    }

    // Try `which sec-mcp`.
    if let Ok(output) = Command::new("which").arg("sec-mcp").output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                return Ok(PathBuf::from(s));
            }
        }
    }

    // Fallback: ~/.cargo/bin/sec-mcp
    if let Some(home) = dirs::home_dir() {
        let cargo_bin = home.join(".cargo/bin/sec-mcp");
        if cargo_bin.exists() {
            return Ok(cargo_bin);
        }
        let local_bin = home.join(".local/bin/sec-mcp");
        if local_bin.exists() {
            return Ok(local_bin);
        }
    }

    Err(anyhow!(
        "could not locate sec-mcp next to `sec`, on PATH, or in ~/.cargo/bin / ~/.local/bin. \
         Pass --binary <path> explicitly, or run `cargo install --path crates/mcp` first."
    ))
}

fn claude_desktop_config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
    #[cfg(target_os = "macos")]
    let path = home
        .join("Library")
        .join("Application Support")
        .join("Claude")
        .join("claude_desktop_config.json");
    #[cfg(target_os = "linux")]
    let path = home
        .join(".config")
        .join("Claude")
        .join("claude_desktop_config.json");
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let path = {
        let _ = home;
        return Err(anyhow!("Claude Desktop config path is unknown on this OS"));
    };
    Ok(path)
}

fn wire_claude_desktop(binary: &std::path::Path) -> Result<PathBuf> {
    let path = claude_desktop_config_path()?;

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            return Err(anyhow!(
                "Claude Desktop is not installed (no config dir at {}). \
                 Install Claude Desktop first or pass --skip-desktop.",
                parent.display()
            ));
        }
    }

    // Read existing config or start fresh.
    let mut root: Value = if path.exists() {
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        if raw.trim().is_empty() {
            Value::Object(Map::new())
        } else {
            serde_json::from_str(&raw)
                .with_context(|| format!("parsing existing {}", path.display()))?
        }
    } else {
        Value::Object(Map::new())
    };

    let root_obj = root
        .as_object_mut()
        .ok_or_else(|| anyhow!("{} is not a JSON object", path.display()))?;

    let servers = root_obj
        .entry("mcpServers".to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| anyhow!("`mcpServers` in {} is not an object", path.display()))?;

    servers.insert(
        "secretariat".to_string(),
        serde_json::json!({
            "command": binary.display().to_string(),
            "args": []
        }),
    );

    write_atomic(&path, &root)?;
    Ok(path)
}

fn wire_claude_code(binary: &std::path::Path) -> Result<()> {
    // Only wire user-scope; project scope already lives at `.mcp.json` in
    // any cloned repo. If `claude` CLI isn't on PATH, surface that as an
    // info-level "skipped" rather than a hard error.
    //
    // PATH fallback matters: when `sec mcp install` runs from a Tauri-
    // launched GUI (the v0.2.5 install flow), the process inherits the
    // macOS GUI default PATH (`/usr/bin:/bin:/usr/sbin:/sbin`) — not the
    // shell PATH. `which claude` fails even though Claude Code is
    // installed at `~/.local/bin/claude` (its standard install path).
    // Without this fallback, Tauri-launched wiring silently skips
    // Claude Code and only Claude Desktop gets the entry.
    let claude = match which("claude").or_else(claude_in_known_locations) {
        Some(p) => p,
        None => {
            return Err(anyhow!(
                "`claude` CLI not found on PATH or in known locations (~/.local/bin, /opt/homebrew/bin, /usr/local/bin) — Claude Code may not be installed"
            ));
        }
    };

    // `claude mcp list` to check if `secretariat` is already wired (idempotent).
    let listing = Command::new(&claude).args(["mcp", "list"]).output();
    let already = listing
        .as_ref()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("secretariat"))
        .unwrap_or(false);

    if already {
        // Remove first so we can re-add with the (possibly updated) binary path.
        let _ = Command::new(&claude)
            .args(["mcp", "remove", "secretariat", "-s", "user"])
            .output();
    }

    let status = Command::new(&claude)
        .args([
            "mcp",
            "add",
            "secretariat",
            "-s",
            "user",
            "--",
            binary.to_str().ok_or_else(|| anyhow!("binary path is not utf-8"))?,
        ])
        .status()
        .with_context(|| format!("running `{} mcp add ...`", claude.display()))?;

    if !status.success() {
        return Err(anyhow!("`claude mcp add` exited with {status}"));
    }
    Ok(())
}

/// Fallback for when `which claude` fails because the parent process has
/// no shell PATH (e.g. macOS GUI app spawning `sec mcp install` on launch).
/// Checks the standard install locations Claude Code uses.
fn claude_in_known_locations() -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = {
        let mut v = Vec::new();
        if let Some(home) = dirs::home_dir() {
            v.push(home.join(".local/bin/claude"));
        }
        v.push(PathBuf::from("/opt/homebrew/bin/claude"));
        v.push(PathBuf::from("/usr/local/bin/claude"));
        v
    };
    candidates.into_iter().find(|p| p.exists())
}

fn which(name: &str) -> Option<PathBuf> {
    let output = Command::new("which").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

fn write_atomic(path: &std::path::Path, value: &Value) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("creating {}", parent.display()))?;

    let pretty = serde_json::to_string_pretty(value)?;
    let mut tmp = NamedTempFile::new_in(parent)
        .with_context(|| format!("opening tempfile in {}", parent.display()))?;
    use std::io::Write as _;
    tmp.write_all(pretty.as_bytes())?;
    tmp.write_all(b"\n")?;
    tmp.persist(path)
        .map_err(|e| anyhow!("renaming tempfile to {}: {}", path.display(), e.error))?;
    Ok(())
}
