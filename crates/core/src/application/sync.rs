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
use crate::infrastructure::transport::{
    ClaimedInviteWire, RelayClient, RelayClientError, RelayInbound, RelayState,
};
use crate::Did;

const TOKEN_REFRESH_BUFFER: Duration = Duration::minutes(5);

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("relay state io: {0}")]
    RelayStateIo(#[from] crate::infrastructure::transport::RelayStateError),
    #[error("contacts io: {0}")]
    ContactsIo(#[from] crate::infrastructure::contact_store::ContactStoreError),
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
        match poll_one_relay(&mut state, &endpoint, did, key, paths).await {
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
) -> Result<usize, RelayClientError> {
    let client = RelayClient::new(endpoint, did.clone(), key);

    refresh_token_if_needed(state, endpoint, &client).await?;

    let (token, cursor) = {
        let entry = state.entry(endpoint).expect("just refreshed");
        (entry.token.clone().unwrap(), entry.cursor)
    };

    let inbound = client.poll(&token, cursor).await?;
    let mut max_id = cursor;
    for env in &inbound {
        if let Err(e) = file_inbound(paths, env) {
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

fn file_inbound(paths: &KeyPaths, env: &RelayInbound) -> Result<(), std::io::Error> {
    let sender_short = env
        .sender_did
        .as_ref()
        .map(|d| short_did(d.as_str()))
        .unwrap_or_else(|| "unknown".to_string());
    let timestamp = env.queued_at.format("%Y-%m-%dT%H-%M-%SZ");
    let filename = format!("{timestamp}-{sender_short}-id{:06}.md", env.id);
    let path = paths.inbox.join(filename);
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

async fn drain_outbox(
    paths: &KeyPaths,
    key: &SigningKey,
) -> Result<(usize, Vec<String>), SyncError> {
    let mut sent = 0;
    let mut warnings = Vec::new();

    if !paths.outbox.exists() {
        return Ok((sent, warnings));
    }
    let contacts = ContactBook::load(&paths.contacts)?;

    for entry in std::fs::read_dir(&paths.outbox).map_err(|e| SyncError::Io {
        path: paths.outbox.clone(),
        source: e,
    })? {
        let entry = entry.map_err(|e| SyncError::Io {
            path: paths.outbox.clone(),
            source: e,
        })?;
        let recipient_dir = entry.path();
        if !recipient_dir.is_dir() {
            continue;
        }
        let sent_dir = recipient_dir.join("sent");

        for inner in std::fs::read_dir(&recipient_dir).map_err(|e| SyncError::Io {
            path: recipient_dir.clone(),
            source: e,
        })? {
            let inner = match inner {
                Ok(i) => i,
                Err(e) => {
                    warnings.push(format!(
                        "{}: {e}",
                        recipient_dir.display()
                    ));
                    continue;
                }
            };
            let p = inner.path();
            if !p.is_file() || p.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            match send_stamped_envelope(&p, &contacts, key, &sent_dir).await {
                Ok(_) => sent += 1,
                // Unstamped drafts are normal — skip silently.
                Err(SendError::NotStamped) => continue,
                Err(e) => warnings.push(format!("{}: {e}", p.display())),
            }
        }
    }
    Ok((sent, warnings))
}
