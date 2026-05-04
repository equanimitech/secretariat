//! `sec daemon` — long-running plumbing process.
//!
//! Three jobs, run on a cadence (default hourly, see
//! `crates/core/src/application/delivery_policy.rs`):
//!
//! 1. Poll every registered relay for new inbound envelopes; file each into
//!    `~/.secretariat/inbox/`.
//! 2. Scan `~/.secretariat/outbox/<recipient-did>/*.md` for stamped envelopes
//!    addressed to a known contact; send via the recipient's relay; move
//!    the file into `outbox/<recipient-did>/sent/`.
//! 3. Persist updated relay-state (cursor + token).
//!
//! Decryption is **not** done by the daemon. Encrypted bodies remain on
//! disk in their wire-string form; `sec read <file>` decrypts on demand.
//! This keeps the principal's signing key out of long-running daemon
//! memory; the daemon is pure plumbing (see milestone "daemon, not agent"
//! section).
//!
//! v0 limitations: foreground only (run via `nohup sec daemon serve &` or
//! similar); no LaunchAgent install, no PID file, no `status` subcommand.
//! Those land in v0.x.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use ed25519_dalek::SigningKey;
use secretariat_core::application::{decide_poll, sync_now, CadenceConfig, PollDecision};
use secretariat_core::infrastructure::keys::{load_signing_key, KeyPaths};
use secretariat_core::infrastructure::transport::{RelayClient, RelayState};
use secretariat_core::Did;
use std::path::Path;
use tracing::{info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use super::paths::{key_paths, load_did};

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Register with a relay (one-time per relay).
    Register {
        /// Relay endpoint URL, e.g. `wss://relay.rafa.equanimi.tech`
        /// or `http://127.0.0.1:8443` for dev.
        #[arg(long)]
        endpoint: String,
    },
    /// Run the foreground daemon loop.
    Serve,
    /// Run a single sync cycle and exit. Useful for cron, post-stamp
    /// pushes, and Tauri's "Sync now" debugging.
    Tick,
    /// Install the daemon as a macOS LaunchAgent. Survives reboot, runs
    /// in the background. Idempotent — safe to re-run after upgrades.
    Install,
    /// Uninstall the LaunchAgent.
    Uninstall,
    /// Report whether the LaunchAgent is loaded + last-known status.
    Status,
}

pub fn run(args: Args) -> Result<()> {
    init_tracing();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;
    runtime.block_on(async move {
        match args.cmd {
            Cmd::Register { endpoint } => register(&endpoint).await,
            Cmd::Serve => serve().await,
            Cmd::Tick => tick_once().await,
            Cmd::Install => install_launchagent().await,
            Cmd::Uninstall => uninstall_launchagent().await,
            Cmd::Status => report_status().await,
        }
    })
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sec=info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().compact())
        .try_init();
}

async fn register(endpoint: &str) -> Result<()> {
    let paths = key_paths()?;
    let did = load_did(&paths)?;
    let key = load_signing_key(&paths.signing_key)
        .with_context(|| format!("loading signing key from {}", paths.signing_key.display()))?;

    let client = RelayClient::new(endpoint, did.clone(), &key);
    client.register().await.context("relay registration")?;

    let mut state = RelayState::load(&paths.relay_state).context("loading relay state")?;
    let entry = state.entry_mut(client.endpoint.as_str());
    entry.registered = true;
    state.save(&paths.relay_state).context("saving relay state")?;

    eprintln!("[sec] registered with {}", client.endpoint);
    eprintln!("[sec]   did: {did}");
    Ok(())
}

async fn serve() -> Result<()> {
    let paths = key_paths()?;
    let did = load_did(&paths)?;
    let key = load_signing_key(&paths.signing_key)
        .with_context(|| format!("loading signing key from {}", paths.signing_key.display()))?;

    paths.ensure_dirs()?;

    let cadence = CadenceConfig::load_or_default(&paths.root.join("cadence.toml"))
        .context("loading cadence config")?;
    info!(
        poll_interval_minutes = cadence.poll_interval_minutes,
        did = %did,
        "daemon serve loop starting"
    );

    let mut last_poll: Option<chrono::DateTime<Utc>> = None;
    loop {
        let now = Utc::now();
        match decide_poll(&cadence, now, last_poll) {
            PollDecision::PollNow => {
                if let Err(e) = tick(&paths, &did, &key).await {
                    warn!(error = %e, "tick failed");
                }
                last_poll = Some(Utc::now());
            }
            PollDecision::WaitUntil(until) => {
                let dur = (until - Utc::now()).to_std().unwrap_or(std::time::Duration::from_secs(60));
                tokio::time::sleep(dur).await;
            }
        }
    }
}

