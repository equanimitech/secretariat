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
    list_inbox_files, list_outbox_queue, read_envelope as core_read_envelope,
    sync_now as core_sync_now,
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
    pub to: Option<String>,
    pub stamped: bool,
    pub encrypted: bool,
}

impl From<secretariat_core::application::ListedEnvelope> for EnvelopeListing {
    fn from(e: secretariat_core::application::ListedEnvelope) -> Self {
        Self {
            file_path: e.file_path,
            from: e.from,
            to: e.to,
            stamped: e.stamped,
            encrypted: e.encrypted,
        }
    }
}

/// List received envelopes (`~/.secretariat/inbox/`).
#[tauri::command]
#[specta::specta]
pub async fn list_inbox() -> Result<Vec<EnvelopeListing>, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let listed = list_inbox_files(&paths.inbox).map_err(|e| format!("list_inbox: {e}"))?;
    Ok(listed.into_iter().map(EnvelopeListing::from).collect())
}

/// List the principal's review queue — outbox drafts awaiting a stamp.
/// Excludes already-stamped drafts (those are in flight to the relay)
/// and the `sent/` historical archive.
#[tauri::command]
#[specta::specta]
pub async fn list_review_queue() -> Result<Vec<EnvelopeListing>, String> {
    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let listed =
        list_outbox_queue(&paths.outbox).map_err(|e| format!("list_review_queue: {e}"))?;
    Ok(listed.into_iter().map(EnvelopeListing::from).collect())
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct EnvelopeRead {
    pub body: String,
    pub from: Option<String>,
    pub to: Option<String>,
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
    let did_file = paths.root.join("did");
    let did_str = std::fs::read_to_string(&did_file)
        .map_err(|e| format!("reading {}: {e}", did_file.display()))?;
    let did = Did::parse(did_str.trim()).map_err(|e| format!("parsing DID: {e}"))?;
    let key = load_signing_key(&paths.signing_key)
        .map_err(|e| format!("loading signing key: {e}"))?;

    let outcome = core_sync_now(&paths, &did, &key)
        .await
        .map_err(|e| format!("sync_now: {e}"))?;

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

// Re-export for the bindings module so it can register these commands.
#[allow(dead_code)]
pub fn _types_used_in_bindings() -> (PathBuf,) {
    (PathBuf::new(),)
}
