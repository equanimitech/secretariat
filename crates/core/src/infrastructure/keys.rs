//! Key + did:web document file management at `~/.secretariat/`.
//!
//! Filesystem layout (substrate-for-themia Move 3c —
//! `docs/pitches/2026-05-21-substrate-for-themia.md`, element §2):
//!
//! ```text
//! ~/.secretariat/
//! ├── identity.md                  principal identity record (signed)
//! ├── identity/
//! │   ├── key                      ed25519 PKCS#8 PEM, mode 0600
//! │   ├── did.json                 DID document scaffold (user hosts)
//! │   └── agents/<name>/key        per-agent signing keys, mode 0600
//! ├── contract-stub.md             user-editable contract.local.md scaffold
//! ├── .contextification.log        append-only contextification ledger
//! ├── relay-state.json
//! ├── template.md                  user-customizable AG template
//! ├── preferences.toml             composition, cognition, and delivery settings
//! ├── channels/<segs>/             principal-owned (self) channels — local-only
//! │   └── envelopes/...
//! ├── orgs/<alias>/                org-as-queue-root (federated)
//! │   ├── contract.md / contract.local.md
//! │   └── channels/<segs>/envelopes/...
//! ├── peers/                       cached did:web docs
//! └── bin/                         user-local helper binaries
//! ```
//!
//! Pre-Move-3c layout wrapped the principal's own state inside
//! `_self/` (so `_self/identity.md`, `_self/channels/...`,
//! `_self/identity/agents/...`); this collapse drops the wrapper.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use ed25519_dalek::pkcs8::{DecodePrivateKey, EncodePrivateKey};
use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use thiserror::Error;

use crate::codec;
use crate::domain::Did;

#[derive(Debug, Error)]
pub enum KeyError {
    #[error("could not resolve home directory")]
    NoHome,
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("key file already exists at {0} — refuse to overwrite")]
    KeyExists(PathBuf),
    #[error("PKCS#8 encode/decode error: {0}")]
    Pkcs8(String),
}

#[derive(Debug, Clone)]
pub struct KeyPaths {
    pub root: PathBuf,
    /// Active signing key, raw PKCS#8 bytes, mode `0600`. Lives at
    /// `<root>/identity/key` (Move 3c+); pre-Move-3c vaults stored it at
    /// `<root>/_self/identity/key` — the migrate command moves them.
    pub signing_key: PathBuf,
    /// DID document scaffold; `did:web` principals upload this to their
    /// `.well-known/did.json`. `<root>/identity/did.json`.
    pub did_document: PathBuf,
    pub relay_state: PathBuf,
    pub peers_cache: PathBuf,
    /// Consolidated identity record at `<root>/identity.md`. Carries
    /// the DID, display_name, full_name, key metadata, and rotation log
    /// in YAML frontmatter; principal-editable prose body.
    pub identity_md: PathBuf,
    /// Directory holding the raw key + DID document — `<root>/identity/`.
    pub identity_dir: PathBuf,
    /// Root for org-scoped state. Layout:
    /// `<orgs_root>/<alias>/.org` (metadata) + `<orgs_root>/<alias>/channels/<segs>/...`.
    pub orgs_root: PathBuf,
    pub bin: PathBuf,
    pub template: PathBuf,
    /// Unified principal preferences (composition, cognition, delivery).
    /// Supersedes the legacy `cognition.json` + `cadence.toml` files.
    pub preferences: PathBuf,
    /// Legacy cognition config — kept only for the one-time migration read
    /// in `load_or_migrate_preferences`. Do not write here; write to
    /// `preferences` instead.
    pub legacy_cognition_config: PathBuf,
    /// Legacy delivery cadence config — kept only for the one-time migration.
    pub legacy_cadence: PathBuf,
    /// Append-only ledger of contextification decisions at
    /// `<root>/.contextification.log`.
    pub contextification_log: PathBuf,
    /// Principal's user-editable `contract.local.md` stub. When this
    /// file exists, `save_stub_if_absent` uses its body as the scaffold
    /// for every newly-created channel; otherwise the built-in fallback
    /// applies. Lives at `<root>/contract-stub.md`.
    pub contract_stub: PathBuf,
}

