//! Tauri commands wrapping `secretariat-core` use cases.
//!
//! The CLI (`sec`) and the MCP server (`sec-mcp`) are unchanged surfaces;
//! these commands give the Tauri front-end the same primitives via IPC,
//! calling into `secretariat-core` directly (no subprocess, no sidecar).
//!
//! See `docs/milestones/2026-05-04-tauri-front-door.md` for why the Tauri
//! shell is becoming the principal-facing front door.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use secretariat_core::application::{
    archive_envelope as core_archive_envelope, defer_envelope as core_defer_envelope,
    list_inbox_files, list_review_queue as core_list_review_queue,
    read_envelope as core_read_envelope, sync_now as core_sync_now,
    SyncOutcome as CoreSyncOutcome,
};
use secretariat_core::domain::DisplayName;
use secretariat_core::infrastructure::keys::{
    generate_keypair, load_signing_key, save_signing_key, KeyPaths,
};
use secretariat_core::infrastructure::profile_store::{
    load_profile as core_load_profile, save_profile as core_save_profile, PrincipalProfile,
};
use secretariat_core::Did;

/// What `init_identity` reports back to the front-end.
#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct IdentityState {
    /// The principal's DID — `did:key:z…` for a fresh install.
    pub did: String,
    /// Whether this call generated a new identity (true) or surfaced an
    /// existing one (false). The UI uses this to switch between
    /// "Welcome — your identity is …" vs "Welcome back — you're …".
    pub created: bool,
}

/// Ensure the principal has an identity. Idempotent — generates a fresh
/// did:key on first call, returns the existing one thereafter.
///
/// Mirrors `sec init` (without the optional `--did did:web:...` flag, which
/// is a power-user case that can stay in the CLI for now).
#[tauri::command]
#[specta::specta]
pub async fn init_identity() -> Result<IdentityState, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    paths
        .ensure_dirs()
        .map_err(|e| format!("creating directories under {}: {e}", paths.root.display()))?;

    let did_file = paths.root.join("did");

    // Already initialized? Surface the existing DID.
    if paths.signing_key.exists() && did_file.exists() {
        let did = fs::read_to_string(&did_file)
            .map_err(|e| format!("reading {}: {e}", did_file.display()))?
            .trim()
            .to_string();
        return Ok(IdentityState { did, created: false });
    }

    // Refuse to clobber a partial install (key exists but no DID file, or
    // vice versa). Surface a clear error so the principal can resolve it
    // by either deleting the stale file or reusing the existing key.
    if paths.signing_key.exists() {
        return Err(format!(
            "signing key exists at {} but no DID file at {} — refusing to regenerate",
            paths.signing_key.display(),
            did_file.display()
        ));
    }

    // Generate fresh keypair + derive did:key.
    let key = generate_keypair();
    let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());

    save_signing_key(&paths.signing_key, &key)
        .map_err(|e| format!("writing signing key to {}: {e}", paths.signing_key.display()))?;
    fs::write(&did_file, format!("{did}\n"))
        .map_err(|e| format!("writing DID file {}: {e}", did_file.display()))?;

    log::info!("init_identity: generated new did:key for principal");
    Ok(IdentityState {
        did: did.as_str().to_string(),
        created: true,
    })
}

/// Surface the current identity without generating one. Returns `None`
/// (serialized as `null`) if no identity exists yet — the front-end can
/// use this to decide whether to show onboarding or the main UI.
#[tauri::command]
#[specta::specta]
pub async fn current_identity() -> Result<Option<IdentityState>, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let did_file = paths.root.join("did");
    if !paths.signing_key.exists() || !did_file.exists() {
        return Ok(None);
    }
    let did = fs::read_to_string(&did_file)
        .map_err(|e| format!("reading {}: {e}", did_file.display()))?
        .trim()
        .to_string();
    Ok(Some(IdentityState { did, created: false }))
}

/// Diagnostic — returns the absolute path to `~/.secretariat/`. Useful for
/// "open in Finder" buttons and for surfacing where keys live.
#[tauri::command]
#[specta::specta]
pub async fn secretariat_root() -> Result<String, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    Ok(paths.root.display().to_string())
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct InviteClaimReport {
    pub inviter_did: String,
    pub claimant_did: String,
    pub claimed_at: String,
    /// True when the relay registered this principal as part of the claim
    /// (first-time onboarding). False when the principal was already a tenant.
    pub registered: bool,
}

