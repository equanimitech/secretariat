//! The daemon's serve loop, one-shot tick, and the shared "do one cycle"
//! primitive both call into.
//!
//! `serve` runs forever: each iteration consults
//! [`secretariat_core::application::decide_poll`] against the configured
//! cadence (default 15-min floor, override via
//! `~/.secretariat/cadence.toml`). When it's time to poll, [`run_tick`]
//! runs one cycle of [`secretariat_core::application::sync_now`].
//!
//! `serve` also brings up the [`crate::ipc`] Unix-socket listener as a
//! sibling task so CLI / Tauri / MCP can route control calls through the
//! running daemon (Slice 1). If the socket can't be bound — another
//! daemon is already running, or the user's filesystem refuses Unix
//! sockets — the loop continues without IPC; behavior is unchanged from
//! v0.2.16. The socket is *additive*; nothing depends on it.
//!
//! `tick_once` runs exactly one cycle and exits. Used by `sec daemon
//! tick` from scripts and the Tauri "Sync now" affordance.
//!
//! Soft errors are logged via `tracing::warn` and do not abort the loop.
//! Each relay is independent: one transient failure doesn't poison the
//! others' cursors. See `sync_now`'s per-relay report for the contract.

use anyhow::{Context, Result};
use chrono::Utc;
use ed25519_dalek::SigningKey;
use secretariat_core::application::{
    decide_poll, sync_now, CadenceConfig, PollDecision, SyncOutcome,
};
use secretariat_core::infrastructure::preferences::load_or_migrate as load_or_migrate_preferences;
use secretariat_core::infrastructure::keys::KeyPaths;
use secretariat_core::Did;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Process-wide serializer for `sync_now` execution.
///
/// `sync_now` does a `RelayState::load` → mutate → `RelayState::save`
/// sequence that is *not* an atomic transaction. Two concurrent callers
/// — the daemon's poll loop on its cadence + an IPC `tick` request
/// arriving over the socket — would both load the same state, mutate
/// independently, and the last writer would clobber the other's cursor
/// advancement. The IPC socket was meant to eliminate this race, but
/// merely moved one caller inside the daemon boundary; serializing
/// here is what actually closes it.
///
/// Global rather than per-instance because the contended resource is
/// a file on disk owned by the process. Multiple daemons in the same
/// process is not a configuration we support (and `spawn_server`'s
/// stale-socket logic would refuse it anyway).
pub(crate) fn tick_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub async fn serve(paths: &KeyPaths, did: &Did, key: &SigningKey) -> Result<()> {
    paths.ensure_dirs()?;

    let prefs = load_or_migrate_preferences(
        &paths.preferences,
        &paths.legacy_cognition_config,
        &paths.legacy_cadence,
    )
    .context("loading preferences")?;
    let cadence = CadenceConfig {
        poll_interval_minutes: prefs.delivery.poll_interval_minutes as i64,
    };
    info!(
        poll_interval_minutes = cadence.poll_interval_minutes,
        did = %did,
        "daemon serve loop starting"
    );

    // Bring up the IPC socket alongside the poll loop. Failures here are
    // logged and swallowed — the loop still serves its primary duty
    // (polling + outbox drain) without IPC. v0.2.16 behavior preserved.
    // We retain the JoinHandle so the shutdown path can abort the
    // listener and unlink the socket file before the process exits.
    let ipc_handle = crate::ipc::spawn_server(paths.clone(), did.clone(), key.clone());

    // FS-notify on the outbox: stamp → send latency drops from cadence
    // (15 min default) to ~200ms debounce. The drain shares
    // `tick_lock` with `run_tick` so it can't race the poll loop's
    // outbox drain. The poll loop stays as the safety net for missed
    // events (e.g. watcher restart during a write).
    // v0.3 substrate: there's no single `paths.outbox` anymore — each
    // queue carries its own `outbox/` subdir scattered across
    // `<root>/<alias>/<namespace>/<segments>/`. Watching the
    // substrate root covers them all; the watcher's filter (`.md`
    // outside any `sent/` ancestor) keeps spurious events at bay.
    let watcher_key = key.clone();
    let watcher_paths = paths.clone();
    let outbox_handle = crate::outbox_watcher::spawn_watcher(
        paths.root.clone(),
        crate::outbox_watcher::DEFAULT_DEBOUNCE,
        move || {
            let paths = watcher_paths.clone();
            let key = watcher_key.clone();
            async move {
                let _guard = tick_lock().lock().await;
                match secretariat_core::application::drain_outbox(&paths, &key).await {
                    Ok((sent, warnings)) => {
                        if sent > 0 {
                            info!(count = sent, "fs-notify drained outbox");
                        }
                        for w in &warnings {
                            warn!(warning = %w, "outbox drain warning");
                        }
                    }
                    Err(e) => warn!(error = %e, "fs-notify outbox drain failed"),
                }
            }
        },
    );

    // SIGTERM is what `launchctl unload` sends; Ctrl-C in a foreground
    // `sec daemon serve` produces SIGINT. Handle both so the next start
    // doesn't have to clean up a stale socket file.
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("installing SIGTERM handler")?;

    let poll_loop = async {
        let mut last_poll: Option<chrono::DateTime<Utc>> = None;
        loop {
            let now = Utc::now();
            match decide_poll(&cadence, now, last_poll) {
                PollDecision::PollNow => {
                    if let Err(e) = tick(paths, did, key).await {
                        warn!(error = %e, "tick failed");
                    }
                    last_poll = Some(Utc::now());
                }
                PollDecision::WaitUntil(until) => {
                    let dur = (until - Utc::now())
                        .to_std()
                        .unwrap_or(std::time::Duration::from_secs(60));
                    tokio::time::sleep(dur).await;
                }
            }
        }
    };

    tokio::select! {
        // The poll loop never returns under normal operation, but we
        // pin it into the select so signal arrival can preempt the
        // `tokio::time::sleep` inside it.
        _ = poll_loop => {}
        _ = tokio::signal::ctrl_c() => {
            info!("daemon shutdown signal received, draining");
        }
        _ = sigterm.recv() => {
            info!("daemon shutdown signal received, draining");
        }
    }

    // Best-effort socket cleanup. Aborting the listener task tears down
    // the `UnixListener`; removing the socket path ensures the next
    // boot sees a clean slate even if the listener already exited.
    ipc_handle.abort();
    outbox_handle.abort();
    let _ = std::fs::remove_file(crate::ipc::socket_path(paths));
    Ok(())
}

