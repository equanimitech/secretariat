//! End-to-end smoke test for the Slice 1 IPC socket. Starts the server
//! task against a temp `SECRETARIAT_HOME`-equivalent root, drives it
//! with the client, checks `ping` / `version` round-trip and that
//! unknown methods surface the expected `-32601`.

use ed25519_dalek::SigningKey;
use secretariat_core::infrastructure::keys::KeyPaths;
use secretariat_core::Did;
use secretariat_daemon::ipc::{call, is_running, socket_path, spawn_server};
use std::time::Duration;

fn fixture(seed: u8) -> (tempfile::TempDir, KeyPaths, Did, SigningKey) {
    let tmp = tempfile::tempdir().unwrap();
    let paths = KeyPaths::under(tmp.path().to_path_buf());
    paths.ensure_dirs().unwrap();
    let key = SigningKey::from_bytes(&[seed; 32]);
    let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
    (tmp, paths, did, key)
}

async fn wait_for_socket(paths: &KeyPaths) {
    for _ in 0..50 {
        if is_running(paths).await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!(
        "ipc socket never came up at {}",
        socket_path(paths).display()
    );
}

#[tokio::test]
async fn ping_round_trips() {
    let (_tmp, paths, did, key) = fixture(0x11);
    let _handle = spawn_server(paths.clone(), did, key);
    wait_for_socket(&paths).await;

    let res = call(&paths, "ping", None).await.expect("ping call");
    assert_eq!(res, serde_json::json!({ "ok": true }));
}

#[tokio::test]
async fn version_returns_cargo_pkg_version() {
    let (_tmp, paths, did, key) = fixture(0x22);
    let _handle = spawn_server(paths.clone(), did, key);
    wait_for_socket(&paths).await;

    let res = call(&paths, "version", None).await.expect("version call");
    let version = res
        .get("version")
        .and_then(|v| v.as_str())
        .expect("version field");
    assert!(!version.is_empty());
}

#[tokio::test]
async fn unknown_method_yields_rpc_error() {
    let (_tmp, paths, did, key) = fixture(0x33);
    let _handle = spawn_server(paths.clone(), did, key);
    wait_for_socket(&paths).await;

    let err = call(&paths, "does_not_exist", None)
        .await
        .expect_err("should error");
    let msg = err.to_string();
    assert!(msg.contains("-32601"), "got: {msg}");
}

#[tokio::test]
async fn is_running_false_when_no_daemon() {
    let (_tmp, paths, _did, _key) = fixture(0x44);
    assert!(!is_running(&paths).await);
}

/// Two concurrent `tick` requests must serialize cleanly. Without the
/// `tick_lock` in `serve.rs`, two calls would interleave their
/// `RelayState` load/mutate/save and risk clobbering the cursor; with
/// it, they run back-to-back and both return a valid `SyncOutcome`.
/// This test verifies no deadlock + both succeed; it doesn't try to
/// observe the race itself, which is non-deterministic anyway.
#[tokio::test]
async fn concurrent_ticks_serialize() {
    let (_tmp, paths, did, key) = fixture(0x55);
    let _handle = spawn_server(paths.clone(), did, key);
    wait_for_socket(&paths).await;

    let p1 = paths.clone();
    let p2 = paths.clone();
    let p3 = paths.clone();
    let p4 = paths.clone();
    let (a, b, c, d) = tokio::join!(
        async move { call(&p1, "tick", None).await },
        async move { call(&p2, "tick", None).await },
        async move { call(&p3, "tick", None).await },
        async move { call(&p4, "tick", None).await },
    );
    for r in [a, b, c, d] {
        let v = r.expect("tick call should succeed");
        assert!(v.get("per_relay").is_some(), "unexpected outcome: {v}");
    }
}

/// `tick_via_ipc_or_inproc` with no daemon running must fall back to
/// running the cycle in-proc. Tauri's `sync_now` and `sec daemon tick`
/// both rely on this fallback when the LaunchAgent isn't installed.
#[tokio::test]
async fn fallback_runs_in_proc_when_no_daemon() {
    let (_tmp, paths, did, key) = fixture(0x66);
    assert!(!is_running(&paths).await);
    secretariat_daemon::ipc::tick_via_ipc_or_inproc(&paths, &did, &key)
        .await
        .expect("in-proc fallback should succeed against an empty install");
}

/// A stale socket file (file at `socket_path` that nothing is listening
/// on — e.g. a previous daemon crashed without cleaning up) must not
/// block startup. The server should detect the file is dead via
/// connect-probe and recreate it. Exercises the cleanup branch of the
/// startup logic.
#[tokio::test]
async fn stale_socket_file_is_reclaimed() {
    let (_tmp, paths, did, key) = fixture(0x88);

    // Plant a stale file at the socket path. A bare file, not a real
    // socket — exactly the residue a crashed daemon leaves behind.
    std::fs::write(socket_path(&paths), b"stale residue").unwrap();
    assert!(socket_path(&paths).exists());

    let _handle = spawn_server(paths.clone(), did, key);
    wait_for_socket(&paths).await;

    // The server must have detected the stale file, unlinked it, and
    // bound a real listener. A round-trip ping proves it's live.
    let res = call(&paths, "ping", None).await.expect("ping after stale");
    assert_eq!(res, serde_json::json!({ "ok": true }));
}

/// Malformed JSON on the wire must produce a structured `-32700` error
/// response, not a closed connection. Protects the parse-error handling
/// path that the accept loop otherwise only surfaces via `warn!`.
#[tokio::test]
async fn malformed_request_yields_parse_error() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let (_tmp, paths, did, key) = fixture(0x77);
    let _handle = spawn_server(paths.clone(), did, key);
    wait_for_socket(&paths).await;

    let mut stream = UnixStream::connect(socket_path(&paths)).await.unwrap();
    stream.write_all(b"this is not json\n").await.unwrap();
    stream.flush().await.unwrap();

    let (rd, _wr) = stream.into_split();
    let mut rd = BufReader::new(rd);
    let mut line = String::new();
    rd.read_line(&mut line).await.unwrap();

    let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    let code = v
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_i64())
        .expect("error.code");
    assert_eq!(code, -32700, "expected PARSE_ERROR, got: {v}");
}
