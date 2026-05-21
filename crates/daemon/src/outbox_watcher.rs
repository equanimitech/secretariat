//! Filesystem watcher on the substrate root. When a `.md` file appears
//! or changes under any queue's `envelopes/` tree — which is what the
//! stamp ceremony's atomic `_drafts/` → `envelopes/YYYY/MM/DD/` rename
//! triggers — debounce briefly, then fire a callback. Wiring this up
//! to `drain_pending_sends` drops stamp→send latency from the poll
//! cadence (15 min default) to the debounce window (~200 ms).
//!
//! v0.9 drop-outbox pitch (`docs/pitches/2026-05-18-drop-outbox.md`):
//! the old `outbox_watcher` watched `outbox/`; this module name is kept
//! for module-path stability while the behavior moves to envelope-tree
//! watching. The poll loop remains the safety net.
//!
//! # Why the API takes a callback
//!
//! The watcher doesn't know about `KeyPaths` or `SigningKey` and
//! doesn't link `secretariat-core`'s drain directly. Its job is pure
//! event plumbing — debounce + dispatch. The caller (the daemon's
//! `serve` loop) wires the real drain. This shape keeps the watcher
//! unit-testable without standing up the crypto stack and matches the
//! v0.3 invariant that callers compose subsystems explicitly.
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
/// `.md` events arrive under the watched root. Events under
/// substrate-staging trees (`_drafts/`, `_ciphertext/`, `sent/`,
/// `archived/`, `deferred/`) are ignored — `_drafts/` carries
/// unstamped files which the drain skips anyway; `sent/` is the
/// post-delivery archive (re-triggering would loop); the rest are
/// out-of-active-surface. The callback is sync over a `Future` so
/// callers can run any async drain logic.
pub fn spawn_watcher<F, Fut>(
    root_dir: PathBuf,
    debounce: Duration,
    on_drain: F,
) -> JoinHandle<()>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = run(root_dir.clone(), debounce, on_drain).await {
            warn!(dir = %root_dir.display(), error = %e, "envelope watcher stopped");
        }
    })
}

async fn run<F, Fut>(root_dir: PathBuf, debounce: Duration, on_drain: F) -> Result<()>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    // Make sure the directory exists before we ask `notify` to watch
    // it; some platforms surface a hard error on a missing path.
    std::fs::create_dir_all(&root_dir)
        .with_context(|| format!("creating {}", root_dir.display()))?;

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
        .watch(&root_dir, RecursiveMode::Recursive)
        .with_context(|| format!("watching {}", root_dir.display()))?;

    info!(dir = %root_dir.display(), "envelope watcher armed");

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
        if !should_trigger(&first, &root_dir) {
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
/// Triggers on Create / Modify of `.md` files outside any out-of-active-
/// surface ancestor (`_drafts`, `sent`, `_ciphertext`, `archived`,
/// `deferred`). Everything else (the daemon's own post-delivery moves,
/// unstamped drafts, dotfiles, directory metadata) is ignored.
fn should_trigger(event: &notify::Result<Event>, _root: &Path) -> bool {
    let Ok(event) = event else {
        return false;
    };
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {}
        _ => return false,
    }
    event.paths.iter().any(|p| is_envelope_send_candidate(p))
}

fn is_envelope_send_candidate(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("md") {
        return false;
    }
    // Skip out-of-active-surface ancestors. We check the full path
    // components rather than stripping the watched-root prefix because
    // macOS FSEvents canonicalizes paths (`/var/folders/…` →
    // `/private/var/folders/…`) and a strip_prefix against the symlink
    // form would silently filter every event out.
    !path.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("_drafts")
                | Some("sent")
                | Some("_ciphertext")
                | Some("archived")
                | Some("deferred")
        )
    })
}
