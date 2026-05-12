//! Filesystem watcher on `~/.secretariat/outbox/`. When a draft appears
//! or changes, debounce briefly, then fire a callback — wiring this up
//! to `drain_outbox` drops stamp→send latency from the poll cadence
//! (15 min default) to the debounce window (~200 ms).
//!
//! Slice 2 per `docs/ideas/2026-05-12-daemon-evolution.md`. The poll
//! loop remains the safety net: its periodic `sync_now` still drains
//! the outbox so a missed event (rare; e.g. watcher restart races a
//! write) doesn't strand a stamped envelope.
//!
//! # Why the API takes a callback
//!
//! `outbox_watcher` doesn't know about `KeyPaths` or `SigningKey` and
//! doesn't link `secretariat-core`'s `drain_outbox` directly. The
//! watcher's job is pure event plumbing — debounce + dispatch. The
//! caller (the daemon's `serve` loop) wires the real drain. This shape
//! keeps the watcher unit-testable without standing up the crypto
//! stack and matches the v0.3 invariant that callers compose
//! subsystems explicitly.
//!
//! # Debounce semantics
//!
//! Block waiting for the first event. Once one arrives, wait
//! `debounce` for a quiet period, draining any further events that
//! pile up inside the window. Then fire the callback exactly once and
//! return to the blocking wait. A burst of N file writes within
//! `debounce` produces one drain. The watcher uses `notify`'s
//! recommended platform-native backend (FSEvents on macOS, inotify on
//! Linux).

use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Default debounce window. A stamp ceremony writes the envelope file
/// in one go, but downstream tools (editors that save via "atomic
/// write" — move-over) emit a burst. 200ms is generous enough to
/// coalesce that burst without making the principal wait.
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(200);

/// Spawn a watcher task. Returns a `JoinHandle` the caller can `.abort()`
/// on shutdown.
///
/// `on_drain` is invoked once per debounce window when relevant
/// `.md` events arrive under `outbox_dir`. Events inside any
/// `sent/` subdirectory are ignored — those are the daemon's own
/// post-delivery moves and re-triggering the drain on them would
/// loop. The callback is sync over a `Future` so callers can run
/// any async drain logic.
pub fn spawn_watcher<F, Fut>(
    outbox_dir: PathBuf,
    debounce: Duration,
    on_drain: F,
) -> JoinHandle<()>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = run(outbox_dir.clone(), debounce, on_drain).await {
            warn!(dir = %outbox_dir.display(), error = %e, "outbox watcher stopped");
        }
    })
}

async fn run<F, Fut>(outbox_dir: PathBuf, debounce: Duration, on_drain: F) -> Result<()>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    // Make sure the directory exists before we ask `notify` to watch
    // it; some platforms surface a hard error on a missing path and
    // the outbox dir may not exist on a fresh install.
    std::fs::create_dir_all(&outbox_dir)
        .with_context(|| format!("creating {}", outbox_dir.display()))?;

    let (tx, mut rx) = mpsc::unbounded_channel::<notify::Result<Event>>();

    // `recommended_watcher` picks FSEvents on macOS, inotify on Linux,
    // ReadDirectoryChangesW on Windows. The closure runs on notify's
    // own thread; we just forward into the tokio channel so the rest
    // of the pipeline stays async.
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .context("constructing filesystem watcher")?;

    watcher
        .watch(&outbox_dir, RecursiveMode::Recursive)
        .with_context(|| format!("watching {}", outbox_dir.display()))?;

    info!(dir = %outbox_dir.display(), "outbox watcher armed");

    // Initial drain: pick up anything that was stamped while the
    // daemon was down. The poll loop would catch it on its first tick
    // anyway, but firing here closes the boot-time window between
    // daemon start and first tick (up to 15 minutes by default).
    let on_drain = Arc::new(on_drain);
    let initial = Arc::clone(&on_drain);
    tokio::spawn(async move { (initial)().await });

    // Move the watcher into the task so it's kept alive for as long as
    // we're processing events. Dropping it tears down the OS handles.
    let _watcher_guard = Mutex::new(watcher);

    loop {
        // Block until the first event.
        let first = match rx.recv().await {
            Some(r) => r,
            None => return Ok(()), // channel closed → shutdown
        };
        if !should_trigger(&first, &outbox_dir) {
            continue;
        }

        // Quiet-period debounce: sleep, drain anything that piled up
        // during the window, then fire exactly once.
        tokio::time::sleep(debounce).await;
        while let Ok(extra) = rx.try_recv() {
            debug!(?extra, "debounced extra event");
        }

        let cb = Arc::clone(&on_drain);
        tokio::spawn(async move { (cb)().await });
    }
}

/// Decide whether an event under the watched root should fire a drain.
/// We trigger on Create / Modify of `.md` files outside any `sent/`
/// subdirectory. Everything else (the daemon's own post-delivery
/// moves, dot-files, directory metadata changes) is ignored.
fn should_trigger(event: &notify::Result<Event>, _outbox_root: &Path) -> bool {
    let Ok(event) = event else {
        return false;
    };
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {}
        _ => return false,
    }
    event.paths.iter().any(|p| is_outbox_draft(p))
}

fn is_outbox_draft(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("md") {
        return false;
    }
    // Skip the daemon's own post-delivery moves, which land under
    // `<outbox>/<recipient>/sent/`. Without this, every successful
    // send would re-trigger the drain. We check the full path
    // components rather than stripping the watched-root prefix
    // because macOS FSEvents canonicalizes paths (`/var/folders/…`
    // → `/private/var/folders/…`) and a strip_prefix against the
    // symlink form would silently filter every event out.
    !path
        .components()
        .any(|c| c.as_os_str() == std::ffi::OsStr::new("sent"))
}
