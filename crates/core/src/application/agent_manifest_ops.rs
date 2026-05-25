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
    Agent, AgentManifest, AgentManifestFrontmatter, Did, EnvelopeSignature, ManifestTarget,
    SignerRole, AGENT_MANIFEST_TYPE,
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
/// The on-disk shape carries TWO independent signatures, both produced
/// by the principal's key:
///   1. **Inner manifest signature** — over the manifest's canonical
///      preimage (`signer`/`target`/`authorized_agents`/`declared_at`).
///      Lets the manifest verify standalone in a cache.
///   2. **Outer envelope `$signature`** — over the body's canonical hash
///      (the body is empty for a manifest, so the doc_hash is
///      `canonical_body_hash("")`). Enforces the substrate's
///      "every envelope carries `$signature`" invariant (hard rule #4).
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
        AgentManifest::sign(signer.clone(), target, authorized_agents, when, principal_key);

    // Outer envelope signature over the body (empty for manifests). The
    // principal signs in their own person — `SignerRole::Principal` —
    // since manifest emission is not a scribe-mediated act.
    const MANIFEST_BODY: &str = "";
    let envelope_signature = EnvelopeSignature::sign_body(
        signer,
        SignerRole::Principal,
        MANIFEST_BODY,
        when,
        principal_key,
    );

    let mut fm: AgentManifestFrontmatter = (&manifest).into();
    fm.envelope_signature = Some(envelope_signature);
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
    use crate::infrastructure::markdown::parse_document;
    let raw = fs::read_to_string(path).map_err(|e| AgentManifestOpsError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    // parse_document gives us the frontmatter/body split — we need the
    // body bytes for outer-signature verification, not just the YAML.
    let parsed = match parse_document(&raw) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    let Some(yaml_block) = parsed.raw_frontmatter.as_deref() else {
        return Ok(None);
    };
    let fm: AgentManifestFrontmatter = match serde_yaml::from_str(yaml_block) {
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
    // Manifest envelopes carry an empty body by emit contract — any
    // bytes here are tamper-indicative even before we check the outer
    // signature.
    if !parsed.body.is_empty() {
        return Err(AgentManifestOpsError::TamperDetected {
            path: path.to_path_buf(),
            reason: "manifest envelope body must be empty",
        });
    }
    // From here on the file claims to be a manifest — any further
    // failure is tamper-evidence, not benign drift.
    let envelope_signature = fm.envelope_signature.clone();
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
    // Layer 1: inner manifest signature must cover the canonical preimage.
    if !manifest.verify(&vk) {
        return Err(AgentManifestOpsError::TamperDetected {
            path: path.to_path_buf(),
            reason: "embedded manifest signature does not cover the manifest fields",
        });
    }
    // Layer 2: outer envelope `$signature` must be present and must
    // cover the body's canonical hash, with the signer matching the
    // manifest's signer (manifests are principal-self-emitted by
    // contract — no scribe in the chain).
    let env_sig = envelope_signature.ok_or_else(|| {
        AgentManifestOpsError::TamperDetected {
            path: path.to_path_buf(),
            reason: "manifest envelope is missing the outer $signature block",
        }
    })?;
    if env_sig.signer != manifest.signer {
        return Err(AgentManifestOpsError::TamperDetected {
            path: path.to_path_buf(),
            reason: "$signature.signer does not match the manifest's signer",
        });
    }
    const MANIFEST_BODY: &str = "";
    if !env_sig.verify_body(MANIFEST_BODY, &vk) {
        return Err(AgentManifestOpsError::TamperDetected {
            path: path.to_path_buf(),
            reason: "outer $signature does not verify against the principal's key",
        });
    }
    Ok(Some(manifest))
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
    fn ingest_rejects_manifest_missing_outer_signature() {
        // Strip the `$signature:` block from a freshly-emitted manifest
        // and confirm ingest reports tamper (outer layer mandatory).
        let tmp = TempDir::new().unwrap();
        let channel_dir = tmp.path().join("channel");
        std::fs::create_dir_all(&channel_dir).unwrap();

        let key = SigningKey::from_bytes(&[0x42; 32]);
        let signer = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        let target = ManifestTarget::Global;
        let when = Utc::now();
        let path = emit_manifest_into_channel(
            &channel_dir,
            target,
            signer,
            vec![sample_agent(when)],
            &key,
            when,
        )
        .unwrap();

        // Strip everything from `$signature:` to end-of-frontmatter.
        let raw = std::fs::read_to_string(&path).unwrap();
        let start = raw.find("$signature:").expect("$signature: present in emit");
        let end_delim = raw[start..].find("\n---").expect("closing delim after $signature");
        let mut without = String::with_capacity(raw.len());
        without.push_str(&raw[..start]);
        without.push_str(&raw[start + end_delim + 1..]);
        std::fs::write(&path, without).unwrap();

        let err = ingest_manifest_from_file(&path).unwrap_err();
        assert!(
            matches!(err, AgentManifestOpsError::TamperDetected { reason, .. }
                if reason.contains("outer $signature block")),
            "expected outer-signature-missing tamper, got {err:?}"
        );
    }

    #[test]
    fn ingest_rejects_manifest_with_mismatched_outer_signer() {
        // Emit a manifest, then overwrite its $signature block with one
        // signed by a different key — outer-signer ≠ inner-signer must
        // surface as tamper.
        let tmp = TempDir::new().unwrap();
        let channel_dir = tmp.path().join("channel");
        std::fs::create_dir_all(&channel_dir).unwrap();

        let principal = SigningKey::from_bytes(&[0x42; 32]);
        let attacker = SigningKey::from_bytes(&[0x99; 32]);
        let signer = Did::from_ed25519_public_key(&principal.verifying_key().to_bytes());
        let when = Utc::now();
        let path = emit_manifest_into_channel(
            &channel_dir,
            ManifestTarget::Global,
            signer,
            vec![sample_agent(when)],
            &principal,
            when,
        )
        .unwrap();

        // Re-emit the file with the attacker signing the outer
        // $signature instead of the principal.
        use crate::domain::SignerRole;
        let raw = std::fs::read_to_string(&path).unwrap();
        let attacker_did =
            Did::from_ed25519_public_key(&attacker.verifying_key().to_bytes());
        let bad_sig =
            EnvelopeSignature::sign_body(attacker_did, SignerRole::Principal, "", when, &attacker);
        let bad_block = serde_yaml::to_string(&serde_yaml::to_value(&bad_sig).unwrap()).unwrap();
        // Replace the existing `$signature:` ... block with the bad one
        // by serializing a minimal frontmatter with both blocks.
        let mut new = raw.clone();
        let start = new.find("$signature:").unwrap();
        let end_delim_rel = new[start..].find("\n---").unwrap();
        new.replace_range(
            start..start + end_delim_rel + 1,
            &format!("$signature:\n{}\n", indent_yaml(&bad_block)),
        );
        std::fs::write(&path, new).unwrap();

        let err = ingest_manifest_from_file(&path).unwrap_err();
        assert!(
            matches!(err, AgentManifestOpsError::TamperDetected { .. }),
            "expected tamper on outer-signer mismatch, got {err:?}"
        );
    }

    fn indent_yaml(yaml: &str) -> String {
        yaml.lines()
            .map(|l| format!("  {l}"))
            .collect::<Vec<_>>()
            .join("\n")
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
