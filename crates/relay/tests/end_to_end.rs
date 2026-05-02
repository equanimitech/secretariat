//! End-to-end test: spin up the relay on a random port, do the full
//! register → challenge/answer → POST inbox → GET inbox round-trip with a
//! real HTTP client.

use std::sync::Arc;

use axum::serve;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use secretariat_core::codec::encode_ed25519_multibase;
use secretariat_core::Did;
use secretariat_relay::{router, AppState, Config};
use tokio::net::TcpListener;

async fn spawn_test_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = AppState::new(Config {
        bind: addr,
        ..Config::default()
    });
    let app = router(state);
    tokio::spawn(async move {
        serve(listener, app).await.unwrap();
    });
    // Give the server a tick to start accepting connections.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    format!("http://{}", addr)
}

fn fresh_principal() -> (SigningKey, Did) {
    let key = SigningKey::generate(&mut OsRng);
    let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
    (key, did)
}

fn b64_sig(key: &SigningKey, msg: &[u8]) -> String {
    let sig = key.sign(msg);
    format!("ed25519:{}", B64.encode(sig.to_bytes()))
}

#[tokio::test]
async fn healthz_responds_ok() {
    let url = spawn_test_server().await;
    let r = reqwest::get(format!("{url}/healthz")).await.unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["ok"], true);
}

#[tokio::test]
async fn full_register_auth_post_get_roundtrip() {
    let url = spawn_test_server().await;
    let (rafa_key, rafa_did) = fresh_principal();
    let pubkey_bytes = rafa_key.verifying_key().to_bytes();
    let pubkey_mb = encode_ed25519_multibase(&pubkey_bytes);

    // Register rafa.
    let mut to_sign = b"secretariat-relay-register:v0:".to_vec();
    to_sign.extend_from_slice(rafa_did.as_str().as_bytes());
    to_sign.extend_from_slice(&pubkey_bytes);
    let sig = b64_sig(&rafa_key, &to_sign);

    let r = reqwest::Client::new()
        .post(format!("{url}/v0/register"))
        .json(&serde_json::json!({
            "did": rafa_did.as_str(),
            "pubkey_multibase": pubkey_mb,
            "signature": sig,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201, "register: {}", r.text().await.unwrap());

    // Sender (marcelo) POSTs an envelope to rafa's inbox.
    let envelope_bytes =
        b"---\n$envelope:\n  $type: tech.equanimi.secretariat.envelope\n---\n# fake body\n";
    let r = reqwest::Client::new()
        .post(format!("{url}/v0/inbox/{}", rafa_did.as_str()))
        .header("content-type", "text/markdown")
        .header("x-sender-did", "did:web:marcelo.example")
        .body(envelope_bytes.to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 202);

    // Rafa requests a challenge.
    let r = reqwest::Client::new()
        .post(format!("{url}/v0/auth/challenge"))
        .json(&serde_json::json!({ "did": rafa_did.as_str() }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    let nonce = body["nonce"].as_str().unwrap().to_string();

    // Rafa signs the auth input and answers.
    let mut auth_input = b"secretariat-relay-auth:v0:".to_vec();
    auth_input.extend_from_slice(nonce.as_bytes());
    let sig = b64_sig(&rafa_key, &auth_input);

    let r = reqwest::Client::new()
        .post(format!("{url}/v0/auth/answer"))
        .json(&serde_json::json!({
            "did": rafa_did.as_str(),
            "nonce": nonce,
            "signature": sig,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "auth answer: {}", r.text().await.unwrap());
    let body: serde_json::Value = r.json().await.unwrap();
    let token = body["token"].as_str().unwrap().to_string();

    // Rafa polls the inbox with the bearer token.
    let r = reqwest::Client::new()
        .get(format!("{url}/v0/inbox/{}", rafa_did.as_str()))
        .header("authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    let envelopes = body["envelopes"].as_array().unwrap();
    assert_eq!(envelopes.len(), 1);
    assert_eq!(envelopes[0]["sender_did"], "did:web:marcelo.example");

    // The body is base64'd; decoding should give back the original bytes.
    let body_b64 = envelopes[0]["body"].as_str().unwrap();
    let decoded = B64.decode(body_b64).unwrap();
    assert_eq!(decoded, envelope_bytes);
}

#[tokio::test]
async fn cannot_register_with_someone_elses_signature() {
    let url = spawn_test_server().await;
    let (rafa_key, rafa_did) = fresh_principal();
    let (marcelo_key, _) = fresh_principal();
    let pubkey_bytes = rafa_key.verifying_key().to_bytes();
    let pubkey_mb = encode_ed25519_multibase(&pubkey_bytes);

    // Marcelo signs rafa's registration message — should fail.
    let mut to_sign = b"secretariat-relay-register:v0:".to_vec();
    to_sign.extend_from_slice(rafa_did.as_str().as_bytes());
    to_sign.extend_from_slice(&pubkey_bytes);
    let sig = b64_sig(&marcelo_key, &to_sign);

    let r = reqwest::Client::new()
        .post(format!("{url}/v0/register"))
        .json(&serde_json::json!({
            "did": rafa_did.as_str(),
            "pubkey_multibase": pubkey_mb,
            "signature": sig,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn poll_with_wrong_token_is_forbidden() {
    let url = spawn_test_server().await;
    let (rafa_key, rafa_did) = fresh_principal();
    let pubkey_mb = encode_ed25519_multibase(&rafa_key.verifying_key().to_bytes());

    // Register rafa.
    let mut to_sign = b"secretariat-relay-register:v0:".to_vec();
    to_sign.extend_from_slice(rafa_did.as_str().as_bytes());
    to_sign.extend_from_slice(&rafa_key.verifying_key().to_bytes());
    let sig = b64_sig(&rafa_key, &to_sign);
    reqwest::Client::new()
        .post(format!("{url}/v0/register"))
        .json(&serde_json::json!({
            "did": rafa_did.as_str(),
            "pubkey_multibase": pubkey_mb,
            "signature": sig,
        }))
        .send()
        .await
        .unwrap();

    // Try to poll without a token.
    let r = reqwest::Client::new()
        .get(format!("{url}/v0/inbox/{}", rafa_did.as_str()))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);

    // Try with a bogus token.
    let r = reqwest::Client::new()
        .get(format!("{url}/v0/inbox/{}", rafa_did.as_str()))
        .header("authorization", "Bearer not-a-real-token")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn post_to_unregistered_recipient_returns_404() {
    let url = spawn_test_server().await;
    let (_, ghost_did) = fresh_principal();

    let r = reqwest::Client::new()
        .post(format!("{url}/v0/inbox/{}", ghost_did.as_str()))
        .body(b"hello".to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
}

// We use Arc just so the helper has a strong reason to import it via the path.
#[allow(dead_code)]
fn _arc_in_scope(_: Arc<u8>) {}
