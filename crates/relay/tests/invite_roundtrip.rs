//! Invite primitive end-to-end: inviter creates → claimant views + claims →
//! relay auto-registers claimant + records the claim.
//!
//! Drives the actual `create_invite` / `view_invite` / `claim_invite`
//! application functions against an in-process relay binary, so this is a
//! single check that the wire-format strings (domain-separation tags, JSON
//! field names) line up between client and server.

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use secretariat_core::application::{claim_invite, create_invite, view_invite, OrgInviteContext};
use secretariat_core::infrastructure::transport::RelayClient;
use secretariat_core::Did;
use secretariat_relay::{router, AppState, Config};
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

#[tokio::test]
async fn create_view_claim_roundtrip() {
    let endpoint = spawn_relay().await;
    let (rafa_key, rafa_did) = fresh_principal();
    let (marcelo_key, marcelo_did) = fresh_principal();

    // Inviter (Rafa) registers first — required for create.
    let rafa_client = RelayClient::new(endpoint.clone(), rafa_did.clone(), &rafa_key);
    rafa_client.register().await.unwrap();

    // Inviter creates invite.
    let endpoint_for_create = endpoint.clone();
    let invite = tokio::task::spawn_blocking(move || {
        create_invite(
            &endpoint_for_create,
            &rafa_did,
            &rafa_key,
            Some("first-contact"),
            Some(24),
            None,
        )
    })
    .await
    .unwrap()
    .unwrap();
    assert!(invite.claim_url.contains("/v0/invite/"));

    // Anyone can preview without auth.
    let claim_url_for_view = invite.claim_url.clone();
    let preview = tokio::task::spawn_blocking(move || view_invite(&claim_url_for_view))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(preview.purpose.as_deref(), Some("first-contact"));
    assert!(preview.claimed_by.is_none());

    // Claimant (Marcelo) — note: NOT pre-registered with the relay. The
    // claim itself should auto-register him.
    let claim_url_for_claim = invite.claim_url.clone();
    let marcelo_did_clone = marcelo_did.clone();
    let claimed = tokio::task::spawn_blocking(move || {
        claim_invite(&claim_url_for_claim, &marcelo_did_clone, &marcelo_key)
    })
    .await
    .unwrap()
    .unwrap();
    assert!(claimed.registered, "claim should auto-register claimant");
    assert_eq!(claimed.claimant_did, marcelo_did);

    // Second view shows the invite as claimed.
    let claim_url_for_view2 = invite.claim_url.clone();
    let preview2 = tokio::task::spawn_blocking(move || view_invite(&claim_url_for_view2))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(preview2.claimed_by, Some(marcelo_did));
}

#[tokio::test]
async fn double_claim_is_rejected() {
    let endpoint = spawn_relay().await;
    let (rafa_key, rafa_did) = fresh_principal();
    let (marcelo_key, marcelo_did) = fresh_principal();
    let (eve_key, eve_did) = fresh_principal();

    let rafa_client = RelayClient::new(endpoint.clone(), rafa_did.clone(), &rafa_key);
    rafa_client.register().await.unwrap();

    let endpoint_for_create = endpoint.clone();
    let invite = tokio::task::spawn_blocking(move || {
        create_invite(&endpoint_for_create, &rafa_did, &rafa_key, None, Some(1), None)
    })
    .await
    .unwrap()
    .unwrap();

    // Marcelo claims first.
    let url1 = invite.claim_url.clone();
    let did1 = marcelo_did.clone();
    let _ = tokio::task::spawn_blocking(move || claim_invite(&url1, &did1, &marcelo_key))
        .await
        .unwrap()
        .unwrap();

    // Eve tries to claim with a DIFFERENT key — must be rejected.
    let url2 = invite.claim_url.clone();
    let did2 = eve_did;
    let r = tokio::task::spawn_blocking(move || claim_invite(&url2, &did2, &eve_key))
        .await
        .unwrap();
    assert!(r.is_err(), "second claim must fail");
}

#[tokio::test]
async fn org_invite_roundtrips_with_context() {
    // Org-flavored invite carries org_did + role + channel handles through
    // the full create → view → claim cycle. v1 signature canonicalization
    // covers the new fields; both ends compute identical preimages.
    let endpoint = spawn_relay().await;
    let (rafa_key, rafa_did) = fresh_principal();
    let (marcelo_key, marcelo_did) = fresh_principal();

    let rafa_client = RelayClient::new(endpoint.clone(), rafa_did.clone(), &rafa_key);
    rafa_client.register().await.unwrap();

    // Synthetic org DID — would be did:web:equanimi.tech in production.
    let (_, org_did) = fresh_principal();

    let org_ctx = OrgInviteContext {
        org_did: org_did.clone(),
        org_alias: "equanimi.tech".to_string(),
        role: "publish".to_string(),
        channel_handles: vec!["dev:secretariat".to_string(), "book".to_string()],
        channel_relay_endpoint: Some(endpoint.clone()),
    };

    let endpoint_for_create = endpoint.clone();
    let org_ctx_for_create = org_ctx.clone();
    let invite = tokio::task::spawn_blocking(move || {
        create_invite(
            &endpoint_for_create,
            &rafa_did,
            &rafa_key,
            Some("join equanimi.tech"),
            Some(24),
            Some(&org_ctx_for_create),
        )
    })
    .await
    .unwrap()
    .unwrap();

    // View should surface the org context.
    let claim_url_view = invite.claim_url.clone();
    let view = tokio::task::spawn_blocking(move || view_invite(&claim_url_view))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(view.org_did.as_ref().unwrap(), &org_did);
    assert_eq!(view.org_alias.as_deref(), Some("equanimi.tech"));
    assert_eq!(view.role.as_deref(), Some("publish"));
    assert_eq!(view.channel_handles, vec!["dev:secretariat", "book"]);
    assert!(view.channel_relay_endpoint.is_some());

    // Claim should surface the same org context.
    let claim_url = invite.claim_url.clone();
    let did_for_claim = marcelo_did.clone();
    let claim = tokio::task::spawn_blocking(move || {
        claim_invite(&claim_url, &did_for_claim, &marcelo_key)
    })
    .await
    .unwrap()
    .unwrap();
    assert_eq!(claim.org_did.as_ref().unwrap(), &org_did);
    assert_eq!(claim.role.as_deref(), Some("publish"));
    assert_eq!(claim.channel_handles, vec!["dev:secretariat", "book"]);
}

#[tokio::test]
async fn unregistered_inviter_cannot_create() {
    let endpoint = spawn_relay().await;
    let (rafa_key, rafa_did) = fresh_principal();
    // Note: NOT calling rafa_client.register() first.

    let endpoint_for_create = endpoint.clone();
    let r = tokio::task::spawn_blocking(move || {
        create_invite(&endpoint_for_create, &rafa_did, &rafa_key, None, Some(1), None)
    })
    .await
    .unwrap();
    assert!(r.is_err(), "create must fail for unregistered inviter");
}
