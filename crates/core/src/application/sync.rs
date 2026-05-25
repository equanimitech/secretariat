//! Use case: one full sync cycle against the principal's registered relays.
//!
//! Three sub-tasks per cycle:
//!
//! 1. **Poll inbound** — for each registered relay, refresh auth token if
//!    needed, pull envelopes since the cursor, file each into
//!    `~/.secretariat/inbox/`, advance the cursor.
//! 2. **Drain claim notifications** — pull claim events for invites this
//!    principal created and auto-add the claimer as a contact (the
//!    defining behavior of a correspondence invite — see
//!    `process_correspondence_claims`).
//! 3. **Drain pending sends** — find every stamped self-authored envelope
//!    under `<root>/<alias>/channels/<segs>/envelopes/YYYY/MM/DD/*.md`,
//!    deliver it via `send_stamped_envelope`, move the file to the
//!    queue's `sent/YYYY/MM/DD/` archive.
//!
//! Three callers today:
//! - The CLI daemon's `serve` loop (background process, calls each tick)
//! - The CLI's `sec daemon tick` one-shot (TBD)
//! - The Tauri app's `sync_now` IPC command (principal-initiated, per the
//!   review-session model — see
//!   `memory/feedback_review_session_model.md`).
//!
//! Returns a [`SyncOutcome`] reporting what happened. Callers log as
//! they see fit; this function is silent.

use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::SigningKey;
use thiserror::Error;

use crate::application::{
    list_channels, process_correspondence_claims, send_stamped_envelope, ClaimProcessOutcome,
    CorrespondenceClaim, SendError,
};
use crate::infrastructure::contact_store::ContactBook;
use crate::infrastructure::keys::KeyPaths;
use crate::infrastructure::markdown::parse_document;
use crate::infrastructure::membership_store::{load_membership, MEMBERSHIP_FILENAME};
use crate::infrastructure::org_store::{list_org_dirs, org_channels_root};
use crate::infrastructure::queue_dir::{envelopes_dir, AliasMap, AliasMapError};
use crate::infrastructure::transport::{
    ClaimedInviteWire, RelayClient, RelayClientError, RelayInbound, RelayState,
};
use crate::domain::QueueHandle;
use crate::Did;

