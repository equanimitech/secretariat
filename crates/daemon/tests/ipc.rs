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
