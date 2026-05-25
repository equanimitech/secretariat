//! macOS LaunchAgent surface for the daemon.
//!
//! `install_launchagent` writes a plist at
//! `~/Library/LaunchAgents/tech.equanimi.secretariat.daemon.plist` and
//! loads it via `launchctl load -w`. `RunAtLoad = true` + `KeepAlive =
//! true` mean the daemon comes up at login and respawns on crash.
//! Idempotent: re-running unloads first.
//!
//! `uninstall_launchagent` unloads and deletes the plist.
//!
//! `report_status` shows whether the plist is installed, whether
//! `launchctl` has it loaded, and dumps the first few lines of `launchctl
//! list <label>` (PID, exit status). Also lists registered relays and
//! their cursors for at-a-glance health.
//!
//! Linux / Windows are explicitly unsupported here — those platforms run
//! the daemon under their own supervisor (systemd, NSSM, etc.) and the
//! caller wires it up by hand.

use anyhow::{anyhow, Context, Result};
use secretariat_core::infrastructure::keys::KeyPaths;
use std::path::Path;

pub const LAUNCHAGENT_LABEL: &str = "tech.equanimi.secretariat.daemon";

fn launchagent_plist_path() -> Result<std::path::PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home directory"))?;
    Ok(home.join(format!("Library/LaunchAgents/{LAUNCHAGENT_LABEL}.plist")))
}

fn render_plist(sec_binary: &Path, log_dir: &Path) -> String {
    let bin = sec_binary.display();
    let stdout = log_dir.join("daemon.stdout.log");
    let stderr = log_dir.join("daemon.stderr.log");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCHAGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
        <string>daemon</string>
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin</string>
    </dict>
</dict>
</plist>
"#,
        stdout.display(),
        stderr.display()
    )
}

pub async fn install_launchagent(paths: &KeyPaths) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(anyhow!(
            "LaunchAgent install is macOS-only; on Linux/Windows, run `sec daemon serve` under your supervisor of choice."
        ));
    }

    paths.ensure_dirs()?;
    let log_dir = paths.root.join("logs");
    std::fs::create_dir_all(&log_dir).with_context(|| format!("creating {}", log_dir.display()))?;

    // Resolve the actual `sec` binary path so the LaunchAgent doesn't depend
    // on inheriting the user's PATH.
    let sec_binary = std::env::current_exe()
        .context("resolving sec binary path")?
        .canonicalize()
        .context("canonicalizing sec binary path")?;

    let plist_path = launchagent_plist_path()?;
    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let body = render_plist(&sec_binary, &log_dir);
    std::fs::write(&plist_path, body)
        .with_context(|| format!("writing {}", plist_path.display()))?;

    // Idempotent reload: unload-first is a no-op on a fresh install.
    let _ = std::process::Command::new("launchctl")
        .args(["unload", plist_path.to_string_lossy().as_ref()])
        .output();

    let load = std::process::Command::new("launchctl")
        .args(["load", "-w", plist_path.to_string_lossy().as_ref()])
        .output()
        .context("invoking launchctl load")?;
    if !load.status.success() {
        return Err(anyhow!(
            "launchctl load failed: {}",
            String::from_utf8_lossy(&load.stderr)
        ));
    }

    eprintln!("[sec] LaunchAgent installed at {}", plist_path.display());
    eprintln!("[sec]   binary:  {}", sec_binary.display());
    eprintln!("[sec]   stdout:  {}/daemon.stdout.log", log_dir.display());
    eprintln!("[sec]   stderr:  {}/daemon.stderr.log", log_dir.display());
    eprintln!(
        "[sec] daemon now runs in the background and survives reboots. \
         Verify with `sec daemon status`."
    );
    Ok(())
}

pub async fn uninstall_launchagent() -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(anyhow!("LaunchAgent uninstall is macOS-only"));
    }
    let plist_path = launchagent_plist_path()?;
    if !plist_path.exists() {
        eprintln!("[sec] no LaunchAgent installed at {}", plist_path.display());
        return Ok(());
    }
    let _ = std::process::Command::new("launchctl")
        .args(["unload", plist_path.to_string_lossy().as_ref()])
        .output();
    std::fs::remove_file(&plist_path)
        .with_context(|| format!("removing {}", plist_path.display()))?;
    eprintln!("[sec] LaunchAgent uninstalled.");
    Ok(())
}

pub async fn report_status(paths: &KeyPaths) -> Result<()> {
    let plist_path = launchagent_plist_path()?;
    let installed = plist_path.exists();

    let loaded_output = std::process::Command::new("launchctl")
        .args(["list", LAUNCHAGENT_LABEL])
        .output();
    let loaded = matches!(&loaded_output, Ok(o) if o.status.success());

    println!("LaunchAgent label:    {LAUNCHAGENT_LABEL}");
    println!(
        "plist installed:      {installed} ({})",
        plist_path.display()
    );
    println!("loaded (launchctl):   {loaded}");
    if loaded {
        let output = loaded_output.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().take(8) {
            println!("  {line}");
        }
    }

    if let Ok(state) =
        secretariat_core::infrastructure::transport::RelayState::load(&paths.relay_state)
    {
        let count = state.iter().count();
        println!("registered relays:    {count}");
        for r in state.iter() {
            let queue_count = r.queue_cursors.len();
            let max_cursor = r.queue_cursors.iter().map(|q| q.cursor).max().unwrap_or(0);
            println!(
                "  {} (queues={queue_count}, max_cursor={max_cursor})",
                r.endpoint
            );
        }
    }
    Ok(())
}
