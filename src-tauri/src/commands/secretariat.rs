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

use secretariat_core::infrastructure::keys::{generate_keypair, save_signing_key, KeyPaths};
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

// Re-export for the bindings module so it can register these commands.
#[allow(dead_code)]
pub fn _types_used_in_bindings() -> (PathBuf,) {
    (PathBuf::new(),)
}
