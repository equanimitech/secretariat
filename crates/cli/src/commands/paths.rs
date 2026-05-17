//! Resolves `~/.secretariat/` paths, honoring the test override `SECRETARIAT_HOME`
//! so smoke tests don't touch the real user directory.

use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

use secretariat_core::infrastructure::identity_store::load_identity;
use secretariat_core::infrastructure::keys::KeyPaths;
use secretariat_core::Did;

pub fn key_paths() -> Result<KeyPaths> {
    if let Ok(p) = std::env::var("SECRETARIAT_HOME") {
        return Ok(KeyPaths::under(PathBuf::from(p)));
    }
    KeyPaths::discover().context("resolving ~/.secretariat")
}

/// Read the principal's DID from `<self_root>/identity.md`.
pub fn load_did(paths: &KeyPaths) -> Result<Did> {
    let identity = load_identity(&paths.identity_md)
        .map_err(|e| anyhow!("loading identity: {e}"))?
        .ok_or_else(|| {
            anyhow!(
                "no identity found at {} — run `sec init` first",
                paths.identity_md.display()
            )
        })?;
    Ok(identity.did)
}
