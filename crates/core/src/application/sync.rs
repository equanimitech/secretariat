//! Use case: one full sync cycle against the principal's registered relays.
//!
//! Per the substrate-for-themia slice (Move 3b), DM / peer / bilateral
//! correspondence primitives have been removed entirely; sync is now
//! channel-only:
//!
//! 1. **Poll inbound** — for each subscribed org channel queue, refresh the
//!    auth token if needed, pull envelopes since the cursor, file each into
//!    its on-disk queue directory, advance the cursor.
//!
//! Outbound federation (was `send_envelope.rs` + `drain_outbox`) moves to
//! the daemon in a follow-up move (Move 5 of the same slice). The claim-
//! notification drain that auto-added bilateral contacts is gone with the
//! contact book.
//!
//! Two callers today:
//! - The CLI daemon's `serve` loop (background process, calls each tick)
//! - The Tauri app's `sync_now` IPC command (principal-initiated, per the
//!   review-session model — see
//!   `memory/feedback_review_session_model.md`).
//!
//! Returns a [`SyncOutcome`] reporting what happened. Callers log as
//! they see fit; this function is silent.

use chrono::{Duration, Utc};
use ed25519_dalek::SigningKey;
use thiserror::Error;

use crate::application::{list_channels, META_HANDLE};
use crate::domain::QueueHandle;
use crate::infrastructure::keys::KeyPaths;
use crate::infrastructure::markdown::parse_document;
use crate::infrastructure::membership_store::{load_membership, MEMBERSHIP_FILENAME};
use crate::infrastructure::org_store::{list_org_dirs, org_channels_root};
use crate::infrastructure::queue_dir::{envelopes_dir, AliasMap, AliasMapError};
use crate::infrastructure::transport::{RelayClient, RelayClientError, RelayInbound, RelayState};
use crate::Did;