const TOKEN_REFRESH_BUFFER: Duration = Duration::minutes(5);

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("relay state io: {0}")]
    RelayStateIo(#[from] crate::infrastructure::transport::RelayStateError),
    #[error("contacts io: {0}")]
    ContactsIo(#[from] crate::infrastructure::contact_store::ContactStoreError),
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
    pub auto_added_contacts: usize,
    /// Errors encountered, in order. Soft errors (one transient relay
    /// down, one malformed claimant DID) don't fail the whole sync —
    /// each relay is independent.
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncOutcome {
    pub per_relay: Vec<RelaySyncReport>,
    pub sent_envelopes: usize,
    /// Outbox drain warnings (e.g. unknown recipient, network failure
    /// for a specific draft). Per-file failures don't fail the sync.
    pub outbox_warnings: Vec<String>,
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

    // One alias map per sync cycle. Loading it once means a contact
    // added partway through the cycle won't be reflected until next
    // tick — fine, since the alternative (reload per envelope) would
    // hit `contacts.json` n times for a one-second window of staleness.
    let aliases = AliasMap::load(did.clone(), paths)?;

    let mut state = RelayState::load(&paths.relay_state)?;

    // Enumerate queues this principal subscribes to. v0.8 sources:
    //   - DM auto-subscription: `(self_did, "inbox:default")` at every
    //     relay we're registered with.
    //   - Org channel subscriptions: walks `<root>/orgs/<alias>/` and
    //     emits one queue per channel-dir the principal joined. Wired
    //     in the org-membership slice; currently returns empty.
    let queues = enumerate_subscribed_queues(paths, did, &state);

    // Group queues by endpoint so token refresh + claim drain happen
    // once per relay, not once per queue.
    let mut endpoints: Vec<String> = queues.iter().map(|q| q.endpoint.clone()).collect();
    endpoints.sort();
    endpoints.dedup();

    for endpoint in &endpoints {
        let mut report = RelaySyncReport {
            endpoint: endpoint.clone(),
            ..Default::default()
        };

        // 1. Refresh token once per endpoint. Same token serves every
        //    queue at this endpoint.
        let client = RelayClient::new(endpoint, did.clone(), key);
        if let Err(e) = refresh_token_if_needed(&mut state, endpoint, &client).await {
            report.warnings.push(format!("auth: {e}"));
            outcome.per_relay.push(report);
            continue;
        }

        // 2. Poll every subscribed queue hosted at this endpoint.
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

        // 3. Drain claim notifications — bilateral peer-invite flow,
        //    one stream per endpoint regardless of how many queues
        //    we poll there.
        match drain_claims_for_relay(&state, endpoint, did, key, paths).await {
            Ok(out) => report.auto_added_contacts = out.added.len(),
            Err(e) => report.warnings.push(format!("claim drain: {e}")),
        }

        outcome.per_relay.push(report);
    }

    state.save(&paths.relay_state)?;

    // 3. Drain pending sends (relay-agnostic — each stamped self-authored
    // envelope has its own `to` and the contact book carries the right
    // relay endpoint per peer).
    let (sent, warns) = drain_outbox(paths, key).await?;
    outcome.sent_envelopes = sent;
    outcome.outbox_warnings = warns;

    Ok(outcome)
}

// ---------------------------------------------------------------------------
// 1. Poll inbound
// ---------------------------------------------------------------------------

/// One queue the daemon's poll loop iterates. Three fields: WHO owns it
/// (`owner` DID — could be self for DMs, an org DID for channels, a peer
/// for non-default DM handles), WHICH queue on that owner's machine
/// (`handle`), and WHERE it's served (`endpoint`).
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

/// Enumerate every queue this principal currently syncs. Two sources:
///
/// - **DM auto-subscription.** For every registered relay endpoint, the
///   principal's own `(self_did, "inbox:default")` queue. This is the
///   two-party-case-of-the-queue-primitive ([[project_namespace_symmetry]]);
///   peers POST envelopes to this queue, the principal polls them.
///
/// - **Org channel subscriptions.** Walks `<root>/orgs/<alias>/` and emits
///   one entry per channel-dir the principal joined. Reads relay endpoint
///   from `membership.local.md` (written by `accept_org_invite` — next
///   slice). Currently returns nothing because no membership files exist
///   yet; ready when the org-membership slice lands.
///
/// Pure compute (modulo a filesystem walk). Idempotent across calls.
fn enumerate_subscribed_queues(
    paths: &KeyPaths,
    self_did: &Did,
    state: &RelayState,
) -> Vec<SubscribedQueue> {
    let mut queues = Vec::new();

    // Source 1: DM at every registered relay.
    let inbox_default = QueueHandle::parse("inbox:default").expect("inbox:default valid");
    for relay in state.iter().filter(|r| r.registered) {
        queues.push(SubscribedQueue {
            owner: self_did.clone(),
            handle: inbox_default.clone(),
            endpoint: relay.endpoint.clone(),
        });
    }

    // Source 2: org channel subscriptions. Walk every org dir under
    // `<root>/orgs/`. For each org that has a `membership.local.md`
    // file (declares the principal joined this org with a role at a
    // relay), enumerate its channels by walking the filesystem and
    // emit one SubscribedQueue per channel found. Filesystem is
    // authoritative — the membership file declares org-level facts;
    // the presence of channel-dirs IS the channel subscription
    // ([[project_filesystem_authoritative]]).
    if let Ok(orgs) = list_org_dirs(&paths.orgs_root) {
        for org in orgs {
            let membership_path = paths
                .orgs_root
                .join(org.alias.as_str())
                .join(MEMBERSHIP_FILENAME);
            let Ok(Some(membership)) = load_membership(&membership_path) else {
                // No membership file → not subscribed to this org's
                // channels (the dir may exist for unrelated reasons,
                // e.g. an org I created locally before federating).
                continue;
            };
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
/// the org's DID but we authenticate as the subscriber. The two happen
/// to match for DM queues.
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
/// `<root>/<alias-of-to>/<namespace>/<segments>/envelopes/YYYY/MM/DD/`.
///
/// Envelopes whose body can't be parsed as UTF-8 markdown, or that
/// parse but have no `$envelope.recipient` (legacy peer letters from
/// before the queues-as-primitive collapse), fall back to
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
    std::fs::write(&path, &env.body)
}

fn short_did(s: &str) -> String {
    s.replace([':', '/'], "_").chars().take(48).collect()
}

// ---------------------------------------------------------------------------
// 2. Drain claim notifications
// ---------------------------------------------------------------------------

async fn drain_claims_for_relay(
    state: &RelayState,
    endpoint: &str,
    did: &Did,
    key: &SigningKey,
    paths: &KeyPaths,
) -> Result<ClaimProcessOutcome, ClaimDrainError> {
    use crate::domain::RelayEndpoint;

    let token = state
        .entry(endpoint)
        .and_then(|e| e.token.clone())
        .ok_or(ClaimDrainError::NotAuthenticated)?;
    let endpoint_url =
        RelayEndpoint::parse(endpoint).map_err(|e| ClaimDrainError::Endpoint(e.to_string()))?;

    let client = RelayClient::new(endpoint, did.clone(), key);
    let wire: Vec<ClaimedInviteWire> = client
        .claimed_invites(&token)
        .await
        .map_err(|e| ClaimDrainError::Pull(e.to_string()))?;
    if wire.is_empty() {
        return Ok(ClaimProcessOutcome {
            added: vec![],
            skipped: vec![],
        });
    }

    // Wire → domain. Drop malformed entries silently; soft errors only
    // affect the offending claim, not the batch.
    let claims: Vec<CorrespondenceClaim> = wire
        .into_iter()
        .filter_map(|w| {
            let claimant = Did::parse(&w.claimant_did).ok()?;
            let claimed_at = DateTime::parse_from_rfc3339(&w.claimed_at)
                .ok()?
                .with_timezone(&Utc);
            Some(CorrespondenceClaim {
                claimant,
                claimed_at,
                purpose: w.purpose,
            })
        })
        .collect();

    process_correspondence_claims(claims, &paths.contacts, &endpoint_url)
        .map_err(|e| ClaimDrainError::Process(e.to_string()))
}

#[derive(Debug, Error)]
enum ClaimDrainError {
    #[error("relay not authenticated for claim drain")]
    NotAuthenticated,
    #[error("relay endpoint parse: {0}")]
    Endpoint(String),
    #[error("pull claimed invites: {0}")]
    Pull(String),
    #[error("process: {0}")]
    Process(String),
}

// ---------------------------------------------------------------------------
// 3. Drain pending sends
// ---------------------------------------------------------------------------

/// Walk the substrate tree for every queue's `envelopes/YYYY/MM/DD/*.md`
/// and deliver every stamped self-authored envelope via its peer's
/// relay. Successful deliveries are moved into the queue's sibling
/// `sent/YYYY/MM/DD/` archive — which is the drain's dedup signal:
/// the next pass won't re-walk into `sent/` and won't re-send.
///
/// Skipped silently:
/// - Unstamped envelopes (stamp ceremony hasn't completed).
/// - Self-addressed envelopes (`to == self_did` — local captures,
///   never relayed).
/// - Envelopes whose `from` is not the principal (received letters —
///   we never re-relay someone else's traffic).
///
/// Exposed for the daemon's FS-notify-driven watcher so stamp→send
/// latency drops from the poll cadence (15 min) to the debounce
/// window (~200ms) without forcing a full `sync_now`.
///
/// Returns `(count_sent, soft_warnings)`. Per-file failures don't fail
/// the whole drain.
pub async fn drain_pending_sends(
    paths: &KeyPaths,
    self_did: &Did,
    key: &SigningKey,
) -> Result<(usize, Vec<String>), SyncError> {
    let mut sent = 0;
    let mut warnings = Vec::new();
    let contacts = ContactBook::load(&paths.contacts)?;
    walk_envelopes_for_drain(
        &paths.root,
        self_did,
        &contacts,
        key,
        &mut sent,
        &mut warnings,
    )
    .await?;
    Ok((sent, warnings))
}

/// Legacy wire — the v0.8 daemon serve loop still calls `drain_outbox`
/// directly without a `self_did`. Loads it from disk on demand to keep
/// callers stable while the daemon migration lands. Once daemon serve
/// is updated, this alias goes away.
pub async fn drain_outbox(
    paths: &KeyPaths,
    key: &SigningKey,
) -> Result<(usize, Vec<String>), SyncError> {
    // Best-effort self-DID resolution. If the identity file is missing
    // (fresh install, key not seeded), there's nothing self-authored to
    // drain — return an empty result.
    let did = match crate::infrastructure::identity_store::load_identity(&paths.identity_md) {
        Ok(Some(identity)) => identity.did,
        _ => return Ok((0, Vec::new())),
    };
    drain_pending_sends(paths, &did, key).await
}

/// Walk the substrate recursively, draining any `envelopes/` directory
/// we encounter. Skips:
/// - The legacy `_unsorted/` fallback (not a queue).
/// - The principal-level `bin/`, `logs/`, `peers/` trees — none carry envelopes.
/// - Sibling `sent/`, `deferred/`, `archived/`, `_drafts/`, `_ciphertext/`
///   trees of `envelopes/` — those are intentionally out-of-active-drain
///   surface (`sent/` is the drain's own destination; `_drafts/` holds
///   unstamped files which `send_stamped_envelope` would skip anyway).
async fn walk_envelopes_for_drain(
    dir: &std::path::Path,
    self_did: &Did,
    contacts: &ContactBook,
    key: &SigningKey,
    sent: &mut usize,
    warnings: &mut Vec<String>,
) -> Result<(), SyncError> {
    if !dir.exists() {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| SyncError::Io {
        path: dir.to_path_buf(),
        source: e,
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| SyncError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if name == "envelopes" {
            drain_one_envelopes_tree(&path, self_did, contacts, key, sent, warnings).await;
        } else if !should_skip_for_drain(name) {
            Box::pin(walk_envelopes_for_drain(
                &path, self_did, contacts, key, sent, warnings,
            ))
            .await?;
        }
    }
    Ok(())
}

/// Walk one queue's `envelopes/` tree depth-first; for each `.md` leaf,
/// attempt delivery. The post-delivery archive lives at
/// `<queue>/sent/<same-day-shard>/`.
async fn drain_one_envelopes_tree(
    envelopes_root: &std::path::Path,
    _self_did: &Did,
    contacts: &ContactBook,
    key: &SigningKey,
    sent: &mut usize,
    warnings: &mut Vec<String>,
) {
    let queue_dir = match envelopes_root.parent() {
        Some(p) => p.to_path_buf(),
        None => return,
    };
    let sent_root = queue_dir.join("sent");

    let mut stack = vec![envelopes_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let read = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(e) => {
                warnings.push(format!("{}: {e}", dir.display()));
                continue;
            }
        };
        for inner in read {
            let inner = match inner {
                Ok(i) => i,
                Err(e) => {
                    warnings.push(format!("{}: {e}", dir.display()));
                    continue;
                }
            };
            let p = inner.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            // Mirror the day-shard structure under `sent/`.
            let day_shard = p
                .parent()
                .and_then(|parent| parent.strip_prefix(envelopes_root).ok())
                .map(|rel| sent_root.join(rel))
                .unwrap_or_else(|| sent_root.clone());

            match send_stamped_envelope(&p, contacts, key, &day_shard).await {
                Ok(_) => *sent += 1,
                Err(SendError::NotStamped) => continue,
                // SelfAddressed dropped in Move 3a; self-owned-channel
                // routing moves to the daemon in Move 5.
                Err(e) => warnings.push(format!("{}: {e}", p.display())),
            }
        }
    }
}

fn should_skip_for_drain(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "_unsorted"
                | "_ciphertext"
                | "_drafts"
                | "deferred"
                | "archived"
                | "sent"
                | "bin"
                | "logs"
                | "peers"
        )
}

#[cfg(test)]
mod enumeration_tests {
    use super::*;
    use crate::infrastructure::membership_store::{save_membership, OrgMembership};
    use crate::infrastructure::org_store::save_org;
    use crate::domain::{Org, OrgAlias, RelayEndpoint};
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
    fn dm_only_when_no_orgs() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);

        let mut state = RelayState::default();
        let entry = state.entry_mut("https://relay.example");
        entry.registered = true;

        let queues = enumerate_subscribed_queues_for_tests(&paths, &self_did(), &state);
        assert_eq!(queues.len(), 1, "exactly one DM queue per registered relay");
        assert_eq!(queues[0].owner, self_did());
        assert_eq!(queues[0].handle.as_str(), "inbox:default");
        assert_eq!(queues[0].endpoint, "https://relay.example");
    }

    #[test]
    fn skips_unregistered_relays_for_dm_source() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);

        let mut state = RelayState::default();
        state.entry_mut("https://registered.example").registered = true;
        state.entry_mut("https://not-yet.example").registered = false;

        let queues = enumerate_subscribed_queues_for_tests(&paths, &self_did(), &state);
        assert_eq!(queues.len(), 1);
        assert_eq!(queues[0].endpoint, "https://registered.example");
    }

    #[test]
    fn org_with_membership_and_channels_emits_one_queue_per_channel() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);

        // Build an org on disk: alias `equanimi.tech`, membership recorded,
        // two channels marked with `channel.md`.
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

        // Create two channel-dirs with `channel.md` markers.
        let channels_root = paths.orgs_root.join(alias.as_str()).join("channels");
        for handle in ["dev/secretariat", "book"] {
            let dir = channels_root.join(handle);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("channel.md"), "---\nname: x\n---\n").unwrap();
        }

        let state = RelayState::default();
        let queues = enumerate_subscribed_queues_for_tests(&paths, &self_did(), &state);

        // Two queues — one per channel — all owned by org_did at the
        // org's relay endpoint.
        assert_eq!(queues.len(), 2);
        for q in &queues {
            assert_eq!(q.owner, org_did());
            assert_eq!(q.endpoint, "https://relay.equanimi.tech");
        }
        let handles: Vec<&str> = queues.iter().map(|q| q.handle.as_str()).collect();
        assert!(handles.contains(&"dev:secretariat"));
        assert!(handles.contains(&"book"));
    }

    #[test]
    fn org_without_membership_is_skipped() {
        // An org dir exists locally (e.g. one I created myself) but
        // there's no membership.local.md → I'm not subscribed via the
        // org-membership pathway; no queues from this org.
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

    #[test]
    fn dm_and_org_queues_coexist() {
        let tmp = TempDir::new().unwrap();
        let paths = paths_in(&tmp);

        let mut state = RelayState::default();
        state.entry_mut("https://self.relay").registered = true;

        let alias = OrgAlias::parse("equanimi.tech").unwrap();
        save_org(
            &paths.orgs_root,
            &Org::new(
                alias.clone(),
                Some(org_did()),
                "EquanimiTech",
                "",
                Utc::now(),
            ),
            false,
        )
        .unwrap();
        save_membership(
            &paths.orgs_root.join(alias.as_str()).join(MEMBERSHIP_FILENAME),
            &OrgMembership {
                org_did: org_did(),
                role: "publish".to_string(),
                relay_endpoint: RelayEndpoint::parse("https://org.relay").unwrap(),
                joined_at: Utc::now(),
                inviter_did: None,
                body: String::new(),
            },
        )
        .unwrap();
        let dev = paths
            .orgs_root
            .join(alias.as_str())
            .join("channels")
            .join("dev")
            .join("secretariat");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::write(dev.join("channel.md"), "---\nname: x\n---\n").unwrap();

        let queues = enumerate_subscribed_queues_for_tests(&paths, &self_did(), &state);
        assert_eq!(queues.len(), 2, "1 DM + 1 org channel");
        // DM is at self.relay; org is at org.relay — distinct endpoints.
        let endpoints: std::collections::HashSet<&str> =
            queues.iter().map(|q| q.endpoint.as_str()).collect();
        assert_eq!(endpoints.len(), 2);
        assert!(endpoints.contains("https://self.relay"));
        assert!(endpoints.contains("https://org.relay"));
    }
}
