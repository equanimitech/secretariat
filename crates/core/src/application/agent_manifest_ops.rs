//! Application — emit + ingest of agentManifest envelopes.
//!
//! Substrate-for-themia Move 1 Phase C — the on-wire publication of
//! the principal's authorized_agents snapshot. The principal emits an
//! agentManifest envelope into every channel they belong to, so
//! receivers can verify `agent → principal` bindings without reading
//! the principal's private identity record.
//!
//! Emit triggers:
//!   - `sec invite accept` — on first joining an org's channels
//!   - `sec agent add` / `agent rotate` / `agent remove` — refresh
//!     into every channel the principal is already a member of
//!
//! Receivers cache the latest manifest per `(signer, target)`. The
//! cache is consulted by `verify_document_layered` hop 3 (P2 verifier
//! chain). Daemon ingest + cache wiring lands in a follow-up slice.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;
use thiserror::Error;

use crate::domain::{
    Agent, AgentManifest, AgentManifestFrontmatter, Did, ManifestTarget, AGENT_MANIFEST_TYPE,
};

#[derive(Debug, Error)]
pub enum AgentManifestOpsError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("yaml serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

/// Mint + sign an `AgentManifest` for the given scope and write it as a
/// `$type`-tagged envelope into the target channel's `envelopes/YYYY/MM/DD/`
/// day-shard. Returns the absolute path written.
///
/// The envelope body is the manifest YAML (with its own embedded signature
/// over the manifest fields, signed by the principal). The envelope-level
/// `$signature` is NOT added here — receivers can derive author identity
/// from the manifest's own signer field. Future iterations may add an
/// envelope-level signature for transport consistency.
///
/// Channel-dir convention: `<channel-dir>/envelopes/YYYY/MM/DD/<rkey>.md`
/// where `<rkey>` is a millisecond-precision RFC3339-ish timestamp +
/// 6-char random suffix, matching the existing compose convention.
pub fn emit_manifest_into_channel(
    channel_dir: &Path,
    target: ManifestTarget,
    signer: Did,
    authorized_agents: Vec<Agent>,
    principal_key: &SigningKey,
    when: DateTime<Utc>,
) -> Result<PathBuf, AgentManifestOpsError> {
    let manifest =
        AgentManifest::sign(signer, target, authorized_agents, when, principal_key);

    let fm: AgentManifestFrontmatter = (&manifest).into();
    let yaml = serde_yaml::to_string(&fm)?;
    let body = format!("---\n{yaml}---\n");

    let day_shard = channel_dir
        .join("envelopes")
        .join(when.format("%Y").to_string())
        .join(when.format("%m").to_string())
        .join(when.format("%d").to_string());
    fs::create_dir_all(&day_shard).map_err(|e| AgentManifestOpsError::Io {
        path: day_shard.clone(),
        source: e,
    })?;

    let filename = format!(
        "{}-manifest.md",
        when.format("%Y%m%dT%H%M%SZ")
    );
    let path = day_shard.join(filename);
    fs::write(&path, body).map_err(|e| AgentManifestOpsError::Io {
        path: path.clone(),
        source: e,
    })?;
    Ok(path)
}

/// Parse an envelope file as an agentManifest. Returns `Ok(None)` if the
/// file's `$type` is not `agentManifest`. Verifies the manifest's
/// embedded signature against the signer's `did:key` (the only DID
/// method this slice supports for manifest verification).
pub fn ingest_manifest_from_file(
    path: &Path,
) -> Result<Option<AgentManifest>, AgentManifestOpsError> {
    let raw = fs::read_to_string(path).map_err(|e| AgentManifestOpsError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let stripped = strip_frontmatter_delimiters(&raw);
    let fm: AgentManifestFrontmatter = match serde_yaml::from_str(stripped) {
        Ok(fm) => fm,
        Err(_) => return Ok(None),
    };
    if fm.ty != AGENT_MANIFEST_TYPE {
        return Ok(None);
    }
    let manifest: AgentManifest = match fm.try_into() {
        Ok(m) => m,
        Err(_) => return Ok(None),
    };
    // Verify against did:key-embedded verifying key. did:web is deferred.
    let Some(pk) = manifest.signer.embedded_ed25519_key() else {
        return Ok(None);
    };
    let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&pk) else {
        return Ok(None);
    };
    if !manifest.verify(&vk) {
        return Ok(None);
    }
    Ok(Some(manifest))
}

/// Strip the `^---\n` ... `\n---\n` envelope delimiters so the inner
/// YAML can be deserialized directly. Tolerates either CRLF or LF
/// terminators.
fn strip_frontmatter_delimiters(raw: &str) -> &str {
    let s = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);
    let after_open = s
        .strip_prefix("---\r\n")
        .or_else(|| s.strip_prefix("---\n"))
        .unwrap_or(s);
    // Find the closing `\n---` boundary.
    if let Some(end) = after_open.find("\n---") {
        &after_open[..end + 1]
    } else {
        after_open
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{AgentName, AgentRole, AgentSubstrate};
    use tempfile::TempDir;

    fn sample_agent(when: DateTime<Utc>) -> Agent {
        Agent::new(
            Did::from_ed25519_public_key(&[0x99; 32]),
            AgentRole::Scribe,
            AgentName::parse("claude").unwrap(),
            AgentSubstrate::parse("claude-code").unwrap(),
            when,
        )
    }

    #[test]
    fn emit_then_ingest_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let channel_dir = tmp.path().join("orgs").join("themia.pro").join("channels").join("assemblee_generale");
        std::fs::create_dir_all(&channel_dir).unwrap();

        let key = SigningKey::from_bytes(&[0x42; 32]);
        let signer = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        let target = ManifestTarget::Org(Did::from_ed25519_public_key(&[0x11; 32]));
        let when = Utc::now();
        let agents = vec![sample_agent(when)];

        let path = emit_manifest_into_channel(
            &channel_dir,
            target.clone(),
            signer.clone(),
            agents.clone(),
            &key,
            when,
        )
        .unwrap();

        assert!(path.exists());
        assert!(path
            .to_string_lossy()
            .contains("envelopes/"));
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("-manifest.md"));

        let ingested = ingest_manifest_from_file(&path).unwrap().unwrap();
        assert_eq!(ingested.signer, signer);
        assert_eq!(ingested.target, target);
        assert_eq!(ingested.authorized_agents.len(), 1);
        assert_eq!(ingested.authorized_agents[0].name.as_str(), "claude");
    }

    #[test]
    fn ingest_rejects_tampered_manifest() {
        let tmp = TempDir::new().unwrap();
        let channel_dir = tmp.path().join("channel");
        std::fs::create_dir_all(&channel_dir).unwrap();

        let key = SigningKey::from_bytes(&[0x42; 32]);
        let signer = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        let target = ManifestTarget::Global;
        let when = Utc::now();
        let agents = vec![sample_agent(when)];

        let path =
            emit_manifest_into_channel(&channel_dir, target, signer, agents, &key, when).unwrap();

        // Tamper the body — change the agent's name.
        let raw = std::fs::read_to_string(&path).unwrap();
        let tampered = raw.replace("claude", "evil");
        std::fs::write(&path, tampered).unwrap();

        // Ingest returns None (signature no longer covers the bytes).
        let ingested = ingest_manifest_from_file(&path).unwrap();
        assert!(ingested.is_none());
    }

    #[test]
    fn ingest_rejects_non_manifest_envelope() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("not-a-manifest.md");
        std::fs::write(
            &path,
            "---\n$type: tech.equanimi.secretariat.envelope\nfrom: did:key:z6Mk\n---\n",
        )
        .unwrap();
        let result = ingest_manifest_from_file(&path).unwrap();
        assert!(result.is_none());
    }
}
