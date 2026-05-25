//! Receiver-side cache of verified `agentManifest` envelopes.
//!
//! When the daemon's inbound pipeline (`file_inbound`) writes a
//! manifest envelope to disk, it ALSO drops a verified copy into this
//! cache. The verifier chain (substrate-for-themia P2 hop 3) consults
//! the cache to answer the question "does this agent DID belong to a
//! principal we have manifest evidence for?".
//!
//! Layout:
//!
//! ```text
//! <root>/agents/manifests/<signer-did-sanitized>/<target-id-sanitized>.md
//! ```
//!
//! Files are stored verbatim — the entire manifest envelope bytes,
//! including outer `$signature` and inner `signature`. Lookups parse
//! through [`crate::application::ingest_manifest_from_file`], which
//! re-verifies both layers; the cache is therefore self-defending against
//! disk tamper (a cache file tampered with on disk fails to ingest and
//! is ignored).
//!
//! Latest-wins per `(signer, target)`. Re-storing under the same key
//! overwrites the previous file (atomic via temp + rename).

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::domain::{AgentManifest, Did, ManifestTarget};

#[derive(Debug, Error)]
pub enum ManifestCacheError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Root of the manifest cache under a vault root.
pub fn cache_root(root: &Path) -> PathBuf {
    root.join("agents").join("manifests")
}

/// File path for the given `(signer, target)` pair under the cache.
pub fn entry_path(root: &Path, signer: &Did, target: &ManifestTarget) -> PathBuf {
    cache_root(root)
        .join(sanitize(signer.as_str()))
        .join(format!("{}.md", sanitize(&target.as_string())))
}

/// Store the manifest envelope bytes for `(manifest.signer, manifest.target)`.
/// Overwrites any prior cache entry under that key (latest-wins).
pub fn store_envelope_bytes(
    root: &Path,
    manifest: &AgentManifest,
    envelope_bytes: &[u8],
) -> Result<PathBuf, ManifestCacheError> {
    let dest = entry_path(root, &manifest.signer, &manifest.target);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ManifestCacheError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let tmp = dest.with_extension("md.tmp");
    std::fs::write(&tmp, envelope_bytes).map_err(|e| ManifestCacheError::Io {
        path: tmp.clone(),
        source: e,
    })?;
    std::fs::rename(&tmp, &dest).map_err(|e| ManifestCacheError::Io {
        path: dest.clone(),
        source: e,
    })?;
    Ok(dest)
}

/// Walk every cached manifest under `root` looking for one whose
/// `authorized_agents` contains `agent_did`. Returns the signer
/// principal's DID on first match. Silently skips files that fail to
/// re-verify (tamper on disk, malformed leftovers, etc.) — those are
/// quarantine candidates for the daemon, not a verifier concern.
///
/// The walk is bounded by the number of (signer, target) pairs the
/// receiver has cached, which is O(channels-subscribed × principals).
/// For Themia today this is single-digit; if it grows, a flat
/// `agent-did → (signer, target)` index file becomes worth adding.
pub fn lookup_principal_for_agent(
    root: &Path,
    agent_did: &Did,
) -> Result<Option<Did>, ManifestCacheError> {
    use crate::application::ingest_manifest_from_file;
    let cache = cache_root(root);
    if !cache.exists() {
        return Ok(None);
    }
    for signer_entry in std::fs::read_dir(&cache).map_err(|e| ManifestCacheError::Io {
        path: cache.clone(),
        source: e,
    })? {
        let signer_dir = match signer_entry {
            Ok(e) => e.path(),
            Err(_) => continue,
        };
        if !signer_dir.is_dir() {
            continue;
        }
        let inner = match std::fs::read_dir(&signer_dir) {
            Ok(it) => it,
            Err(_) => continue,
        };
        for manifest_entry in inner {
            let manifest_path = match manifest_entry {
                Ok(e) => e.path(),
                Err(_) => continue,
            };
            if !manifest_path
                .extension()
                .map(|s| s == "md")
                .unwrap_or(false)
            {
                continue;
            }
            // Silently skip tamper / parse failures — see fn docs.
            let Ok(Some(manifest)) = ingest_manifest_from_file(&manifest_path) else {
                continue;
            };
            if manifest
                .authorized_agents
                .iter()
                .any(|a| &a.did == agent_did)
            {
                return Ok(Some(manifest.signer));
            }
        }
    }
    Ok(None)
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ':' | '/' | '#' | '*' | '\\' | '?' | '<' | '>' | '|' | '"' => '_',
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::emit_manifest_into_channel;
    use crate::domain::{Agent, AgentName, AgentRole, AgentSubstrate};
    use chrono::Utc;
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;

    fn sample_agent(when: chrono::DateTime<Utc>) -> Agent {
        Agent::new(
            Did::from_ed25519_public_key(&[0x99; 32]),
            AgentRole::Scribe,
            AgentName::parse("claude").unwrap(),
            AgentSubstrate::parse("claude-code").unwrap(),
            when,
        )
    }

    fn cache_a_manifest(
        root: &Path,
        target: ManifestTarget,
    ) -> (Did, Did) {
        // Emit a manifest into a scratch channel, then copy the
        // verbatim envelope bytes into the cache. Returns (signer,
        // agent_did) for assertions.
        let key = SigningKey::from_bytes(&[0x42; 32]);
        let signer = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        let when = Utc::now();
        let agent = sample_agent(when);
        let channel_dir = root.join("scratch-channel");
        std::fs::create_dir_all(&channel_dir).unwrap();
        let env_path = emit_manifest_into_channel(
            &channel_dir,
            target.clone(),
            signer.clone(),
            vec![agent.clone()],
            &key,
            when,
        )
        .unwrap();
        let bytes = std::fs::read(&env_path).unwrap();

        // Reconstruct the manifest for store_envelope_bytes (it needs
        // signer + target to compute the key).
        let manifest = AgentManifest::sign(
            signer.clone(),
            target,
            vec![agent.clone()],
            when,
            &key,
        );
        store_envelope_bytes(root, &manifest, &bytes).unwrap();
        (signer, agent.did)
    }

    #[test]
    fn lookup_finds_agent_after_cache_store() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let (signer, agent_did) = cache_a_manifest(root, ManifestTarget::Global);
        let found = lookup_principal_for_agent(root, &agent_did).unwrap();
        assert_eq!(found, Some(signer));
    }

    #[test]
    fn lookup_returns_none_for_unknown_agent() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let _ = cache_a_manifest(root, ManifestTarget::Global);
        let unknown = Did::from_ed25519_public_key(&[0xAB; 32]);
        let found = lookup_principal_for_agent(root, &unknown).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn lookup_returns_none_when_cache_missing() {
        let tmp = TempDir::new().unwrap();
        let unknown = Did::from_ed25519_public_key(&[0xAB; 32]);
        let found = lookup_principal_for_agent(tmp.path(), &unknown).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn tampered_cache_file_is_silently_skipped() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let (signer, agent_did) = cache_a_manifest(root, ManifestTarget::Global);
        // Append bytes to the cached envelope body — re-verify fails
        // (body must be empty per emit contract). Lookup MUST return
        // None, NOT propagate an error.
        let entry = entry_path(root, &signer, &ManifestTarget::Global);
        let mut raw = std::fs::read_to_string(&entry).unwrap();
        raw.push_str("tamper\n");
        std::fs::write(&entry, raw).unwrap();

        let found = lookup_principal_for_agent(root, &agent_did).unwrap();
        assert!(found.is_none(), "tampered cache must not yield a principal");
    }
}