impl KeyPaths {
    pub fn discover() -> Result<Self, KeyError> {
        if let Ok(p) = std::env::var("SECRETARIAT_HOME") {
            if !p.is_empty() {
                return Ok(Self::under(PathBuf::from(p)));
            }
        }
        let home = dirs::home_dir().ok_or(KeyError::NoHome)?;
        Ok(Self::under(home.join(".secretariat")))
    }

    pub fn under(root: PathBuf) -> Self {
        let identity_dir = root.join("identity");
        Self {
            signing_key: identity_dir.join("key"),
            did_document: identity_dir.join("did.json"),
            identity_md: root.join("identity.md"),
            relay_state: root.join("relay-state.json"),
            peers_cache: root.join("peers"),
            orgs_root: root.join("orgs"),
            bin: root.join("bin"),
            template: root.join("template.md"),
            preferences: root.join("preferences.toml"),
            legacy_cognition_config: root.join("cognition.json"),
            legacy_cadence: root.join("cadence.toml"),
            contextification_log: root.join(".contextification.log"),
            contract_stub: root.join("contract-stub.md"),
            identity_dir,
            root,
        }
    }

    /// `<root>/channels/` — the principal's own (self) channels root.
    /// Per the Move 3c two-channel-tree-roots layout: self channels sit
    /// at the vault root, peer-with-orgs at `<root>/orgs/<alias>/channels/`.
    pub fn personal_channels_root(&self) -> PathBuf {
        self.root.join("channels")
    }

    /// `<root>/identity/agents/` — directory holding per-agent signing
    /// keys (substrate-for-themia slice; see
    /// `docs/pitches/2026-05-21-substrate-for-themia.md`). Each agent's key
    /// lives at `<agents_root>/<name>/key` (raw PKCS#8 bytes, mode `0600`),
    /// mirroring the principal-key file pattern.
    pub fn agents_root(&self) -> PathBuf {
        self.identity_dir.join("agents")
    }

    /// Path to a specific agent's signing key. Caller is responsible for
    /// validating the name as an [`crate::domain::AgentName`] before passing.
    pub fn agent_signing_key_path(&self, name: &str) -> PathBuf {
        self.agents_root().join(name).join("key")
    }

    pub fn ensure_dirs(&self) -> Result<(), KeyError> {
        for dir in [
            &self.root,
            &self.peers_cache,
            &self.identity_dir,
            &self.agents_root(),
            &self.personal_channels_root(),
            &self.orgs_root,
            &self.bin,
        ] {
            fs::create_dir_all(dir).map_err(|e| KeyError::Io {
                path: dir.clone(),
                source: e,
            })?;
        }
        Ok(())
    }
}

pub fn generate_keypair() -> SigningKey {
    SigningKey::generate(&mut OsRng)
}

