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

// Re-export for the bindings module so it can register these commands.
#[allow(dead_code)]
pub fn _types_used_in_bindings() -> (PathBuf,) {
    (PathBuf::new(),)
}