/// One sync cycle: poll inbound across registered relays, drain claim
/// notifications, drain stamped outbox. Returns the [`SyncOutcome`] so
/// callers (loop, IPC server, CLI fallback) can decide how to surface
/// results.
pub async fn run_tick(
    paths: &KeyPaths,
    did: &Did,
    key: &SigningKey,
) -> Result<SyncOutcome> {
    let _guard = tick_lock().lock().await;
    sync_now(paths, did, key).await.context("sync_now")
}

/// Run one tick and log the outcome via `tracing`. The loop's per-tick
/// entry point.
pub async fn tick(paths: &KeyPaths, did: &Did, key: &SigningKey) -> Result<()> {
    let outcome = run_tick(paths, did, key).await?;
    log_outcome(&outcome);
    Ok(())
}

/// One-line human summary of a sync cycle, e.g.
/// `[sec] tick: 2 inbound, 1 sent, 0 warnings` (or
/// `[sec] tick: nothing to do` when all totals are zero). Used by the
/// `sec daemon tick` one-shot to give the principal *some* feedback on
/// stderr regardless of whether the cycle ran via IPC or in-proc. The
/// long-running serve loop deliberately does NOT print this on every
/// tick — that path stays `tracing`-structured to keep ambient logs
/// quiet under `RUST_LOG=info`.
pub fn summary_line(outcome: &SyncOutcome) -> String {
    let inbound: usize = outcome.per_relay.iter().map(|r| r.inbound_count).sum();
    let sent = outcome.sent_envelopes;
    let warnings: usize = outcome
        .per_relay
        .iter()
        .map(|r| r.warnings.len())
        .sum::<usize>()
        + outcome.outbox_warnings.len();
    if inbound == 0 && sent == 0 && warnings == 0 {
        "[sec] tick: nothing to do".to_string()
    } else {
        format!("[sec] tick: {inbound} inbound, {sent} sent, {warnings} warnings")
    }
}

pub fn log_outcome(outcome: &SyncOutcome) {
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
}

/// Run a single cycle and exit. Used by `sec daemon tick` when no daemon
/// is running, and by `tick_via_ipc_or_inproc` as the fallback path.
pub async fn tick_once(paths: &KeyPaths, did: &Did, key: &SigningKey) -> Result<()> {
    tick(paths, did, key).await
}