/// Claim a correspondence invite from a deep link or HTTPS URL.
///
/// Accepts either form:
/// - `secretariat://<host>/v0/invite/<token>` (deep link from landing page)
/// - `https://<host>/v0/invite/<token>` (raw HTTPS URL the inviter shared)
///
/// Generates a fresh identity if none exists yet (so a deep link click is
/// the only step a first-time recipient needs). Maps to the existing
/// `secretariat-core::application::claim_invite` use case.
#[tauri::command]
#[specta::specta]
pub async fn claim_invite_url(deep_link_or_url: String) -> Result<InviteClaimReport, String> {
    use secretariat_core::application::claim_invite;
    use secretariat_core::infrastructure::keys::load_signing_key;

    let claim_url = normalize_invite_url(&deep_link_or_url)
        .ok_or_else(|| format!("URL is not a recognizable invite URL: {deep_link_or_url}"))?;

    // Ensure identity exists. First-time recipients land here with no
    // signing key; auto-init keeps the deep link click as the only step.
    let _ = init_identity().await?;

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let did_file = paths.root.join("did");
    let claimant_did_str = std::fs::read_to_string(&did_file)
        .map_err(|e| format!("reading {}: {e}", did_file.display()))?;
    let claimant_did = Did::parse(claimant_did_str.trim())
        .map_err(|e| format!("parsing principal DID: {e}"))?;
    let key = load_signing_key(&paths.signing_key)
        .map_err(|e| format!("loading signing key: {e}"))?;

    // claim_invite is sync (uses reqwest::blocking). Call via Tauri's
    // bundled tokio so we don't block the runtime thread.
    let result = tauri::async_runtime::spawn_blocking(move || {
        claim_invite(&claim_url, &claimant_did, &key)
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
    .map_err(|e| format!("claim failed: {e}"))?;

    log::info!("claim_invite_url succeeded; inviter = {}", result.inviter_did);

    Ok(InviteClaimReport {
        inviter_did: result.inviter_did.as_str().to_string(),
        claimant_did: result.claimant_did.as_str().to_string(),
        claimed_at: result.claimed_at.to_rfc3339(),
        registered: result.registered,
    })
}

/// Convert `secretariat://<host>/v0/invite/<token>` (deep link from the
/// landing page) into the HTTPS form that
/// `secretariat-core::application::claim_invite` expects. Pass-through
/// for already-HTTPS URLs. Returns `None` when the URL doesn't look like
/// an invite at all.
fn normalize_invite_url(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        return Some(trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("secretariat://") {
        return Some(format!("https://{rest}"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_https_passthrough() {
        let url = "https://secretariat.equanimi.tech/v0/invite/abc";
        assert_eq!(normalize_invite_url(url), Some(url.to_string()));
    }

    #[test]
    fn normalize_deep_link_to_https() {
        let url = "secretariat://secretariat.equanimi.tech/v0/invite/abc";
        assert_eq!(
            normalize_invite_url(url),
            Some("https://secretariat.equanimi.tech/v0/invite/abc".to_string())
        );
    }

    #[test]
    fn normalize_unknown_returns_none() {
        assert_eq!(normalize_invite_url("ftp://nope.example/x"), None);
        assert_eq!(normalize_invite_url("just-a-token"), None);
    }
}

// ---------------------------------------------------------------------------
// Review surface — inbox + outbox queue + envelope read
// ---------------------------------------------------------------------------
//
// Per the review-session model (memory/feedback_review_session_model.md),
// the Tauri app surfaces two collections to the principal: received
// envelopes (inbox) and unstamped drafts awaiting review (outbox queue).
// The principal opens the app at a chosen time, reads bodies, stamps
// approved drafts. No notifications, no push.

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct EnvelopeListing {
    pub file_path: String,
    pub from: Option<String>,
    /// DID of the queue *owner* (recipient). Always set on well-formed
    /// envelopes. UI compares to the principal's own DID to discriminate
    /// local capture (`to == self`) from peer/channel post (`to != self`).
    pub to: Option<String>,
    /// Queue handle on the owner's machine (`<namespace>:<slug>`). Always
    /// set on well-formed envelopes alongside `to`. Direct messages
    /// conventionally use `inbox:default`.
    pub queue: Option<String>,
    pub stamped: bool,
    pub encrypted: bool,
}

impl From<secretariat_core::application::ListedEnvelope> for EnvelopeListing {
    fn from(e: secretariat_core::application::ListedEnvelope) -> Self {
        Self {
            file_path: e.file_path,
            from: e.from,
            to: e.to,
            queue: e.queue,
            stamped: e.stamped,
            encrypted: e.encrypted,
        }
    }
}

/// List received envelopes — walks the v0.3 substrate tree under
/// `~/.secretariat/` for every `envelopes/` directory and collects
/// the `.md` leaves.
#[tauri::command]
#[specta::specta]
pub async fn list_inbox() -> Result<Vec<EnvelopeListing>, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let listed = list_inbox_files(&paths.root).map_err(|e| format!("list_inbox: {e}"))?;
    Ok(listed.into_iter().map(EnvelopeListing::from).collect())
}

/// List the principal's review queue — unstamped outbox drafts plus
/// every envelope on disk. Substrate v0.3 (namespace-symmetric
/// queues) unions per-queue `outbox/*.md` (drafts awaiting a stamp)
/// with per-queue `envelopes/*.md` (received letters + local
/// captures) under one substrate root. Both `to` and `queue` are
/// populated on every entry — discriminate local vs peer by
/// comparing `to` to the principal's own DID.
#[tauri::command]
#[specta::specta]
pub async fn list_review_queue() -> Result<Vec<EnvelopeListing>, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let listed = core_list_review_queue(&paths.root)
        .map_err(|e| format!("list_review_queue: {e}"))?;
    Ok(listed.into_iter().map(EnvelopeListing::from).collect())
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct EnvelopeRead {
    pub body: String,
    pub from: Option<String>,
    /// DID of the queue *owner* (recipient).
    pub to: Option<String>,
    /// Queue handle on the owner's machine (`<namespace>:<slug>`).
    pub queue: Option<String>,
    pub was_encrypted: bool,
}

/// Decrypt + return the body of an envelope file. Plaintext envelopes
/// pass through unchanged; encrypted envelopes are decrypted using the
/// local signing key (key never leaves the device).
#[tauri::command]
#[specta::specta]
pub async fn read_envelope(file_path: String) -> Result<EnvelopeRead, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let path = PathBuf::from(file_path);
    let res =
        core_read_envelope(&path, &paths.signing_key).map_err(|e| format!("read_envelope: {e}"))?;
    Ok(EnvelopeRead {
        body: res.body,
        from: res.envelope_from.map(|d| d.as_str().to_string()),
        to: res.envelope_to.map(|d| d.as_str().to_string()),
        queue: res.envelope_queue.map(|h| h.as_str().to_string()),
        was_encrypted: res.was_encrypted,
    })
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct RelaySyncReport {
    pub endpoint: String,
    pub inbound_count: u32,
    pub auto_added_contacts: u32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct SyncReport {
    pub per_relay: Vec<RelaySyncReport>,
    pub sent_envelopes: u32,
    pub outbox_warnings: Vec<String>,
}

/// Run one sync cycle against every registered relay. Pulls inbound
/// envelopes, auto-adds contacts from claim events, drains stamped
/// drafts from the outbox. Principal-initiated per the review-session
/// model — no background push.
///
/// Idempotent and safe to call repeatedly. Returns a report the UI can
/// surface (counts + non-fatal warnings).
#[tauri::command]
#[specta::specta]
pub async fn sync_now() -> Result<SyncReport, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;

    // Prefer the running daemon's IPC socket so we don't race against
    // its `RelayState` saves. Fall back to running the cycle in-proc
    // when no daemon is reachable — same shape as the CLI's
    // `sec daemon tick` (Slice 1, see daemon-evolution doc).
    let outcome: CoreSyncOutcome = if secretariat_daemon::ipc::is_running(&paths).await {
        let value = secretariat_daemon::ipc::call(&paths, "tick", None)
            .await
            .map_err(|e| format!("ipc tick: {e}"))?;
        serde_json::from_value(value).map_err(|e| format!("decoding outcome: {e}"))?
    } else {
        let did_file = paths.root.join("did");
        let did_str = std::fs::read_to_string(&did_file)
            .map_err(|e| format!("reading {}: {e}", did_file.display()))?;
        let did = Did::parse(did_str.trim()).map_err(|e| format!("parsing DID: {e}"))?;
        let key = load_signing_key(&paths.signing_key)
            .map_err(|e| format!("loading signing key: {e}"))?;
        core_sync_now(&paths, &did, &key)
            .await
            .map_err(|e| format!("sync_now: {e}"))?
    };

    Ok(SyncReport {
        per_relay: outcome
            .per_relay
            .into_iter()
            .map(|r| RelaySyncReport {
                endpoint: r.endpoint,
                inbound_count: r.inbound_count as u32,
                auto_added_contacts: r.auto_added_contacts as u32,
                warnings: r.warnings,
            })
            .collect(),
        sent_envelopes: outcome.sent_envelopes as u32,
        outbox_warnings: outcome.outbox_warnings,
    })
}

/// Move an inbox envelope to `inbox/deferred/` — "remind me later".
/// Returns the new path. Idempotent.
#[tauri::command]
#[specta::specta]
pub async fn defer_inbox_envelope(file_path: String) -> Result<String, String> {
    let _paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let p = std::path::PathBuf::from(file_path);
    let dest = core_defer_envelope(&p)
        .map_err(|e| format!("defer_envelope: {e}"))?;
    Ok(dest.display().to_string())
}

/// Move an inbox envelope to `inbox/archived/` — "ignore / handled".
/// Returns the new path. Idempotent.
#[tauri::command]
#[specta::specta]
pub async fn archive_inbox_envelope(file_path: String) -> Result<String, String> {
    let _paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let p = std::path::PathBuf::from(file_path);
    let dest = core_archive_envelope(&p)
        .map_err(|e| format!("archive_envelope: {e}"))?;
    Ok(dest.display().to_string())
}

/// Stamp an outbox draft and (best-effort) deliver it immediately. Touch
/// ID fires from the app's window context. Returns the relay-assigned
/// id on successful delivery, or stamp metadata only if delivery fails
/// (the daemon's next sync tick retries — same fallback as the CLI's
/// stamp-immediate-send path).
#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct StampReport {
    pub stamped_path: String,
    pub doc_hash: String,
    pub stamped_at: String,
    pub delivered: bool,
    /// Relay-assigned envelope ID. String to avoid BigInt/JS-number
    /// roundtripping (specta forbids `u64` directly).
    pub relay_assigned_id: Option<String>,
    pub delivery_warning: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn stamp_envelope(file_path: String) -> Result<StampReport, String> {
    use secretariat_core::application::{
        send_stamped_envelope as core_send, stamp_document, SendError, StampError,
    };
    use secretariat_core::domain::StampAct;
    use secretariat_core::infrastructure::biometric::build_signer;
    use secretariat_core::infrastructure::contact_store::ContactBook;
    use secretariat_core::ports::SignerError;

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let did_file = paths.root.join("did");
    let did_str = std::fs::read_to_string(&did_file)
        .map_err(|e| format!("reading {}: {e}", did_file.display()))?;
    let did = Did::parse(did_str.trim()).map_err(|e| format!("parsing DID: {e}"))?;
    let key = load_signing_key(&paths.signing_key)
        .map_err(|e| format!("loading signing key: {e}"))?;

    let path = std::path::PathBuf::from(file_path);

    // Stamp lives in a blocking call (the Touch ID gate is sync via the
    // touchid-prompt helper). spawn_blocking keeps the runtime healthy.
    let path_for_stamp = path.clone();
    let did_for_stamp = did.clone();
    let key_for_stamp = key.clone();
    let stamp_result = tauri::async_runtime::spawn_blocking(move || -> Result<_, String> {
        let signer = build_signer(did_for_stamp, key_for_stamp, false)
            .map_err(|e| format!("biometric gate setup: {e}"))?;
        match stamp_document(&path_for_stamp, &signer, StampAct::Attest, false, chrono::Utc::now()) {
            Ok(out) => Ok(out),
            Err(StampError::AlreadyStamped) => Err("file is already stamped".to_string()),
            Err(StampError::Signer(SignerError::BiometricRefused)) => {
                Err("Touch ID refused or cancelled".to_string())
            }
            Err(e) => Err(format!("stamp failed: {e}")),
        }
    })
    .await
    .map_err(|e| format!("join error: {e}"))??;

    let mut report = StampReport {
        stamped_path: stamp_result.stamped_path.display().to_string(),
        doc_hash: stamp_result.stamp.doc_hash.to_string(),
        stamped_at: stamp_result.stamp.stamped_at.to_rfc3339(),
        delivered: false,
        relay_assigned_id: None,
        delivery_warning: None,
    };

    // Best-effort delivery. On failure the daemon's regular sync picks it up.
    let parent = stamp_result
        .stamped_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| path.clone());
    let sent_dir = parent.join("sent");
    let contacts = match ContactBook::load(&paths.contacts) {
        Ok(c) => c,
        Err(e) => {
            report.delivery_warning =
                Some(format!("loading contacts for delivery: {e}"));
            return Ok(report);
        }
    };

    match core_send(&stamp_result.stamped_path, &contacts, &key, &sent_dir).await {
        Ok(out) => {
            report.delivered = true;
            report.relay_assigned_id = Some(out.relay_assigned_id.to_string());
            report.stamped_path = out.moved_to.display().to_string();
        }
        Err(SendError::NotStamped) => {
            // Shouldn't happen — we just stamped. Surface as warning.
            report.delivery_warning =
                Some("internal error: stamp confirmed but file appears unstamped".into());
        }
        Err(e) => {
            report.delivery_warning = Some(format!(
                "stamped, queued for daemon delivery on next sync ({e})"
            ));
        }
    }

    Ok(report)
}

/// Create an invite at the principal's first registered relay. Returns
/// the HTTPS claim URL the inviter shares (recipient's HTML landing
/// page lives at the same URL with `Accept: text/html`). Optional
/// `purpose` becomes the suggested contact name on the receiving side.
#[tauri::command]
#[specta::specta]
pub async fn create_invite(purpose: Option<String>) -> Result<String, String> {
    use secretariat_core::application::{
        create_invite as core_create_invite, DEFAULT_INVITE_TTL_HOURS,
    };
    use secretariat_core::infrastructure::transport::RelayState;

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let did_file = paths.root.join("did");
    let did_str = std::fs::read_to_string(&did_file)
        .map_err(|e| format!("reading {}: {e}", did_file.display()))?;
    let did = Did::parse(did_str.trim()).map_err(|e| format!("parsing DID: {e}"))?;
    let key = load_signing_key(&paths.signing_key)
        .map_err(|e| format!("loading signing key: {e}"))?;

    let state = RelayState::load(&paths.relay_state)
        .map_err(|e| format!("loading relay state: {e}"))?;
    let endpoint = state
        .iter()
        .find(|r| r.registered)
        .map(|r| r.endpoint.clone())
        .ok_or_else(|| {
            "no registered relay yet. Use Settings → Transports to register first."
                .to_string()
        })?;

    // create_invite is sync (reqwest::blocking).
    let purpose_clone = purpose.clone();
    let endpoint_clone = endpoint.clone();
    let did_clone = did.clone();
    let key_clone = key.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        core_create_invite(
            &endpoint_clone,
            &did_clone,
            &key_clone,
            purpose_clone.as_deref(),
            Some(DEFAULT_INVITE_TTL_HOURS),
        )
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
    .map_err(|e| format!("create_invite: {e}"))?;

    Ok(result.claim_url)
}

// ---------------------------------------------------------------------------
// Principal profile — display name (presence, not identity)
// ---------------------------------------------------------------------------
//
// The DID is identity; the profile is presence. The principal sets a
// display name during onboarding (and can edit later). Stored locally
// only — never sent over the wire.

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct Profile {
    pub display_name: String,
}

impl From<PrincipalProfile> for Profile {
    fn from(p: PrincipalProfile) -> Self {
        Self {
            display_name: p.display_name.to_string(),
        }
    }
}

/// Read the principal's profile. Returns null when no profile has been
/// set yet (fresh install pre-onboarding).
#[tauri::command]
#[specta::specta]
pub async fn get_profile() -> Result<Option<Profile>, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let profile = core_load_profile(&paths.profile).map_err(|e| format!("load_profile: {e}"))?;
    Ok(profile.map(Profile::from))
}

/// Set the principal's display name. Idempotent — overwrites whatever
/// was there. The DisplayName parser enforces validity (non-empty,
/// reasonable length, etc.).
#[tauri::command]
#[specta::specta]
pub async fn set_profile(display_name: String) -> Result<Profile, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    paths
        .ensure_dirs()
        .map_err(|e| format!("creating directories: {e}"))?;
    let parsed = DisplayName::parse(&display_name)
        .map_err(|e| format!("invalid name: {e}"))?;
    let profile = PrincipalProfile {
        display_name: parsed,
    };
    core_save_profile(&paths.profile, &profile)
        .map_err(|e| format!("save_profile: {e}"))?;
    Ok(profile.into())
}

// ---------------------------------------------------------------------------
// Contacts — display-name resolution for Sign-mode blobs
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct ContactListing {
    pub did: String,
    pub display_name: String,
}

/// List the principal's known contacts. The Sign-mode home surface
/// resolves recipient DIDs to display names through this; falls back to
/// truncated DID when a peer isn't yet in the contact book.
#[tauri::command]
#[specta::specta]
pub async fn list_contacts() -> Result<Vec<ContactListing>, String> {
    use secretariat_core::application::list_contacts as core_list_contacts;
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let contacts =
        core_list_contacts(&paths.contacts).map_err(|e| format!("list_contacts: {e}"))?;
    Ok(contacts
        .into_iter()
        .map(|c| ContactListing {
            did: c.did.as_str().to_string(),
            display_name: c.display_name.to_string(),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Assistant launcher — open Claude (or any CLI assistant) in Terminal.app
// ---------------------------------------------------------------------------
//
// Click on a home-screen blob spawns the principal's chosen assistant. The
// MCP server is already wired to whichever assistant the principal runs, so
// the launcher just needs to open a terminal and start the binary. No
// clipboard prompt, no prefilled instruction — the assistant picks up
// context naturally via the MCP tools.
//
// Configurability is deferred: hardcoded to `claude` in `Terminal.app` for
// the v0.3 cut. Settings entry lands in a follow-up.

/// Where the home-screen blob launcher points the principal. CLI clients
/// (Claude Code, Gemini CLI, aider) need a terminal to live in; Claude
/// Desktop launches as a regular macOS app and picks up MCP from its own
/// config. Unknown values fall back to Terminal.app.
///
/// macOS-only today. Windows + Linux variants are a separate slice once
/// the GUI ships there (see AGENTS.md "Mac-only Day 1").
#[derive(Debug, Clone, Copy)]
enum AssistantTarget {
    Terminal,
    ITerm,
    Ghostty,
    ClaudeDesktop,
}

impl AssistantTarget {
    fn from_pref(s: Option<&str>) -> Self {
        match s.map(|x| x.trim().to_ascii_lowercase()).as_deref() {
            Some("iterm") | Some("iterm2") => Self::ITerm,
            Some("ghostty") => Self::Ghostty,
            Some("claude") | Some("claude-desktop") => Self::ClaudeDesktop,
            _ => Self::Terminal,
        }
    }
}

#[cfg(target_os = "macos")]
fn launch_macos(target: AssistantTarget, command: &str) -> Result<(), String> {
    launch_macos_in(target, command, None)
}

#[cfg(target_os = "macos")]
fn launch_macos_in(
    target: AssistantTarget,
    command: &str,
    cwd: Option<&std::path::Path>,
) -> Result<(), String> {
    // ClaudeDesktop is a direct app open — no terminal, no command. The
    // command + cwd are ignored for this target.
    if matches!(target, AssistantTarget::ClaudeDesktop) {
        let status = std::process::Command::new("open")
            .args(["-a", "Claude"])
            .status()
            .map_err(|e| format!("spawning `open`: {e}"))?;
        if !status.success() {
            return Err("could not open Claude.app — is it installed?".to_string());
        }
        return Ok(());
    }

    // Prepend a `cd "<cwd>" && ` when caller passed a working directory.
    // Quote-escape the path so spaces are safe; the surrounding script
    // already escapes the resulting `"` for osascript below.
    let full_command = match cwd {
        Some(dir) => {
            let dir_str = dir.to_string_lossy().replace('"', "\\\"");
            format!("cd \"{dir_str}\" && {command}")
        }
        None => command.to_string(),
    };
    let escaped = full_command.replace('"', "\\\"");
    let script = match target {
        AssistantTarget::Terminal => format!(
            "tell application \"Terminal\"\n    activate\n    do script \"{escaped}\"\nend tell"
        ),
        AssistantTarget::ITerm => format!(
            "tell application \"iTerm\"\n    activate\n    create window with default profile\n    tell current session of current window\n        write text \"{escaped}\"\n    end tell\nend tell"
        ),
        AssistantTarget::Ghostty => format!(
            "do shell script \"open -na Ghostty --args -e '{}'\"",
            escaped.replace('\'', "'\\''")
        ),
        AssistantTarget::ClaudeDesktop => unreachable!(),
    };
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("spawning osascript: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("osascript failed: {stderr}"));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn launch_macos(_target: AssistantTarget, _command: &str) -> Result<(), String> {
    Err("assistant launcher: only macOS is supported in this build".to_string())
}

#[cfg(not(target_os = "macos"))]
fn launch_macos_in(
    _target: AssistantTarget,
    _command: &str,
    _cwd: Option<&std::path::Path>,
) -> Result<(), String> {
    Err("assistant launcher: only macOS is supported in this build".to_string())
}

/// Spawn the principal's assistant in their preferred environment. Reads
/// `AppPreferences::assistant_terminal` + `assistant_command` from the
/// caller; defaults to Terminal.app + `claude`.
#[tauri::command]
#[specta::specta]
pub async fn launch_assistant(
    terminal: Option<String>,
    command: Option<String>,
) -> Result<(), String> {
    let target = AssistantTarget::from_pref(terminal.as_deref());
    let cmd = command.as_deref().unwrap_or("claude");
    launch_macos(target, cmd)
}

/// A reviewable organization the principal can dispatch a review session into.
#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct ReviewableOrg {
    /// `_self` for the private vault, alias DNS-label for orgs.
    pub alias: String,
    /// Human-readable label rendered on the button.
    pub display_name: String,
    /// Resolved working directory the review session will cd into.
    pub root_path: String,
}

/// List every vault the principal can review — orgs + Private.
/// Backs the simplified main-window org picker.
#[tauri::command]
#[specta::specta]
pub async fn list_reviewable_orgs() -> Result<Vec<ReviewableOrg>, String> {
    use secretariat_core::application::list_orgs;
    use secretariat_core::infrastructure::org_store::org_dir;

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let mut out = vec![ReviewableOrg {
        alias: "_self".to_string(),
        display_name: "Private".to_string(),
        root_path: paths.root.to_string_lossy().to_string(),
    }];
    let orgs = list_orgs(&paths.orgs_root).map_err(|e| format!("list_orgs: {e}"))?;
    for o in orgs {
        let dir = org_dir(&paths.orgs_root, &o.alias);
        out.push(ReviewableOrg {
            alias: o.alias.as_str().to_string(),
            display_name: o.name.clone(),
            root_path: dir.to_string_lossy().to_string(),
        });
    }
    Ok(out)
}

/// Launch a review session in the principal's chosen terminal, with
/// cwd set to the org's substrate root (or `~/.secretariat` for
/// Private). Passes `--agent review` to surface the org-local review
/// agent if one exists under `<org-root>/.claude/agents/review.md`.
///
/// `alias` is `_self` for Private, or the org's DNS-label alias.
#[tauri::command]
#[specta::specta]
pub async fn review_org(
    alias: String,
    terminal: Option<String>,
    command: Option<String>,
) -> Result<(), String> {
    use secretariat_core::domain::OrgAlias;
    use secretariat_core::infrastructure::org_store::org_dir;

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let cwd = if alias == "_self" {
        paths.root.clone()
    } else {
        let parsed = OrgAlias::parse(&alias).map_err(|e| format!("invalid alias `{alias}`: {e}"))?;
        org_dir(&paths.orgs_root, &parsed)
    };
    if !cwd.is_dir() {
        return Err(format!(
            "vault `{alias}` has no directory at {} — has the org been initialised?",
            cwd.display()
        ));
    }

    let target = AssistantTarget::from_pref(terminal.as_deref());
    let base_cmd = command.as_deref().unwrap_or("claude");
    // `--agent review` selects a per-vault subagent if one exists; if not,
    // Claude Code falls back gracefully to the default conversation.
    let cmd = format!("{base_cmd} --agent review");
    launch_macos_in(target, &cmd, Some(&cwd))
}

// ---------------------------------------------------------------------------
// Quick-pane launcher commands
// ---------------------------------------------------------------------------

/// A channel the principal can launch into from the quick-pane typeahead.
#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct LaunchableChannel {
    pub handle: String,
    pub org: Option<String>,
    pub name: String,
    pub root_path: String,
    pub has_cognition_override: bool,
}

/// List every channel the principal can launch into.
#[tauri::command]
#[specta::specta]
pub async fn list_launchable_channels() -> Result<Vec<LaunchableChannel>, String> {
    use secretariat_core::application::{list_channels, list_orgs};
    use secretariat_core::domain::QueueHandle;
    use secretariat_core::infrastructure::channel_def_store::channel_dir;
    use secretariat_core::infrastructure::{load_channel_binding, org_store::org_channels_root};

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let mut out: Vec<LaunchableChannel> = Vec::new();

    if paths.channels.is_dir() {
        for ch in list_channels(&paths.channels).map_err(|e| format!("list_channels: {e}"))? {
            let parsed = match QueueHandle::parse(&ch.handle) {
                Ok(h) => h,
                Err(_) => continue,
            };
            let default = channel_dir(&paths.channels, &parsed);
            let binding = load_channel_binding(&default).unwrap_or_default();
            let root = binding
                .root_path
                .clone()
                .unwrap_or(default)
                .to_string_lossy()
                .to_string();
            let has_override = binding.launch_command.is_some()
                || !binding.launch_args.is_empty()
                || !binding.launch_env.is_empty();
            out.push(LaunchableChannel {
                handle: ch.handle,
                org: None,
                name: ch.name,
                root_path: root,
                has_cognition_override: has_override,
            });
        }
    }

    for o in list_orgs(&paths.orgs_root).map_err(|e| format!("list_orgs: {e}"))? {
        let root = org_channels_root(&paths.orgs_root, &o.alias);
        if !root.is_dir() {
            continue;
        }
        for ch in list_channels(&root).map_err(|e| format!("list_channels: {e}"))? {
            let parsed = match QueueHandle::parse(&ch.handle) {
                Ok(h) => h,
                Err(_) => continue,
            };
            let default = channel_dir(&root, &parsed);
            let binding = load_channel_binding(&default).unwrap_or_default();
            let resolved = binding
                .root_path
                .clone()
                .unwrap_or(default)
                .to_string_lossy()
                .to_string();
            let has_override = binding.launch_command.is_some()
                || !binding.launch_args.is_empty()
                || !binding.launch_env.is_empty();
            out.push(LaunchableChannel {
                handle: ch.handle,
                org: Some(o.alias.as_str().to_string()),
                name: ch.name,
                root_path: resolved,
                has_cognition_override: has_override,
            });
        }
    }
    Ok(out)
}

/// Launch a channel from the quick-pane via `sec launch` semantics
/// (binding-aware cwd + per-channel cognition overrides applied).
#[tauri::command]
#[specta::specta]
pub async fn launch_channel_from_pane(
    handle: String,
    org: Option<String>,
    terminal: Option<String>,
) -> Result<(), String> {
    use secretariat_core::application::launch_channel_with_binding;
    use secretariat_core::domain::{OrgAlias, QueueHandle};
    use secretariat_core::infrastructure::{
        load_or_migrate_preferences, org_store::org_channels_root, PrefsLauncher,
    };

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let parsed_handle =
        QueueHandle::parse(&handle).map_err(|e| format!("invalid handle `{handle}`: {e}"))?;
    let channels_root = match org.as_deref() {
        None => paths.channels.clone(),
        Some(s) => {
            let alias = OrgAlias::parse(s)
                .map_err(|e| format!("invalid org alias `{s}`: {e}"))?;
            org_channels_root(&paths.orgs_root, &alias)
        }
    };
    let prefs = load_or_migrate_preferences(
        &paths.preferences,
        &paths.legacy_cognition_config,
        &paths.legacy_cadence,
    )
    .map_err(|e| format!("loading preferences: {e}"))?;

    let base = PrefsLauncher::from_prefs(&prefs.cognition);
    let (_p, binding) = launch_channel_with_binding(&channels_root, &parsed_handle, &base)
        .map_err(|e| format!("{e}"))?;
    let launcher = PrefsLauncher::from_prefs_with_binding(&prefs.cognition, &binding);
    let (plan, _b) =
        launch_channel_with_binding(&channels_root, &parsed_handle, &launcher)
            .map_err(|e| format!("{e}"))?;

    let mut shell = String::new();
    for (k, v) in &plan.env {
        let escaped = v.replace('"', "\\\"");
        shell.push_str(&format!("{k}=\"{escaped}\" "));
    }
    shell.push_str(&plan.command);
    for a in &plan.args {
        let escaped = a.replace('"', "\\\"");
        shell.push_str(&format!(" \"{escaped}\""));
    }
    let target = AssistantTarget::from_pref(terminal.as_deref());
    launch_macos_in(target, &shell, Some(&plan.cwd))
}

/// Capture an arbitrary blob of text to `inbox:triage` from the quick-pane.
#[tauri::command]
#[specta::specta]
pub async fn quick_capture(text: String) -> Result<String, String> {
    use chrono::Utc;
    use secretariat_core::application::{capture_to_queue, CaptureRequest, CaptureRoots};
    use secretariat_core::domain::QueueHandle;

    if text.trim().is_empty() {
        return Err("text is empty".to_string());
    }
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    paths.ensure_dirs().map_err(|e| format!("{e}"))?;
    let did = load_self_did(&paths)?;
    let queue =
        QueueHandle::parse("inbox:triage").map_err(|e| format!("invalid queue: {e}"))?;
    let req = CaptureRequest {
        from: did,
        queue,
        body: text,
        source: "quick-pane".to_string(),
    };
    let roots = CaptureRoots {
        flat_queues: &paths.queues,
        channel_tree: &paths.channels,
    };
    let path = capture_to_queue(req, roots, Utc::now())
        .map_err(|e| format!("capture failed: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

fn load_self_did(paths: &KeyPaths) -> Result<Did, String> {
    let did_file = paths.root.join("did");
    if did_file.exists() {
        let raw = std::fs::read_to_string(&did_file).map_err(|e| format!("reading did: {e}"))?;
        return Did::parse(raw.trim()).map_err(|e| format!("invalid did: {e}"));
    }
    if paths.did_document.exists() {
        let raw = std::fs::read_to_string(&paths.did_document)
            .map_err(|e| format!("reading did.json: {e}"))?;
        let v: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| format!("parsing did.json: {e}"))?;
        let id = v
            .get("id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| "did.json has no `id`".to_string())?;
        return Did::parse(id).map_err(|e| format!("invalid did in did.json: {e}"));
    }
    Err("no DID — run `sec init` first".to_string())
}

// Re-export for the bindings module so it can register these commands.
#[allow(dead_code)]
pub fn _types_used_in_bindings() -> (PathBuf,) {
    (PathBuf::new(),)
}
