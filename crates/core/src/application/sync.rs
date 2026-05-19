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
//! 3. **Drain outbox** — find every stamped draft in
//!    `~/.secretariat/outbox/<recipient>/*.md`, deliver it via
//!    `send_stamped_envelope`, move to `sent/`.
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
    process_correspondence_claims, send_stamped_envelope, ClaimProcessOutcome,
    CorrespondenceClaim, SendError,
};
use crate::infrastructure::contact_store::ContactBook;
use crate::infrastructure::keys::KeyPaths;
use crate::infrastructure::markdown::parse_document;
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
    let endpoints: Vec<String> = state
        .iter()
        .filter(|r| r.registered)
        .map(|r| r.endpoint.clone())
        .collect();

    for endpoint in endpoints {
        let mut report = RelaySyncReport {
            endpoint: endpoint.clone(),
            ..Default::default()
        };

        // 1. Poll inbound.
        match poll_one_relay(&mut state, &endpoint, did, key, paths, &aliases).await {
            Ok(count) => report.inbound_count = count,
            Err(e) => report.warnings.push(format!("inbound poll: {e}")),
        }

        // 2. Drain claim notifications. Reuses the auth token poll_one set up.
        match drain_claims_for_relay(&state, &endpoint, did, key, paths).await {
            Ok(out) => report.auto_added_contacts = out.added.len(),
            Err(e) => report.warnings.push(format!("claim drain: {e}")),
        }

        outcome.per_relay.push(report);
    }

    state.save(&paths.relay_state)?;

    // 3. Drain outbox (relay-agnostic — each draft has its own `to` and
    // the contact book carries the right relay endpoint per peer).
    let (sent, warns) = drain_outbox(paths, key).await?;
    outcome.sent_envelopes = sent;
    outcome.outbox_warnings = warns;

    Ok(outcome)
}

// ---------------------------------------------------------------------------
// 1. Poll inbound
// ---------------------------------------------------------------------------

async fn poll_one_relay(
    state: &mut RelayState,
    endpoint: &str,
    did: &Did,
    key: &SigningKey,
    paths: &KeyPaths,
    aliases: &AliasMap,
) -> Result<usize, RelayClientError> {
    let client = RelayClient::new(endpoint, did.clone(), key);

    refresh_token_if_needed(state, endpoint, &client).await?;

    let (token, cursor) = {
        let entry = state.entry(endpoint).expect("just refreshed");
        (entry.token.clone().unwrap(), entry.cursor)
    };

    // DMs are the channel-of-two case of the queue primitive — `(self, inbox:default)`.
    // The single index axis on the relay covers both DM and channel traffic;
    // self-DM stream is just the channel keyed on the principal's own DID
    // under `inbox:default`. When channel subscriptions land, this grows to
    // a sibling per-`(owner, handle)` poll loop with its own cursors.
    let inbox_default = QueueHandle::parse("inbox:default").expect("inbox:default valid");
    let inbound = client
        .poll_channel(did, &inbox_default, &token, cursor)
        .await?;
    let mut max_id = cursor;
    for env in &inbound {
        if let Err(e) = file_inbound(paths, aliases, env) {
            // Soft failure on a single envelope shouldn't poison the cursor;
            // log via the returned RelayClientError variant the caller can
            // propagate.
            return Err(RelayClientError::BadResponse(format!(
                "could not file inbound id={}: {e}",
                env.id
            )));
        }
        if env.id > max_id {
            max_id = env.id;
        }
    }
    state.entry_mut(endpoint).cursor = max_id;
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
// 3. Drain outbox
// ---------------------------------------------------------------------------

/// Walk the substrate tree for every `<root>/<alias>/<namespace>/
/// <segments>/outbox/*.md`, deliver every stamped draft via its peer's
/// relay, move successes into the queue's `outbox/sent/` subdir.
/// Unstamped drafts are skipped silently.
///
/// Exposed for the daemon's FS-notify-driven outbox watcher (Slice 2) so
/// stamp→send latency drops from the poll cadence (15 min) to the
/// debounce window (~200ms) without forcing a full `sync_now` (which
/// would also hit registered relays for inbound poll).
///
/// Returns `(count_sent, soft_warnings)`. Per-file failures don't fail
/// the whole drain.
pub async fn drain_outbox(
    paths: &KeyPaths,
    key: &SigningKey,
) -> Result<(usize, Vec<String>), SyncError> {
    let mut sent = 0;
    let mut warnings = Vec::new();
    let contacts = ContactBook::load(&paths.contacts)?;
    walk_outboxes_recursive(&paths.root, &contacts, key, &mut sent, &mut warnings).await?;
    Ok((sent, warnings))
}

/// Walk the substrate recursively, draining any `outbox/` directory we
/// encounter. Skips:
/// - The legacy `_unsorted/` fallback (not a queue).
/// - The principal-level dotfiles + the `bin/`, `logs/`, `peers/`
///   trees — none of those carry envelopes.
/// - The `sent/` subdir under each `outbox/` — that's the daemon's
///   own post-delivery target and re-draining it would loop.
async fn walk_outboxes_recursive(
    dir: &std::path::Path,
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
        if name == "outbox" {
            drain_one_outbox(&path, contacts, key, sent, warnings).await;
        } else if !should_skip_for_drain(name) {
            Box::pin(walk_outboxes_recursive(&path, contacts, key, sent, warnings)).await?;
        }
    }
    Ok(())
}

async fn drain_one_outbox(
    outbox: &std::path::Path,
    contacts: &ContactBook,
    key: &SigningKey,
    sent: &mut usize,
    warnings: &mut Vec<String>,
) {
    let sent_dir = outbox.join("sent");
    let read = match std::fs::read_dir(outbox) {
        Ok(r) => r,
        Err(e) => {
            warnings.push(format!("{}: {e}", outbox.display()));
            return;
        }
    };
    for inner in read {
        let inner = match inner {
            Ok(i) => i,
            Err(e) => {
                warnings.push(format!("{}: {e}", outbox.display()));
                continue;
            }
        };
        let p = inner.path();
        if !p.is_file() || p.extension().and_then(|x| x.to_str()) != Some("md") {
            continue;
        }
        match send_stamped_envelope(&p, contacts, key, &sent_dir).await {
            Ok(_) => *sent += 1,
            Err(SendError::NotStamped) => continue,
            Err(e) => warnings.push(format!("{}: {e}", p.display())),
        }
    }
}

fn should_skip_for_drain(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "_unsorted" | "_ciphertext" | "envelopes" | "deferred" | "archived" | "sent"
                | "bin" | "logs" | "peers"
        )
}
