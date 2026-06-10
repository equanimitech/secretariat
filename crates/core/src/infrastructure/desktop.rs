//! Desktop integration — open a document in the Secretariat app.
//!
//! Thin LaunchServices adapter shared by `sec view` and the compose
//! callers (CLI + MCP). Spawning the bundled app routes the path through
//! the Tauri single-instance plugin → a window; if the app is not
//! running, LaunchServices launches it first.
//!
//! Callers treat this as **best-effort**: a compose that succeeds must not
//! fail just because no GUI session is present (CI, headless). They log
//! the error and carry on.

use std::path::Path;

/// Open `path` in the Secretariat desktop app (macOS LaunchServices).
#[cfg(target_os = "macos")]
pub fn open_in_secretariat(path: &Path) -> std::io::Result<()> {
    let status = std::process::Command::new("open")
        .args(["-a", "Secretariat"])
        .arg(path)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "`open -a Secretariat` exited {status}"
        )))
    }
}

/// Non-macOS: the Tauri shell is Mac-only, so there is nothing to open.
#[cfg(not(target_os = "macos"))]
pub fn open_in_secretariat(_path: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "opening the Secretariat desktop app is macOS-only",
    ))
}
