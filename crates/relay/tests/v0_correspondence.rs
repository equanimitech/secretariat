//! v0 round-trip — the milestone-gating end-to-end test.
//!
//! Two principals (Rafa + Marcelo) with their own ed25519 keys. An
//! in-process relay binary. Rafa composes an encrypted envelope to Marcelo,
//! stamps it (signature over the ciphertext-bytes body), sends via the
//! relay. Marcelo polls the relay, receives the bytes, parses the envelope,
//! verifies the signature against Rafa's resolved DID, decrypts the body
//! with his X25519 secret, and reads the plaintext.
//!
//! Per `docs/milestones/2026-05-02-v0-correspondence.md`, this is the
//! acceptance criterion: the *correspondence loop* — not just the stamp —
//! works end-to-end with sovereignty intact (keys local, body sealed,
//! relay sees ciphertext only).

use chrono::Utc;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use secretariat_core::application::stamp_document;
use secretariat_core::domain::EnvelopeBuilder;
use secretariat_core::infrastructure::crypto::sealed::{
    open, pubkey_to_x25519, seal, signing_to_x25519, SealedBox,
};
use secretariat_core::infrastructure::ed25519_signer::{AlwaysAllowGate, Ed25519Signer};
use secretariat_core::infrastructure::markdown::{embed_stamp, parse_document};
use secretariat_core::infrastructure::transport::RelayClient;
use secretariat_core::{Did, EncryptionScheme, Envelope, EnvelopeDepth, EnvelopeUrgency};
use secretariat_relay::{router, AppState, Config};
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::net::TcpListener;

