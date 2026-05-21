//! Tests for the envelope FS-notify watcher. Don't touch the crypto
//! stack — wire the watcher's callback to an `AtomicUsize` counter so
//! we can assert exactly when (and how many times) a drain would fire.
//! The real `drain_pending_sends` is exercised in core's integration tests.
//!
//! The watcher watches the substrate root and triggers on `.md` events
//! whose path is NOT under any `_drafts/`, `sent/`, `_ciphertext/`,
//! `archived/`, or `deferred/` ancestor — so the stamp ceremony's
//! atomic `_drafts/<x>.md` → `envelopes/YYYY/MM/DD/<x>.md` rename is
//! what the watcher reacts to.

use secretariat_daemon::outbox_watcher::{spawn_watcher, DEFAULT_DEBOUNCE};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn make_substrate_root() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("substrate");
    std::fs::create_dir_all(&root).unwrap();
    (tmp, root)
}

fn install_counter() -> (
    Arc<AtomicUsize>,
    impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        + Send
        + Sync
        + 'static,
) {
    let counter = Arc::new(AtomicUsize::new(0));
    let cb_counter = Arc::clone(&counter);
    let cb = move || {
        let c = Arc::clone(&cb_counter);
        Box::pin(async move {
            c.fetch_add(1, Ordering::SeqCst);
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
    };
    (counter, cb)
}

/// On startup the watcher fires one initial drain so envelopes that
/// were stamped while the daemon was down get picked up immediately,
/// instead of waiting up to one full cadence floor (15 min).
#[tokio::test]
async fn initial_drain_fires_on_startup() {
    let (_tmp, root) = make_substrate_root();
    let (counter, cb) = install_counter();

    let _handle = spawn_watcher(root, DEFAULT_DEBOUNCE, cb);

    // Initial drain is spawned, not awaited inline, so give it a beat.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(counter.load(Ordering::SeqCst), 1, "initial drain expected");
}

/// A `.md` file appearing in an `envelopes/YYYY/MM/DD/` shard
/// (what the stamp ceremony's atomic rename produces) triggers
/// exactly one drain after the debounce window. Drains beyond the
/// initial-startup one must come from real events.
#[tokio::test]
async fn md_file_triggers_drain() {
    let (_tmp, root) = make_substrate_root();
    let day_shard = root
        .join("did_key_zsomething/channels/inbox/default/envelopes/2026/05/21");
    std::fs::create_dir_all(&day_shard).unwrap();

    let (counter, cb) = install_counter();
    let _handle = spawn_watcher(root.clone(), Duration::from_millis(80), cb);

    // Drain the initial-startup call so we measure the FS-notify path
    // in isolation.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let baseline = counter.load(Ordering::SeqCst);

    std::fs::write(day_shard.join("stamped.md"), "stamped").unwrap();

    // macOS FSEvents can take up to ~1s to deliver the first event
    // after a watcher arms. Wait generously, plus the debounce window.
    tokio::time::sleep(Duration::from_millis(2000)).await;
    let after = counter.load(Ordering::SeqCst);
    assert_eq!(
        after - baseline,
        1,
        "expected exactly one drain after one .md write (baseline {baseline} → {after})"
    );
}

/// A burst of writes inside the debounce window collapses to a single
/// drain. The whole point of the debounce: stamp + downstream tools
/// that touch the file shouldn't spam delivery attempts.
#[tokio::test]
async fn burst_is_debounced() {
    let (_tmp, root) = make_substrate_root();
    let day_shard = root.join("did_key_zburst/channels/inbox/default/envelopes/2026/05/21");
    std::fs::create_dir_all(&day_shard).unwrap();

    let (counter, cb) = install_counter();
    let _handle = spawn_watcher(root.clone(), Duration::from_millis(150), cb);

    tokio::time::sleep(Duration::from_millis(50)).await;
    let baseline = counter.load(Ordering::SeqCst);

    // Hammer the envelopes tree with five writes inside one debounce window.
    for i in 0..5 {
        std::fs::write(day_shard.join(format!("d{i}.md")), "x").unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Wait long enough for FSEvents to deliver + debounce to fire.
    tokio::time::sleep(Duration::from_millis(2000)).await;
    let after = counter.load(Ordering::SeqCst);
    assert_eq!(
        after - baseline,
        1,
        "5 writes inside debounce should collapse to 1 drain (baseline {baseline} → {after})"
    );
}

/// Writes under any `sent/` ancestor are the daemon's own post-
/// delivery archive moves. The watcher must ignore them; otherwise
/// every successful send would re-trigger the drain and loop forever.
#[tokio::test]
async fn events_in_sent_subdir_are_ignored() {
    let (_tmp, root) = make_substrate_root();
    let sent = root.join("did_key_zsent/channels/inbox/default/sent/2026/05/21");
    std::fs::create_dir_all(&sent).unwrap();

    let (counter, cb) = install_counter();
    let _handle = spawn_watcher(root.clone(), Duration::from_millis(80), cb);

    tokio::time::sleep(Duration::from_millis(50)).await;
    let baseline = counter.load(Ordering::SeqCst);

    std::fs::write(sent.join("delivered.md"), "post-send").unwrap();

    tokio::time::sleep(Duration::from_millis(2000)).await;
    let after = counter.load(Ordering::SeqCst);
    assert_eq!(
        after, baseline,
        "writes under sent/ must not trigger drain (baseline {baseline} → {after})"
    );
}

/// Writes under any `_drafts/` ancestor are unstamped drafts the
/// drain would skip anyway. The watcher must ignore them so a
/// /compose tool call doesn't fire a useless drain pass.
#[tokio::test]
async fn events_in_drafts_subdir_are_ignored() {
    let (_tmp, root) = make_substrate_root();
    let drafts = root.join("did_key_zdrafts/channels/inbox/default/_drafts");
    std::fs::create_dir_all(&drafts).unwrap();

    let (counter, cb) = install_counter();
    let _handle = spawn_watcher(root.clone(), Duration::from_millis(80), cb);

    tokio::time::sleep(Duration::from_millis(50)).await;
    let baseline = counter.load(Ordering::SeqCst);

    std::fs::write(drafts.join("draft.md"), "unstamped").unwrap();

    tokio::time::sleep(Duration::from_millis(2000)).await;
    let after = counter.load(Ordering::SeqCst);
    assert_eq!(
        after, baseline,
        "writes under _drafts/ must not trigger drain (baseline {baseline} → {after})"
    );
}

/// Non-`.md` files (lockfiles, .DS_Store, swap files) shouldn't fire
/// drains. The filter keeps the watcher quiet on busy filesystems.
#[tokio::test]
async fn non_md_files_are_ignored() {
    let (_tmp, root) = make_substrate_root();
    let recipient = root.join("did_key_znoise");
    std::fs::create_dir_all(&recipient).unwrap();

    let (counter, cb) = install_counter();
    let _handle = spawn_watcher(root.clone(), Duration::from_millis(80), cb);

    tokio::time::sleep(Duration::from_millis(50)).await;
    let baseline = counter.load(Ordering::SeqCst);

    std::fs::write(recipient.join(".DS_Store"), "junk").unwrap();
    std::fs::write(recipient.join("draft.tmp"), "swap").unwrap();

    tokio::time::sleep(Duration::from_millis(2000)).await;
    let after = counter.load(Ordering::SeqCst);
    assert_eq!(
        after, baseline,
        "non-md files must not trigger drain (baseline {baseline} → {after})"
    );
}