/// One pass over: poll all registered relays for inbound, drain claim
/// notifications (auto-add bilateral contacts from invite claims), then
/// drain outbox. Delegates to `core::application::sync_now`; this
/// function only logs.
async fn tick(paths: &KeyPaths, did: &Did, key: &SigningKey) -> Result<()> {
    let outcome = sync_now(paths, did, key).await.context("sync_now")?;
    for r in &outcome.per_relay {
        if r.inbound_count > 0 {
            info!(endpoint = %r.endpoint, count = r.inbound_count, "filed inbound envelopes");
        }
        if r.auto_added_contacts > 0 {
            info!(
                endpoint = %r.endpoint,
                added = r.auto_added_contacts,
                "auto-added correspondence contacts"
            );
        }
        for w in &r.warnings {
            warn!(endpoint = %r.endpoint, warning = %w, "relay sync warning");
        }
    }
    if outcome.sent_envelopes > 0 {
        info!(count = outcome.sent_envelopes, "sent stamped envelopes");
    }
    for w in &outcome.outbox_warnings {
        warn!(warning = %w, "outbox drain warning");
    }
    Ok(())
}

// Sync orchestration (poll inbound + drain claim notifications + drain
// outbox) lives in `core::application::sync_now` so the CLI daemon, the
// CLI's `sec daemon tick` one-shot, and the Tauri app's "Sync now" command
// share one source of truth. This file contains only the daemon's
// schedule-and-log loop + the LaunchAgent install/uninstall surface.

async fn tick_once() -> Result<()> {
    init_tracing();
    let paths = key_paths()?;
    let did = load_did(&paths)?;
    let key = load_signing_key(&paths.signing_key)
        .with_context(|| format!("loading signing key from {}", paths.signing_key.display()))?;
    tick(&paths, &did, &key).await
}

// -----------------------------------------------------------------------------
// LaunchAgent (install / uninstall / status)
// -----------------------------------------------------------------------------

const LAUNCHAGENT_LABEL: &str = "tech.equanimi.secretariat.daemon";

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

async fn install_launchagent() -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(anyhow!(
            "LaunchAgent install is macOS-only; on Linux/Windows, run `sec daemon serve` under your supervisor of choice."
        ));
    }

    let paths = key_paths()?;
    paths.ensure_dirs()?;
    let log_dir = paths.root.join("logs");
    std::fs::create_dir_all(&log_dir)
        .with_context(|| format!("creating {}", log_dir.display()))?;

    // Resolve the actual `sec` binary path so the LaunchAgent doesn't depend
    // on the LaunchAgent process inheriting the user's PATH.
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

    // Reload: try unload first (idempotent on first install — ignored if not loaded).
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

    eprintln!(
        "[sec] LaunchAgent installed at {}",
        plist_path.display()
    );
    eprintln!("[sec]   binary:  {}", sec_binary.display());
    eprintln!("[sec]   stdout:  {}/daemon.stdout.log", log_dir.display());
    eprintln!("[sec]   stderr:  {}/daemon.stderr.log", log_dir.display());
    eprintln!(
        "[sec] daemon now runs in the background and survives reboots. \
         Verify with `sec daemon status`."
    );
    Ok(())
}

async fn uninstall_launchagent() -> Result<()> {
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

async fn report_status() -> Result<()> {
    let plist_path = launchagent_plist_path()?;
    let installed = plist_path.exists();

    let loaded_output = std::process::Command::new("launchctl")
        .args(["list", LAUNCHAGENT_LABEL])
        .output();
    let loaded = matches!(&loaded_output, Ok(o) if o.status.success());

    println!("LaunchAgent label:    {LAUNCHAGENT_LABEL}");
    println!("plist installed:      {installed} ({})", plist_path.display());
    println!("loaded (launchctl):   {loaded}");
    if loaded {
        // launchctl list <label> prints PID + status; dump it.
        let output = loaded_output.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().take(8) {
            println!("  {line}");
        }
    }

    // Also list registered relays + queued outbox files for at-a-glance health.
    let paths = key_paths()?;
    if let Ok(state) = secretariat_core::infrastructure::transport::RelayState::load(
        &paths.relay_state,
    ) {
        let count = state.iter().count();
        println!("registered relays:    {count}");
        for r in state.iter() {
            let cursor = r.cursor;
            println!("  {} (cursor={cursor})", r.endpoint);
        }
    }
    Ok(())
}
