//! End-to-end test: drive the actual `RelayClient` from `secretariat-core`
//! against an in-process relay binary. Validates the client and server speak
//! the same wire spec without manually constructing JSON.

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use secretariat_core::domain::QueueHandle;
use secretariat_core::infrastructure::transport::RelayClient;
use secretariat_core::Did;
use secretariat_relay::{router, AppState, Config};
use tokio::net::TcpListener;

fn dm() -> QueueHandle {
    QueueHandle::parse("inbox:default").unwrap()
}

async fn spawn_test_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = AppState::new(Config {
        bind: addr,
        ..Config::default()
    });
    let app = router(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    format!("http://{}", addr)
}

fn fresh_principal() -> (SigningKey, Did) {
    let key = SigningKey::generate(&mut OsRng);
    let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
    (key, did)
}

#[tokio::test]
async fn rafa_sends_marcelo_receives() {
    let url = spawn_test_server().await;
    let (rafa_key, rafa_did) = fresh_principal();
    let (marcelo_key, marcelo_did) = fresh_principal();

    let rafa_client = RelayClient::new(url.clone(), rafa_did.clone(), &rafa_key);
    let marcelo_client = RelayClient::new(url.clone(), marcelo_did.clone(), &marcelo_key);

    // Both register.
    rafa_client.register().await.unwrap();
    marcelo_client.register().await.unwrap();

    // Rafa sends an envelope to Marcelo.
    let envelope_bytes =
        b"---\n$envelope:\n  $type: tech.equanimi.secretariat.envelope\n  from: did:web:rafa.equanimi.tech\n  encryption: x25519-xchacha20poly1305\n---\nx25519:fakeb64:fakeb64:fakeb64\n";
    let id = rafa_client
        .send_channel(&marcelo_did, &dm(), envelope_bytes, "text/markdown")
        .await
        .unwrap();
    assert!(id >= 1);

    // Marcelo authenticates and polls.
    let (token, _expires) = marcelo_client.authenticate().await.unwrap();
    let inbound = marcelo_client
        .poll_channel(&marcelo_did, &dm(), &token, 0)
        .await
        .unwrap();
    assert_eq!(inbound.len(), 1);
    let env = &inbound[0];
    assert_eq!(env.body, envelope_bytes);
    assert_eq!(env.sender_did.as_ref().map(|d| d.as_str()), Some(rafa_did.as_str()));
}

#[tokio::test]
async fn duplicate_register_is_treated_as_success() {
    let url = spawn_test_server().await;
    let (key, did) = fresh_principal();
    let client = RelayClient::new(url, did, &key);

    client.register().await.unwrap();
    // Second register should not error (server returns 409, client treats as OK).
    client.register().await.unwrap();
}

#[tokio::test]
async fn poll_advances_via_cursor() {
    let url = spawn_test_server().await;
    let (rafa_key, rafa_did) = fresh_principal();
    let (marcelo_key, marcelo_did) = fresh_principal();

    let rafa_client = RelayClient::new(url.clone(), rafa_did.clone(), &rafa_key);
    let marcelo_client = RelayClient::new(url.clone(), marcelo_did.clone(), &marcelo_key);

    rafa_client.register().await.unwrap();
    marcelo_client.register().await.unwrap();

    // Send three.
    for i in 0..3 {
        let body = format!("envelope-{i}");
        rafa_client
            .send_channel(&marcelo_did, &dm(), body.as_bytes(), "text/markdown")
            .await
            .unwrap();
    }

    let (token, _) = marcelo_client.authenticate().await.unwrap();

    // First poll: get all three.
    let first = marcelo_client
        .poll_channel(&marcelo_did, &dm(), &token, 0)
        .await
        .unwrap();
    assert_eq!(first.len(), 3);
    let last_id = first.iter().map(|e| e.id).max().unwrap();

    // Second poll: nothing new.
    let second = marcelo_client
        .poll_channel(&marcelo_did, &dm(), &token, last_id)
        .await
        .unwrap();
    assert!(second.is_empty());

    // Sender posts one more.
    rafa_client
        .send_channel(&marcelo_did, &dm(), b"envelope-3", "text/markdown")
        .await
        .unwrap();

    // Third poll: just the new one.
    let third = marcelo_client
        .poll_channel(&marcelo_did, &dm(), &token, last_id)
        .await
        .unwrap();
    assert_eq!(third.len(), 1);
    assert_eq!(third[0].body, b"envelope-3");
}
