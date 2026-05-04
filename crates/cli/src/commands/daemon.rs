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

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use ed25519_dalek::SigningKey;
use secretariat_core::application::{decide_poll, CadenceConfig, PollDecision};
use secretariat_core::infrastructure::contact_store::ContactBook;
use secretariat_core::infrastructure::keys::{load_signing_key, KeyPaths};
use secretariat_core::infrastructure::markdown::parse_document;
use secretariat_core::infrastructure::transport::{RelayClient, RelayInbound, RelayState};
use secretariat_core::Did;
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

/// One pass over: poll all registered relays for inbound, then drain outbox.
async fn tick(paths: &KeyPaths, did: &Did, key: &SigningKey) -> Result<()> {
    poll_all_relays(paths, did, key).await?;
    drain_outbox(paths, did, key).await?;
    Ok(())
}

async fn poll_all_relays(paths: &KeyPaths, did: &Did, key: &SigningKey) -> Result<()> {
    let mut state = RelayState::load(&paths.relay_state).context("loading relay state")?;

    // Collect endpoints up-front; we mutate state inside the loop.
    let endpoints: Vec<String> = state
        .iter()
        .filter(|r| r.registered)
        .map(|r| r.endpoint.clone())
        .collect();

    for endpoint in endpoints {
        if let Err(e) = poll_one(&mut state, &endpoint, did, key, paths).await {
            warn!(endpoint = %endpoint, error = %e, "poll failed; will retry next tick");
        }
    }

    state.save(&paths.relay_state).context("saving relay state")?;
    Ok(())
}

async fn poll_one(
    state: &mut RelayState,
    endpoint: &str,
    did: &Did,
    key: &SigningKey,
    paths: &KeyPaths,
) -> Result<()> {
    let client = RelayClient::new(endpoint, did.clone(), key);

    // Refresh token if missing or near expiry.
    let needs_auth = match state.entry(endpoint) {
        Some(e) => match (e.token.as_ref(), e.token_expires_at) {
            (Some(_), Some(exp)) => Utc::now() >= exp - chrono::Duration::minutes(5),
            _ => true,
        },
        None => true,
    };
    if needs_auth {
        let (token, expires_at) = client.authenticate().await.context("relay authenticate")?;
        let entry = state.entry_mut(endpoint);
        entry.token = Some(token);
        entry.token_expires_at = Some(expires_at);
    }

    let (token, cursor) = {
        let e = state.entry(endpoint).expect("just upserted");
        (e.token.clone().unwrap(), e.cursor)
    };

    let inbound = client.poll(&token, cursor).await.context("relay poll")?;
    let mut max_id = cursor;
    for env in &inbound {
        if let Err(e) = file_inbound(paths, env) {
            warn!(id = env.id, error = %e, "could not file inbound envelope");
            continue;
        }
        if env.id > max_id {
            max_id = env.id;
        }
    }
    if !inbound.is_empty() {
        info!(endpoint = %endpoint, count = inbound.len(), "filed inbound envelopes");
    }
    state.entry_mut(endpoint).cursor = max_id;
    Ok(())
}

fn file_inbound(paths: &KeyPaths, env: &RelayInbound) -> Result<()> {
    let sender_short = env
        .sender_did
        .as_ref()
        .map(|d| short_did(d.as_str()))
        .unwrap_or_else(|| "unknown".to_string());
    let timestamp = env.queued_at.format("%Y-%m-%dT%H-%M-%SZ");
    let filename = format!("{timestamp}-{sender_short}-id{:06}.md", env.id);
    let path = paths.inbox.join(filename);
    std::fs::write(&path, &env.body)
        .with_context(|| format!("writing inbox file {}", path.display()))?;
    Ok(())
}

fn short_did(s: &str) -> String {
    s.replace([':', '/'], "_")
        .chars()
        .take(48)
        .collect()
}

async fn drain_outbox(paths: &KeyPaths, _did: &Did, key: &SigningKey) -> Result<()> {
    if !paths.outbox.exists() {
        return Ok(());
    }
    let contacts = ContactBook::load(&paths.contacts).context("loading contacts")?;

    // Iterate outbox/<recipient-did>/*.md (one level deep, excluding the
    // `sent/` subdirectory).
    for entry in std::fs::read_dir(&paths.outbox)? {
        let entry = entry?;
        let recipient_dir = entry.path();
        if !recipient_dir.is_dir() {
            continue;
        }
        let sent_dir = recipient_dir.join("sent");
        std::fs::create_dir_all(&sent_dir)
            .with_context(|| format!("creating {}", sent_dir.display()))?;

        for inner in std::fs::read_dir(&recipient_dir)? {
            let inner = inner?;
            let p = inner.path();
            if !p.is_file() {
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            if let Err(e) = try_send_one(&p, &contacts, key, &sent_dir).await {
                warn!(file = %p.display(), error = %e, "outbox send failed");
            }
        }
    }
    Ok(())
}

async fn try_send_one(
    path: &Path,
    contacts: &ContactBook,
    key: &SigningKey,
    sent_dir: &Path,
) -> Result<()> {
    let raw = std::fs::read(path)
        .with_context(|| format!("reading {}", path.display()))?;
    let raw_str = std::str::from_utf8(&raw)
        .with_context(|| format!("envelope {} is not valid utf-8", path.display()))?;
    let parsed = parse_document(raw_str).context("parsing envelope")?;

    let envelope = parsed
        .envelope
        .ok_or_else(|| anyhow!("envelope frontmatter missing — composer should have written it"))?;
    if parsed.stamp.is_none() {
        // Not stamped yet; principal hasn't approved. Skip silently.
        return Ok(());
    }
    let recipient_did = envelope
        .to
        .as_ref()
        .ok_or_else(|| anyhow!("envelope has no `to` — cannot route"))?;

    let contact = contacts
        .find_by_did(recipient_did)
        .ok_or_else(|| anyhow!("no contact for recipient {recipient_did}"))?;
    let endpoint = contact
        .relay_endpoint
        .as_ref()
        .ok_or_else(|| anyhow!(
            "contact `{}` has no relay_endpoint and v0 does not yet do live did:web service-endpoint discovery",
            contact.display_name
        ))?;

    let client = RelayClient::new(endpoint.as_str(), envelope.from.clone(), key);
    let id = client
        .send(recipient_did, &raw, "text/markdown")
        .await
        .context("relay send")?;

    let dest = sent_dir.join(path.file_name().unwrap());
    std::fs::rename(path, &dest)
        .with_context(|| format!("moving {} → {}", path.display(), dest.display()))?;
    info!(file = %path.display(), id, "sent and moved to sent/");
    Ok(())
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