async fn spawn_relay() -> String {
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

/// Compose an encrypted envelope from rafa to marcelo, stamp it, return the
/// markdown bytes ready for transport.
fn compose_and_stamp(
    rafa_key: &SigningKey,
    rafa_did: &Did,
    marcelo_did: &Did,
    marcelo_pubkey: &VerifyingKey,
    plaintext: &[u8],
    outbox_root: &std::path::Path,
) -> PathBuf {
    // Derive marcelo's X25519 pubkey from his ed25519 verifying key, then seal.
    let marcelo_x25519_pub = pubkey_to_x25519(marcelo_pubkey);
    let sealed = seal(plaintext, &marcelo_x25519_pub).unwrap();
    let body_wire = sealed.to_wire_string();

    // Build envelope w/ encryption marker.
    let envelope: Envelope = EnvelopeBuilder::new(rafa_did.clone())
        .to(marcelo_did.clone())
        .depth(EnvelopeDepth::Subtle)
        .urgency(EnvelopeUrgency::Whenever)
        .source("v0-correspondence-test")
        .encryption(EncryptionScheme::X25519XChaCha20Poly1305)
        .build();

    // Embed envelope (no stamp yet) into markdown.
    let unstamped = embed_stamp(&body_wire, Some(&envelope), None).unwrap();

    // Write to outbox; stamp_document operates on the file in place.
    let recipient_dir = outbox_root.join(marcelo_did.as_str().replace([':', '/'], "_"));
    std::fs::create_dir_all(&recipient_dir).unwrap();
    let path = recipient_dir.join(format!("{}-test.md", Utc::now().format("%Y-%m-%dT%H-%M-%SZ")));
    std::fs::write(&path, unstamped).unwrap();

    // Stamp it (test biometric gate always allows).
    let signer = Ed25519Signer::new(rafa_did.clone(), rafa_key.clone(), AlwaysAllowGate);
    let _outcome = stamp_document(
        &path,
        &signer,
        secretariat_core::StampAct::Attest,
        false,
        Utc::now(),
    )
    .expect("stamp should succeed");

    path
}

#[tokio::test]
async fn rafa_to_marcelo_full_correspondence_loop() {
    // 1. Spin up the relay.
    let relay_url = spawn_relay().await;

    // 2. Bootstrap two principals.
    let (rafa_key, rafa_did) = fresh_principal();
    let (marcelo_key, marcelo_did) = fresh_principal();

    let rafa_outbox_dir = TempDir::new().unwrap();
    let marcelo_inbox_dir = TempDir::new().unwrap();

    // 3. Both register with the relay.
    let rafa_client = RelayClient::new(relay_url.clone(), rafa_did.clone(), &rafa_key);
    let marcelo_client = RelayClient::new(relay_url.clone(), marcelo_did.clone(), &marcelo_key);
    rafa_client.register().await.unwrap();
    marcelo_client.register().await.unwrap();

    // 4. Rafa needs marcelo's pubkey to encrypt to. For did:key it's
    //    derivable from the DID directly (no live resolution needed).
    let marcelo_verifying = marcelo_key.verifying_key();

    // 5. Rafa composes + encrypts + stamps.
    let plaintext = b"# ch7 push-back\n\nstaff vs. tools - agency != accountability\n";
    let stamped_path = compose_and_stamp(
        &rafa_key,
        &rafa_did,
        &marcelo_did,
        &marcelo_verifying,
        plaintext,
        rafa_outbox_dir.path(),
    );

    // 6. Sender's daemon (simulated): read stamped file, send via relay.
    let stamped_bytes = std::fs::read(&stamped_path).unwrap();
    let sent_id = rafa_client
        .send(&marcelo_did, &stamped_bytes, "text/markdown")
        .await
        .unwrap();
    assert!(sent_id >= 1);

    // 7. Recipient's daemon (simulated): authenticate + poll.
    let (token, _expiry) = marcelo_client.authenticate().await.unwrap();
    let inbound = marcelo_client.poll(&token, 0).await.unwrap();
    assert_eq!(inbound.len(), 1, "marcelo should see exactly one envelope");

    let inbound_env = &inbound[0];
    assert_eq!(inbound_env.body, stamped_bytes, "wire bytes survive transit");

    // File to marcelo's inbox.
    let inbox_filename = format!("v0-test-id{:06}.md", inbound_env.id);
    let inbox_path = marcelo_inbox_dir.path().join(&inbox_filename);
    std::fs::write(&inbox_path, &inbound_env.body).unwrap();

    // 8. Marcelo reads the inbox file: parse + verify hash + decrypt.
    let raw_str = std::fs::read_to_string(&inbox_path).unwrap();
    let parsed = parse_document(&raw_str).unwrap();
    let envelope = parsed.envelope.expect("envelope present after transit");
    assert_eq!(envelope.from, rafa_did);
    assert_eq!(envelope.to.as_ref(), Some(&marcelo_did));
    assert!(envelope.is_encrypted());

    // Stamp present and matches the body's hash invariant.
    let stamp = parsed.stamp.expect("stamp present after transit");
    assert_eq!(
        stamp.doc_hash,
        secretariat_core::domain::canonical_body_hash(&parsed.body),
        "hash invariant must hold post-transit"
    );

    // Decrypt with marcelo's signing key.
    let marcelo_x25519_secret = signing_to_x25519(&marcelo_key);
    let sealed = SealedBox::parse_wire_string(parsed.body.trim()).unwrap();
    let opened = open(&sealed, &marcelo_x25519_secret).unwrap();
    assert_eq!(opened, plaintext, "round-trip plaintext recovery");
}

#[tokio::test]
async fn tampered_in_transit_rejects_on_verify() {
    let relay_url = spawn_relay().await;
    let (rafa_key, rafa_did) = fresh_principal();
    let (marcelo_key, marcelo_did) = fresh_principal();

    let rafa_outbox_dir = TempDir::new().unwrap();
    let rafa_client = RelayClient::new(relay_url.clone(), rafa_did.clone(), &rafa_key);
    let marcelo_client = RelayClient::new(relay_url.clone(), marcelo_did.clone(), &marcelo_key);
    rafa_client.register().await.unwrap();
    marcelo_client.register().await.unwrap();

    let marcelo_verifying = marcelo_key.verifying_key();

    let plaintext = b"original";
    let stamped_path = compose_and_stamp(
        &rafa_key,
        &rafa_did,
        &marcelo_did,
        &marcelo_verifying,
        plaintext,
        rafa_outbox_dir.path(),
    );
    let stamped_bytes = std::fs::read(&stamped_path).unwrap();

    // Tamper before sending: flip a bit in the body region.
    let mut tampered = stamped_bytes.clone();
    let last = tampered.len() - 5;
    tampered[last] ^= 1;

    rafa_client
        .send(&marcelo_did, &tampered, "text/markdown")
        .await
        .unwrap();

    let (token, _) = marcelo_client.authenticate().await.unwrap();
    let inbound = marcelo_client.poll(&token, 0).await.unwrap();
    assert_eq!(inbound.len(), 1);
    let raw_str = std::str::from_utf8(&inbound[0].body).unwrap();
    let parsed = parse_document(raw_str).unwrap();
    let stamp = parsed.stamp.unwrap();

    // Hash invariant breaks because the body was modified mid-flight.
    let observed_hash = secretariat_core::domain::canonical_body_hash(&parsed.body);
    assert_ne!(
        stamp.doc_hash, observed_hash,
        "tampered body must yield a different hash than the stamp asserts"
    );
}

#[tokio::test]
async fn wrong_recipient_cannot_decrypt() {
    let relay_url = spawn_relay().await;
    let (rafa_key, rafa_did) = fresh_principal();
    let (marcelo_key, marcelo_did) = fresh_principal();
    let (eve_key, _eve_did) = fresh_principal();

    let rafa_outbox_dir = TempDir::new().unwrap();
    let rafa_client = RelayClient::new(relay_url.clone(), rafa_did.clone(), &rafa_key);
    let marcelo_client = RelayClient::new(relay_url.clone(), marcelo_did.clone(), &marcelo_key);
    rafa_client.register().await.unwrap();
    marcelo_client.register().await.unwrap();

    let marcelo_verifying = marcelo_key.verifying_key();

    let plaintext = b"for marcelo only";
    let stamped_path = compose_and_stamp(
        &rafa_key,
        &rafa_did,
        &marcelo_did,
        &marcelo_verifying,
        plaintext,
        rafa_outbox_dir.path(),
    );
    let stamped_bytes = std::fs::read(&stamped_path).unwrap();
    rafa_client
        .send(&marcelo_did, &stamped_bytes, "text/markdown")
        .await
        .unwrap();

    let (token, _) = marcelo_client.authenticate().await.unwrap();
    let inbound = marcelo_client.poll(&token, 0).await.unwrap();
    let raw_str = std::str::from_utf8(&inbound[0].body).unwrap();
    let parsed = parse_document(raw_str).unwrap();

    // Eve gets her hands on the bytes (steals from disk, sniffs network, etc.)
    // and tries to decrypt with her own X25519 secret. Should fail.
    let eve_x25519_secret = signing_to_x25519(&eve_key);
    let sealed = SealedBox::parse_wire_string(parsed.body.trim()).unwrap();
    let r = open(&sealed, &eve_x25519_secret);
    assert!(r.is_err(), "eve must not be able to decrypt");
}
