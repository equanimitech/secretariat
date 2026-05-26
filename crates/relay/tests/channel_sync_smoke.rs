//! v0.8 channel sync — end-to-end smoke tests.
//!
//! These tests exercise the post-collapse channel substrate as a whole,
//! not individual layers. Two shapes:
//!
//! 1. **Wire-level smoke** (`org_channel_wire_roundtrip`) — owner posts
//!    to `(org_did, handle)` via `RelayClient::send`, subscriber pulls
//!    via `RelayClient::poll`. No daemon, no org dirs on disk, no
//!    membership files. Validates the HTTP route + single queue index
//!    + per-queue cursor in isolation.
//!
//! 2. **Daemon-level smoke** (`org_channel_daemon_sync_files_inbound`) —
//!    full receiver-side path: subscriber's vault hand-fixtured with an
//!    org dir + `membership.local.md` + a `channel.md` marker, then
//!    `sync_now` runs. The daemon's enumeration walks the orgs tree,
//!    finds the channel, polls the relay, files the envelope to
//!    disk under the right queue dir. Validates the new sync engine
//!    end-to-end without programmatic `accept_org_invite` (which lands
//!    next session — until then, the receiver substrate is testable
//!    via hand-fixtured state).

use chrono::Utc;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use secretariat_core::application::sync_now;
use secretariat_core::domain::{Org, OrgAlias, QueueHandle, RelayEndpoint};
use secretariat_core::infrastructure::membership_store::{save_membership, OrgMembership};
use secretariat_core::infrastructure::org_store::save_org;
use secretariat_core::infrastructure::transport::{RelayClient, RelayState};
use secretariat_core::infrastructure::KeyPaths;
use secretariat_core::Did;
use secretariat_relay::{router, AppState, Config};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;