const TOKEN_REFRESH_BUFFER: Duration = Duration::minutes(5);

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("relay state io: {0}")]
    RelayStateIo(#[from] crate::infrastructure::transport::RelayStateError),
    #[error("alias map: {0}")]
    AliasMap(#[from] AliasMapError),
    #[error("key/path setup: {0}")]
    Keys(#[from] crate::infrastructure::keys::KeyError),
    #[error("io error at {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Per-relay outcome from one sync cycle.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelaySyncReport {
    pub endpoint: String,
    pub inbound_count: usize,
    /// Errors encountered, in order. Soft errors (one transient relay
    /// down, one malformed envelope) don't fail the whole sync —
    /// each relay is independent.
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncOutcome {
    pub per_relay: Vec<RelaySyncReport>,
    /// Outbound federation summary across all orgs. Move 5 — drains
    /// undelivered envelope drafts on each tick. See
    /// [`crate::application::drain_undelivered`].
    #[serde(default)]
    pub federation_sent: usize,
    #[serde(default)]
    pub federation_local_marked: usize,
    #[serde(default)]
    pub federation_warnings: Vec<String>,
}

/// Run one full sync cycle. Idempotent + safe to call back-to-back; the
/// daemon's serve loop calls this on a 15-minute floor (configurable via
/// `cadence.toml`) and the Tauri app's "sync now" affordance calls it
/// on demand.
pub async fn sync_now(
    paths: &KeyPaths,
    did: &Did,
    key: &SigningKey,
) -> Result<SyncOutcome, SyncError> {
    paths.ensure_dirs()?;

    let mut outcome = SyncOutcome::default();

    // One alias map per sync cycle.
    let aliases = AliasMap::load(did.clone(), paths)?;

    let mut state = RelayState::load(&paths.relay_state)?;

    // Enumerate org channel subscriptions only. DM auto-subscribe was
    // removed in the substrate-for-themia slice (Move 3b).
    let queues = enumerate_subscribed_queues(paths, did, &state);

    // Group queues by endpoint so token refresh happens once per relay,
    // not once per queue.
    let mut endpoints: Vec<String> = queues.iter().map(|q| q.endpoint.clone()).collect();
    endpoints.sort();
    endpoints.dedup();

    for endpoint in &endpoints {
        let mut report = RelaySyncReport {
            endpoint: endpoint.clone(),
            ..Default::default()
        };

        let client = RelayClient::new(endpoint, did.clone(), key);
        if let Err(e) = refresh_token_if_needed(&mut state, endpoint, &client).await {
            report.warnings.push(format!("auth: {e}"));
            outcome.per_relay.push(report);
            continue;
        }

        for queue in queues.iter().filter(|q| &q.endpoint == endpoint) {
            match poll_one_queue(&mut state, queue, did, key, paths, &aliases).await {
                Ok(count) => report.inbound_count += count,
                Err(e) => report.warnings.push(format!(
                    "poll {}#{}: {e}",
                    queue.owner.as_str(),
                    queue.handle.as_str()
                )),
            }
        }

        outcome.per_relay.push(report);
    }

    // Move 5: drain outbound undelivered envelopes after the inbound
    // poll. Same `state` so token refreshes are shared with the
    // inbound path. Soft errors are collected, not fatal — a transient
    // relay failure on one envelope retries next tick.
    match crate::application::drain_undelivered(paths, did, key, &mut state).await {
        Ok(fed) => {
            outcome.federation_sent = fed.sent;
            outcome.federation_local_marked = fed.local_marked;
            outcome.federation_warnings = fed.warnings;
        }
        Err(e) => {
            outcome
                .federation_warnings
                .push(format!("drain_undelivered: {e}"));
        }
    }

    state.save(&paths.relay_state)?;

    Ok(outcome)
}

// ---------------------------------------------------------------------------
// Poll inbound
// ---------------------------------------------------------------------------

/// One queue the daemon's poll loop iterates. Three fields: WHO owns it
/// (`owner` DID — an org DID for channel subscriptions), WHICH queue on
/// that owner's machine (`handle`), and WHERE it's served (`endpoint`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribedQueue {
    pub owner: Did,
    pub handle: QueueHandle,
    pub endpoint: String,
}

/// `pub(crate)` so the focused enumeration test below (and future
/// integration tests) can call it directly without spinning up a relay.
#[cfg(test)]
pub(crate) fn enumerate_subscribed_queues_for_tests(
    paths: &KeyPaths,
    self_did: &Did,
    state: &RelayState,
) -> Vec<SubscribedQueue> {
    enumerate_subscribed_queues(paths, self_did, state)
}

/// Enumerate every queue this principal currently syncs. Source today
/// (post-substrate-for-themia):
///
/// - **Org channel subscriptions.** Walks `<root>/orgs/<alias>/` and emits
///   one entry per channel-dir the principal joined. Reads relay endpoint
///   from `membership.local.md`.
///
/// The DM auto-subscribe source was removed with the contact book and
/// the `inbox:default` synthesizer (substrate-for-themia, Moves 3a/3b).
///
/// Pure compute (modulo a filesystem walk). Idempotent across calls.
fn enumerate_subscribed_queues(
    paths: &KeyPaths,
    _self_did: &Did,
    _state: &RelayState,
) -> Vec<SubscribedQueue> {
    let mut queues = Vec::new();

    // Walk every org dir under `<root>/orgs/`. For each org that has a
    // `membership.local.md` file (declares the principal joined this org
    // with a role at a relay), enumerate its channels by walking the
    // filesystem and emit one SubscribedQueue per channel found.
    if let Ok(orgs) = list_org_dirs(&paths.orgs_root) {
        for org in orgs {
            let membership_path = paths
                .orgs_root
                .join(org.alias.as_str())
                .join(MEMBERSHIP_FILENAME);
            let Ok(Some(membership)) = load_membership(&membership_path) else {
                continue;
            };
            // Always subscribe to the org's `_meta` queue — that's where
            // channelDef announcements ride (Slice A'). The walker doesn't
            // surface `_meta` (substrate-private, leading-underscore), so
            // we add it explicitly per active org membership.
            if let Ok(meta_handle) = QueueHandle::parse(META_HANDLE) {
                queues.push(SubscribedQueue {
                    owner: membership.org_did.clone(),
                    handle: meta_handle,
                    endpoint: membership.relay_endpoint.as_str().to_string(),
                });
            }
            let channels_root = org_channels_root(&paths.orgs_root, &org.alias);
            let Ok(channels) = list_channels(&channels_root) else {
                continue;
            };
            for ch in channels {
                let Ok(handle) = QueueHandle::parse(&ch.handle) else {
                    continue;
                };
                queues.push(SubscribedQueue {
                    owner: membership.org_did.clone(),
                    handle,
                    endpoint: membership.relay_endpoint.as_str().to_string(),
                });
            }
        }
    }

    queues
}

/// Poll one `(owner, handle)` queue at its relay, file every inbound
/// envelope, advance the per-queue cursor. Assumes the caller already
/// refreshed the auth token for `queue.endpoint`.
///
/// `did` is the principal's own DID — the bearer identity for the poll.
/// Distinct from `queue.owner`: for org channels the queue is owned by
/// the org's DID but we authenticate as the subscriber.
async fn poll_one_queue(
    state: &mut RelayState,
    queue: &SubscribedQueue,
    did: &Did,
    key: &SigningKey,
    paths: &KeyPaths,
    aliases: &AliasMap,
) -> Result<usize, RelayClientError> {
    let (token, cursor) = {
        let entry = state.entry(&queue.endpoint).ok_or_else(|| {
            RelayClientError::BadResponse(format!(
                "no relay state entry for endpoint {} — refresh skipped?",
                queue.endpoint
            ))
        })?;
        let token = entry.token.clone().ok_or_else(|| {
            RelayClientError::BadResponse(format!("no token for endpoint {}", queue.endpoint))
        })?;
        (token, entry.cursor_for(&queue.owner, &queue.handle))
    };

    let client = RelayClient::new(&queue.endpoint, did.clone(), key);
    let inbound = client
        .poll(&queue.owner, &queue.handle, &token, cursor)
        .await?;
    let mut max_id = cursor;
    for env in &inbound {
        if let Err(e) = file_inbound(paths, aliases, env) {
            return Err(RelayClientError::BadResponse(format!(
                "could not file inbound id={}: {e}",
                env.id
            )));
        }
        if env.id > max_id {
            max_id = env.id;
        }
    }
    state
        .entry_mut(&queue.endpoint)
        .set_cursor_for(&queue.owner, &queue.handle, max_id);
    Ok(inbound.len())
}

async fn refresh_token_if_needed(
    state: &mut RelayState,
    endpoint: &str,
    client: &RelayClient<'_>,
) -> Result<(), RelayClientError> {
    let needs = match state.entry(endpoint) {
        Some(e) => match (e.token.as_ref(), e.token_expires_at) {
            (Some(_), Some(exp)) => Utc::now() >= exp - TOKEN_REFRESH_BUFFER,
            _ => true,
        },
        None => true,
    };
    if needs {
        let (token, expires_at) = client.authenticate().await?;
        let entry = state.entry_mut(endpoint);
        entry.token = Some(token);
        entry.token_expires_at = Some(expires_at);
    }
    Ok(())
}

/// Route one inbound envelope to its on-disk queue directory.
///
/// Reads `(to, handle)` out of the envelope's frontmatter (which is
/// plaintext — only the body is sealed) and writes the file under
/// `<root>/<alias-of-to>/channels/<segments>/envelopes/YYYY/MM/DD/`.
///
/// Envelopes whose body can't be parsed as UTF-8 markdown, or that
/// parse but have no `$envelope.recipient`, fall back to
/// `<root>/_unsorted/`. The daemon never decrypts, so we only need
/// the frontmatter to route — encrypted body bytes are preserved
/// verbatim.
fn file_inbound(
    paths: &KeyPaths,
    aliases: &AliasMap,
    env: &RelayInbound,
) -> Result<(), std::io::Error> {
    let sender_short = env
        .sender_did
        .as_ref()
        .map(|d| short_did(d.as_str()))
        .unwrap_or_else(|| "unknown".to_string());
    let timestamp = env.queued_at.format("%Y-%m-%dT%H-%M-%SZ");
    let filename = format!("{timestamp}-{sender_short}-id{:06}.md", env.id);

    let body_str = std::str::from_utf8(&env.body).ok();
    let recipient = body_str
        .and_then(|s| parse_document(s).ok())
        .and_then(|d| d.envelope.map(|e| e.recipient));

    let target_dir = match recipient {
        Some(r) => {
            let base = envelopes_dir(aliases, &r, &paths.root);
            base.join(env.queued_at.format("%Y/%m/%d").to_string())
        }
        None => paths.root.join("_unsorted"),
    };

    std::fs::create_dir_all(&target_dir)?;
    let path = target_dir.join(filename);
    std::fs::write(&path, &env.body)?;

    // Verifier chain hop 3 wiring: if the freshly-filed envelope IS an
    // agentManifest, ingest it (verifies both inner + outer signatures)
    // and drop a verified copy into the receiver's manifest cache. From
    // this point on, every subsequent verify of an envelope signed by
    // any agent listed in this manifest can attribute it to the manifest
    // signer (the principal). Best-effort: a verify failure here is
    // logged and skipped — the envelope itself stays on disk for later
    // human inspection, but the cache is not poisoned.
    if let Ok(Some(manifest)) = crate::application::ingest_manifest_from_file(&path) {
        if let Err(e) = crate::infrastructure::manifest_cache::store_envelope_bytes(
            &paths.root,
            &manifest,
            &env.body,
        ) {
            eprintln!(
                "[sync] manifest cache store failed at {}: {e}",
                path.display()
            );
        }
    }

    // Slice A': if the freshly-filed envelope IS a channelDef, mirror it
    // into the local org's channels tree (or apply a tombstone). Best-
    // effort: a parse or write failure here is logged and skipped — the
    // envelope file itself stays on disk, and the next poll's eager
    // bootstrap can pick it up.
    if let Some(body) = body_str {
        if let Ok(Some((alias, expected_signer))) =
            derive_org_alias_from_body(body, &paths.orgs_root)
        {
            if let Some(s) = body_str {
                match crate::application::ingest_channel_def_envelope(
                    &paths.orgs_root,
                    &alias,
                    &expected_signer,
                    s,
                ) {
                    Ok(crate::application::IngestOutcome::Created { handle }) => eprintln!(
                        "[sync] channelDef ingest: created `{}` in org `{}`",
                        handle.as_str(),
                        alias.as_str()
                    ),
                    Ok(crate::application::IngestOutcome::Tombstoned { handle }) => eprintln!(
                        "[sync] channelDef ingest: tombstoned `{}` in org `{}`",
                        handle.as_str(),
                        alias.as_str()
                    ),
                    Ok(crate::application::IngestOutcome::NoOp { .. }) => {}
                    Err(crate::application::ChannelDefEnvelopeError::NotAChannelDef) => {}
                    Err(crate::application::ChannelDefEnvelopeError::UnauthorisedSigner {
                        signer,
                        org_did,
                    }) => eprintln!(
                        "[sync] channelDef ingest REJECTED: signer `{signer}` not authorised for org `{org_did}` (expected = org owner DID); ignoring envelope"
                    ),
                    Err(e) => eprintln!("[sync] channelDef ingest failed: {e}"),
                }
            }
        }
    }
    Ok(())
}

/// Resolve `(org_alias, expected_signer_did)` for a freshly-filed
/// envelope by reading its `$envelope.to` DID and looking up an org-dir
/// whose metadata declares that DID. `Ok(None)` means the envelope is
/// for an org we don't know about — channelDef ingest is skipped (the
/// substrate refuses to mirror channels from orgs it doesn't recognise).
fn derive_org_alias_from_body(
    body: &str,
    orgs_root: &std::path::Path,
) -> Result<Option<(crate::domain::OrgAlias, Did)>, std::io::Error> {
    use crate::infrastructure::org_store::list_org_dirs;
    let Ok(parsed) = parse_document(body) else {
        return Ok(None);
    };
    let Some(env) = parsed.envelope else {
        return Ok(None);
    };
    let to_did = env.recipient.owner.as_str().to_string();
    let Ok(orgs) = list_org_dirs(orgs_root) else {
        return Ok(None);
    };
    for org in orgs {
        if let Some(did) = &org.did {
            if did.as_str() == to_did {
                return Ok(Some((org.alias, did.clone())));
            }
        }
    }
    Ok(None)
}

fn short_did(s: &str) -> String {
    s.replace([':', '/'], "_").chars().take(48).collect()
}

#[cfg(test)]
mod enumeration_tests {
    use super::*;
    use crate::domain::{Org, OrgAlias, RelayEndpoint};
    use crate::infrastructure::membership_store::{save_membership, OrgMembership};
    use crate::infrastructure::org_store::save_org;
    use tempfile::TempDir;

    fn self_did() -> Did {
        Did::from_ed25519_public_key(&[1u8; 32])
    }

    fn org_did() -> Did {
        Did::from_ed25519_public_key(&[2u8; 32])
    }

    fn paths_in(tmp: &TempDir) -> KeyPaths {
        let kp = KeyPaths::under(tmp.path().to_path_buf());
        kp.ensure_dirs().unwrap();
        kp
    }

    #[test]
    fn no_orgs_means_no_queues() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);

        let mut state = RelayState::default();
        let entry = state.entry_mut("https://relay.example");
        entry.registered = true;

        let queues = enumerate_subscribed_queues_for_tests(&paths, &self_did(), &state);
        assert!(
            queues.is_empty(),
            "no orgs + no DM auto-subscribe = no queues (substrate-for-themia)"
        );
    }

    #[test]
    fn org_with_membership_and_channels_emits_one_queue_per_channel() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);

        let alias = OrgAlias::parse("equanimi.tech").unwrap();
        let org = Org::new(
            alias.clone(),
            Some(org_did()),
            "EquanimiTech",
            "Test org",
            Utc::now(),
        );
        save_org(&paths.orgs_root, &org, false).unwrap();

        let membership_path = paths
            .orgs_root
            .join(alias.as_str())
            .join(MEMBERSHIP_FILENAME);
        let membership = OrgMembership {
            org_did: org_did(),
            role: "publish".to_string(),
            relay_endpoint: RelayEndpoint::parse("https://relay.equanimi.tech").unwrap(),
            joined_at: Utc::now(),
            inviter_did: None,
            body: String::new(),
        };
        save_membership(&membership_path, &membership).unwrap();

        let channels_root = paths.orgs_root.join(alias.as_str()).join("channels");
        for handle in ["dev/secretariat", "book"] {
            let dir = channels_root.join(handle);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("channel.md"), "---\nname: x\n---\n").unwrap();
        }

        let state = RelayState::default();
        let queues = enumerate_subscribed_queues_for_tests(&paths, &self_did(), &state);

        // 2 channels + the `_meta` queue (Slice A' — channelDef
        // announcements ride on `<alias>:_meta`, always subscribed per
        // active org membership).
        assert_eq!(queues.len(), 3);
        assert!(
            queues.iter().any(|q| q.handle.as_str() == META_HANDLE),
            "_meta must be in subscribed queues"
        );
        for q in &queues {
            assert_eq!(q.owner, org_did());
            assert_eq!(q.endpoint, "https://relay.equanimi.tech");
        }
    }

    #[test]
    fn org_without_membership_is_skipped() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);

        let alias = OrgAlias::parse("local-only.test").unwrap();
        let org = Org::new(
            alias.clone(),
            Some(org_did()),
            "Local Only",
            "no membership file",
            Utc::now(),
        );
        save_org(&paths.orgs_root, &org, false).unwrap();

        let channels_root = paths.orgs_root.join(alias.as_str()).join("channels");
        let dir = channels_root.join("notes");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("channel.md"), "---\nname: x\n---\n").unwrap();

        let state = RelayState::default();
        let queues = enumerate_subscribed_queues_for_tests(&paths, &self_did(), &state);
        assert!(queues.is_empty(), "no membership = no org subscriptions");
    }
}
