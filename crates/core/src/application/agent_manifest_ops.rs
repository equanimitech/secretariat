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
    /// File claims to be an agentManifest (`$type` matches or
    /// frontmatter parses as one) but its cryptographic contract is
    /// broken — signature verification failed, signer DID embeds no
    /// ed25519 key, or the embedded key bytes are malformed. Per
    /// hard rule #8 ("envelope whose signature fails is malformed and
    /// must be quarantined, not surfaced"), callers MUST quarantine
    /// rather than fall back to a cached or empty view of the
    /// signer's `authorized_agents`.
    #[error(
        "manifest at {path} failed verification — {reason}; quarantine and do not consume"
    )]
    TamperDetected { path: PathBuf, reason: &'static str },
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

/// Parse an envelope file as an agentManifest.
///
/// Three outcomes:
///   - `Ok(Some(manifest))` — file IS a verified agentManifest.
///   - `Ok(None)` — file is NOT an agentManifest (wrong `$type`, no
///     manifest frontmatter, unparseable YAML before we could see
///     `$type`). Caller treats as a plain envelope or skips.
///   - `Err(TamperDetected)` — file claims to be an agentManifest
///     (`$type` matches) but its cryptographic contract is broken.
///     Caller MUST quarantine rather than silently fall back to a
///     stale or empty view of the signer's `authorized_agents`
///     (hard rule #8).
///
/// Verifies the manifest's embedded signature against the signer's
/// `did:key` (the only DID method this slice supports for manifest
/// verification).
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
        // Frontmatter unparseable before we could see `$type` — treat
        // as not-a-manifest. (If a tamper edit broke the YAML itself,
        // the receiver simply won't pick this up; the prior cached
        // manifest stays in force.)
        Err(_) => return Ok(None),
    };
    if fm.ty != AGENT_MANIFEST_TYPE {
        return Ok(None);
    }
    // From here on the file claims to be a manifest — any further
    // failure is tamper-evidence, not benign drift.
    let manifest: AgentManifest = fm.try_into().map_err(|_| {
        AgentManifestOpsError::TamperDetected {
            path: path.to_path_buf(),
            reason: "manifest fields malformed",
        }
    })?;
    let pk = manifest.signer.embedded_ed25519_key().ok_or_else(|| {
        AgentManifestOpsError::TamperDetected {
            path: path.to_path_buf(),
            reason: "signer DID embeds no ed25519 key",
        }
    })?;
    let vk = ed25519_dalek::VerifyingKey::from_bytes(&pk).map_err(|_| {
        AgentManifestOpsError::TamperDetected {
            path: path.to_path_buf(),
            reason: "signer's embedded key bytes are not a valid ed25519 verifying key",
        }
    })?;
    if !manifest.verify(&vk) {
        return Err(AgentManifestOpsError::TamperDetected {
            path: path.to_path_buf(),
            reason: "embedded signature does not cover the manifest fields",
        });
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

        // Ingest returns Err — tamper detected, NOT Ok(None). A silent
        // None here would let the receiver fall back to a cached or
        // empty view of the signer's authorized_agents (hard rule #8
        // violation).
        let err = ingest_manifest_from_file(&path).unwrap_err();
        assert!(
            matches!(err, AgentManifestOpsError::TamperDetected { .. }),
            "expected TamperDetected, got {err:?}"
        );
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
