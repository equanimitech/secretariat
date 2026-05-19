//! Persistence — registry / queues / invites survive a relay restart when
//! `data_dir` is configured.
//!
//! Strategy: spawn a relay on port A with `data_dir = tmpdir`; do mutations;
//! shut it down; spawn a *fresh* relay on port B pointing at the same
//! `tmpdir`; assert the state was rehydrated.

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use secretariat_core::application::{create_invite, view_invite};
use secretariat_core::domain::QueueHandle;
use secretariat_core::infrastructure::transport::RelayClient;
use secretariat_core::Did;
use secretariat_relay::{router, AppState, Config};

fn dm() -> QueueHandle {
    QueueHandle::parse("inbox:default").unwrap()
}
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

struct RelayServer {
    url: String,
    handle: JoinHandle<()>,
}

impl RelayServer {
    async fn spawn(data_dir: std::path::PathBuf) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let state = AppState::load(Config {
            bind: addr,
            data_dir: Some(data_dir),
            ..Config::default()
        })
        .expect("load relay state");
        let app = router(state);
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        Self {
            url: format!("http://{}", addr),
            handle,
        }
    }

    async fn shutdown(self) {
        self.handle.abort();
        // give axum a moment to release the socket
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

fn fresh_principal() -> (SigningKey, Did) {
    let key = SigningKey::generate(&mut OsRng);
    let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
    (key, did)
}

#[tokio::test]
async fn registration_survives_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    // Boot 1: register Rafa.
    let server1 = RelayServer::spawn(path.clone()).await;
    let (rafa_key, rafa_did) = fresh_principal();
    let client = RelayClient::new(server1.url.clone(), rafa_did.clone(), &rafa_key);
    client.register().await.unwrap();
    server1.shutdown().await;

    // Boot 2: fresh server, same data_dir. Rafa should still be registered.
    let server2 = RelayServer::spawn(path.clone()).await;
    let body = reqwest::get(format!("{}/healthz", server2.url))
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(body["registered_count"].as_u64(), Some(1));

    // Authenticate with the previously-stored pubkey: proves the relay
    // really has Rafa's pubkey on file (not just a placeholder).
    let client2 = RelayClient::new(server2.url.clone(), rafa_did.clone(), &rafa_key);
    let (_token, _expiry) = client2.authenticate().await.unwrap();
    server2.shutdown().await;
}

#[tokio::test]
async fn queued_envelope_survives_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let (rafa_key, rafa_did) = fresh_principal();
    let (marcelo_key, marcelo_did) = fresh_principal();

    // Boot 1: both register; rafa POSTs an envelope to marcelo's inbox.
    let server1 = RelayServer::spawn(path.clone()).await;
    let rafa_client = RelayClient::new(server1.url.clone(), rafa_did.clone(), &rafa_key);
    let marcelo_client = RelayClient::new(server1.url.clone(), marcelo_did.clone(), &marcelo_key);
    rafa_client.register().await.unwrap();
    marcelo_client.register().await.unwrap();

    let envelope_bytes = b"---\n$envelope:\n  $type: tech.equanimi.secretariat.envelope\n---\nhello\n";
    rafa_client
        .send(&marcelo_did, &dm(), envelope_bytes, "text/markdown")
        .await
        .unwrap();
    server1.shutdown().await;

    // Boot 2: marcelo polls. The envelope must still be in his queue.
    let server2 = RelayServer::spawn(path.clone()).await;
    let marcelo_client2 =
        RelayClient::new(server2.url.clone(), marcelo_did.clone(), &marcelo_key);
    let (token, _) = marcelo_client2.authenticate().await.unwrap();
    let inbound = marcelo_client2
        .poll(&marcelo_did, &dm(), &token, 0)
        .await
        .unwrap();
    assert_eq!(inbound.len(), 1);
    assert_eq!(inbound[0].body, envelope_bytes);
    server2.shutdown().await;
}

#[tokio::test]
async fn invite_token_survives_restart() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();

    let (rafa_key, rafa_did) = fresh_principal();

    // Boot 1: Rafa registers + creates invite.
    let server1 = RelayServer::spawn(path.clone()).await;
    let rafa_client = RelayClient::new(server1.url.clone(), rafa_did.clone(), &rafa_key);
    rafa_client.register().await.unwrap();

    let url1 = server1.url.clone();
    let did1 = rafa_did.clone();
    let key1 = rafa_key.clone();
    let invite =
        tokio::task::spawn_blocking(move || create_invite(&url1, &did1, &key1, Some("hi"), Some(24)))
            .await
            .unwrap()
            .unwrap();
    server1.shutdown().await;

    // Boot 2: viewing the invite by token should still work.
    let server2 = RelayServer::spawn(path.clone()).await;
    let claim_url = format!("{}/v0/invite/{}", server2.url, invite.token);
    let preview = tokio::task::spawn_blocking(move || view_invite(&claim_url))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(preview.purpose.as_deref(), Some("hi"));
    assert!(preview.claimed_by.is_none());
    server2.shutdown().await;
}

// Anchor unused import for `Arc` — used to keep the `tokio::task::spawn_blocking`
// closures plus the `RelayServer` ergonomic across tests.
#[allow(dead_code)]
fn _arc_anchor(_: Arc<u8>) {}
