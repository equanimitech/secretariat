//! Resolves `~/.secretariat/` paths, honoring the test override `SECRETARIAT_HOME`
//! so smoke tests don't touch the real user directory.

use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

use secretariat_core::infrastructure::keys::KeyPaths;
use secretariat_core::Did;

pub fn key_paths() -> Result<KeyPaths> {
    if let Ok(p) = std::env::var("SECRETARIAT_HOME") {
        return Ok(KeyPaths::under(PathBuf::from(p)));
    }
    KeyPaths::discover().context("resolving ~/.secretariat")
}

/// Read the principal's DID from `~/.secretariat/did`, with a backward-compat
/// fallback to the old behavior of pulling it out of `did.json`.
pub fn load_did(paths: &KeyPaths) -> Result<Did> {
    let did_file = paths.root.join("did");
    if did_file.exists() {
        let raw = std::fs::read_to_string(&did_file)
            .with_context(|| format!("reading {}", did_file.display()))?;
        let trimmed = raw.trim();
        return Did::parse(trimmed).map_err(|e| anyhow!("invalid DID in {}: {e}", did_file.display()));
    }
    // Fallback: did:web installs that pre-date the `did` file.
    if paths.did_document.exists() {
        let raw = std::fs::read_to_string(&paths.did_document)
            .with_context(|| format!("reading {}", paths.did_document.display()))?;
        let v: serde_json::Value =
            serde_json::from_str(&raw).context("parsing did.json")?;
        let id = v
            .get("id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| anyhow!("did.json has no `id` field"))?;
        return Did::parse(id).map_err(|e| anyhow!("did.json id is invalid: {e}"));
    }
    Err(anyhow!(
        "no DID found at {} — run `sec init` first",
        paths.root.display()
    ))
}