async fn spawn_relay() -> (String, Arc<AppState>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = AppState::new(Config {
        bind: addr,
        ..Config::default()
    });
    let app_state = state.clone();
    let app = router(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (format!("http://{}", addr), app_state)
}

fn fresh_principal() -> (SigningKey, Did) {
    let key = SigningKey::generate(&mut OsRng);
    let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
    (key, did)
}

#[tokio::test]
async fn org_channel_wire_roundtrip() {
    // Two principals + one channel. Owner posts; subscriber polls.
    let (relay_url, _state) = spawn_relay().await;

    let (owner_key, owner_did) = fresh_principal();
    let (sub_key, sub_did) = fresh_principal();

    let owner_client = RelayClient::new(relay_url.clone(), owner_did.clone(), &owner_key);
    let sub_client = RelayClient::new(relay_url.clone(), sub_did.clone(), &sub_key);
    owner_client.register().await.unwrap();
    sub_client.register().await.unwrap();

    let handle = QueueHandle::parse("dev:secretariat").unwrap();

    // Owner posts three envelopes.
    for i in 0..3 {
        let body = format!("envelope #{i} on the channel");
        owner_client
            .send(&owner_did, &handle, body.as_bytes(), "text/markdown")
            .await
            .unwrap();
    }

    // Subscriber authenticates + polls the channel queue.
    let (token, _) = sub_client.authenticate().await.unwrap();
    let inbound = sub_client
        .poll(&owner_did, &handle, &token, 0)
        .await
        .unwrap();
    assert_eq!(inbound.len(), 3);
    assert_eq!(inbound[0].body, b"envelope #0 on the channel");
    assert_eq!(inbound[2].body, b"envelope #2 on the channel");

    // Cursor advances correctly — polling past the tail returns empty.
    let last_id = inbound[2].id;
    let empty = sub_client
        .poll(&owner_did, &handle, &token, last_id)
        .await
        .unwrap();
    assert!(empty.is_empty());

    // Subscriber's own DM queue (`(sub_did, inbox:default)`) is empty —
    // distinct from the org channel queue. Proves the index axis
    // separates queues by (owner, handle), not by relay session.
    let dm = QueueHandle::parse("inbox:default").unwrap();
    let dm_inbound = sub_client.poll(&sub_did, &dm, &token, 0).await.unwrap();
    assert!(dm_inbound.is_empty());
}

#[tokio::test]
async fn org_channel_daemon_sync_files_inbound() {
    // Full receiver-side flow: subscriber's vault is hand-fixtured with
    // an org dir + membership + channel.md, then sync_now runs. The
    // daemon's enumeration finds the channel queue, polls the relay,
    // and files inbound envelopes to disk.
    let (relay_url, _state) = spawn_relay().await;

    let (owner_key, owner_did) = fresh_principal();
    let (sub_key, sub_did) = fresh_principal();

    let owner_client = RelayClient::new(relay_url.clone(), owner_did.clone(), &owner_key);
    let sub_client = RelayClient::new(relay_url.clone(), sub_did.clone(), &sub_key);
    owner_client.register().await.unwrap();
    sub_client.register().await.unwrap();

    let handle = QueueHandle::parse("dev:secretariat").unwrap();

    // Owner posts an envelope. Body is opaque bytes; for this smoke
    // test we use a minimal valid envelope frontmatter so file_inbound
    // can parse the recipient and route the file under the right
    // queue-dir on the subscriber's disk.
    let body = format!(
        "---\n\
$envelope:\n  \
$type: tech.equanimi.secretariat.envelope\n  \
from: {}\n  \
to: {}\n  \
handle: {}\n  \
source: smoke-test\n\
---\nhello channel\n",
        owner_did.as_str(),
        owner_did.as_str(),
        handle.as_str(),
    );
    owner_client
        .send(&owner_did, &handle, body.as_bytes(), "text/markdown")
        .await
        .unwrap();

    // Hand-fixture the subscriber's vault so the daemon sees:
    //   <root>/orgs/equanimi.tech/.org             — org metadata with owner_did
    //   <root>/orgs/equanimi.tech/membership.local.md  — membership facts
    //   <root>/orgs/equanimi.tech/channels/dev/secretariat/channel.md — channel marker
    let tmp = TempDir::new().unwrap();
    let sub_paths = KeyPaths::under(tmp.path().to_path_buf());
    sub_paths.ensure_dirs().unwrap();

    let alias = OrgAlias::parse("equanimi.tech").unwrap();
    save_org(
        &sub_paths.orgs_root,
        &Org::new(
            alias.clone(),
            Some(owner_did.clone()),
            "EquanimiTech",
            "smoke test org",
            Utc::now(),
        ),
        false,
    )
    .unwrap();

    save_membership(
        &sub_paths
            .orgs_root
            .join(alias.as_str())
            .join("membership.local.md"),
        &OrgMembership {
            org_did: owner_did.clone(),
            role: "subscribe".to_string(),
            relay_endpoint: RelayEndpoint::parse(&relay_url).unwrap(),
            joined_at: Utc::now(),
            inviter_did: None,
            body: String::new(),
        },
    )
    .unwrap();

    let channel_dir = sub_paths
        .orgs_root
        .join(alias.as_str())
        .join("channels")
        .join("dev")
        .join("secretariat");
    std::fs::create_dir_all(&channel_dir).unwrap();
    std::fs::write(
        channel_dir.join("channel.md"),
        "---\nname: Secretariat Dev\n---\n",
    )
    .unwrap();

    // Subscriber also needs RelayState registered for the test relay
    // so refresh_token_if_needed will run. Mark as registered + cleared
    // cursors so sync_now does the full handshake.
    let mut state = RelayState::default();
    state.entry_mut(&relay_url).registered = true;
    state.save(&sub_paths.relay_state).unwrap();

    // Run one sync cycle as the subscriber.
    let outcome = sync_now(&sub_paths, &sub_did, &sub_key).await.unwrap();

    // The poll loop hit one endpoint with two queues (DM + channel)
    // — both should report success.
    let report = outcome
        .per_relay
        .iter()
        .find(|r| r.endpoint == relay_url)
        .expect("sync touched the relay");
    assert!(
        report.warnings.is_empty(),
        "no warnings: {:?}",
        report.warnings
    );
    assert_eq!(
        report.inbound_count, 1,
        "exactly one envelope pulled across all queues"
    );

    // Envelope must be on disk somewhere — full vault walk, not just
    // the expected location, because there's a known divergence between
    // where org metadata lives (<root>/orgs/<alias>/) and where
    // queue_dir composes envelope paths (<root>/<alias>/channels/...).
    // The substrate routes inbound correctly; the path is just not yet
    // unified with the org metadata tree. See
    // [[project_v07_layout_complete_roadmap]] for the alignment fix.
    let mut found = Vec::new();
    walk_md(tmp.path(), &mut found);
    // Filter out the hand-fixtured marker files so we're checking for
    // the actual inbound envelope.
    found.retain(|p| {
        !p.ends_with("channel.md")
            && !p.ends_with("org.md")
            && !p.to_string_lossy().contains("membership.local.md")
            && !p.ends_with("identity.md")
            && !p.ends_with("contacts.md")
    });
    assert_eq!(found.len(), 1, "exactly one inbound envelope filed");
    let raw = std::fs::read_to_string(&found[0]).unwrap();
    assert!(raw.contains("hello channel"));
    assert!(raw.contains(&format!("handle: {}", handle.as_str())));

    // Post-Move-3c: org-owned queues file under
    // `<root>/orgs/<alias>/channels/<segs>/...` — same root the metadata
    // lives under. The pre-Move-3c divergence (<root>/<alias>/channels/...
    // vs <root>/orgs/<alias>/channels/...) is resolved.
    let filed_path = found[0].to_string_lossy().to_string();
    assert!(
        filed_path.contains("orgs/equanimi.tech/channels/dev/secretariat/envelopes"),
        "envelope path: {filed_path}"
    );
}

fn walk_md(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if !dir.is_dir() {
        return;
    }
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk_md(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("md") {
            out.push(p);
        }
    }
}