/// Save a PKCS#8 PEM-encoded ed25519 private key with `0600` permissions.
/// Refuses to overwrite an existing file (decision log: keys are precious).
pub fn save_signing_key(path: &Path, key: &SigningKey) -> Result<(), KeyError> {
    if path.exists() {
        return Err(KeyError::KeyExists(path.to_path_buf()));
    }
    let pem = key
        .to_pkcs8_pem(ed25519_dalek::pkcs8::spki::der::pem::LineEnding::LF)
        .map_err(|e| KeyError::Pkcs8(e.to_string()))?;

    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }

    let mut f = opts.open(path).map_err(|e| KeyError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    f.write_all(pem.as_bytes()).map_err(|e| KeyError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

pub fn load_signing_key(path: &Path) -> Result<SigningKey, KeyError> {
    let pem = fs::read_to_string(path).map_err(|e| KeyError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    SigningKey::from_pkcs8_pem(&pem).map_err(|e| KeyError::Pkcs8(e.to_string()))
}

/// Write a `did.json` containing the principal's ed25519 verifying key as a
/// `Ed25519VerificationKey2020` verification method. The user hosts this file
/// at the URL their `did:web` resolves to.
pub fn write_did_document(
    path: &Path,
    did: &Did,
    public_key: &VerifyingKey,
) -> Result<(), KeyError> {
    let key_id = format!("{}#stamp-key-1", did);
    let multibase = codec::encode_ed25519_multibase(&public_key.to_bytes());

    let doc = serde_json::json!({
        "@context": ["https://www.w3.org/ns/did/v1"],
        "id": did.as_str(),
        "verificationMethod": [{
            "id": key_id,
            "type": "Ed25519VerificationKey2020",
            "controller": did.as_str(),
            "publicKeyMultibase": multibase,
        }],
        "assertionMethod": [key_id],
    });

    let pretty = serde_json::to_string_pretty(&doc).map_err(|e| KeyError::Pkcs8(e.to_string()))?;
    fs::write(path, pretty + "\n").map_err(|e| KeyError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

// Multibase encode/decode helpers for ed25519 keys live in `crate::codec`.
// Use `codec::encode_ed25519_multibase` / `codec::decode_ed25519_multibase`.

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn keypaths_under_tempdir() {
        let dir = TempDir::new().unwrap();
        let paths = KeyPaths::under(dir.path().to_path_buf());
        paths.ensure_dirs().unwrap();
        assert!(paths.root.is_dir());
        assert!(paths.peers_cache.is_dir());
        assert!(paths.personal_channels_root().is_dir());
        assert!(paths.orgs_root.is_dir());
        assert!(paths.bin.is_dir());
        assert!(paths.agents_root().is_dir());
        assert!(paths.identity_dir.ends_with("identity"));
        assert!(paths.identity_md.ends_with("identity.md"));
        assert!(paths.personal_channels_root().ends_with("channels"));
        assert!(paths.orgs_root.ends_with("orgs"));
        assert!(paths.agents_root().ends_with("identity/agents"));
        // The legacy `_self/` wrapper is gone.
        assert!(!paths.identity_dir.to_string_lossy().contains("_self"));
        assert!(!paths.identity_md.to_string_lossy().contains("_self"));
        assert!(!paths
            .personal_channels_root()
            .to_string_lossy()
            .contains("_self"));
    }

    #[test]
    fn agent_signing_key_path_shape() {
        let dir = TempDir::new().unwrap();
        let paths = KeyPaths::under(dir.path().to_path_buf());
        let p = paths.agent_signing_key_path("claude");
        assert!(p.ends_with("identity/agents/claude/key"));
        assert!(!p.to_string_lossy().contains("_self"));
    }

    #[test]
    fn key_save_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("key");
        let key = generate_keypair();
        save_signing_key(&path, &key).unwrap();
        let loaded = load_signing_key(&path).unwrap();
        assert_eq!(key.to_bytes(), loaded.to_bytes());

        // Permissions check on Unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "key file mode must be 0600 (got {:o})", mode);
        }
    }

    #[test]
    fn key_save_refuses_to_overwrite() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("key");
        let key = generate_keypair();
        save_signing_key(&path, &key).unwrap();
        let other = generate_keypair();
        let r = save_signing_key(&path, &other);
        assert!(matches!(r, Err(KeyError::KeyExists(_))));
    }

    #[test]
    fn did_document_written_with_correct_shape() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("did.json");
        let key = generate_keypair();
        let did = Did::parse("did:web:rafa.equanimi.tech").unwrap();
        write_did_document(&path, &did, &key.verifying_key()).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["id"], "did:web:rafa.equanimi.tech");
        assert_eq!(
            v["verificationMethod"][0]["type"],
            "Ed25519VerificationKey2020"
        );
        let mb = v["verificationMethod"][0]["publicKeyMultibase"]
            .as_str()
            .unwrap();
        let raw = codec::decode_ed25519_multibase(mb).unwrap();
        assert_eq!(raw, key.verifying_key().to_bytes());
    }
}
