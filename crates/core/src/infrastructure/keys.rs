//! Key + did:web document file management at `~/.secretariat/`.
//!
//! Filesystem layout (v0.5 namespace-collapse —
//! `docs/pitches/2026-05-17-collapse-namespaces.md`):
//!
//! ```text
//! ~/.secretariat/
//! ├── key                          ed25519 PKCS#8 PEM, mode 0600
//! ├── did.json                     DID document scaffold (user hosts)
//! ├── contacts.json                known peers (Contact aggregate, mode 0600)
//! ├── template.md                  user-customizable AG template
//! ├── preferences.toml             composition, cognition, and delivery settings
//! ├── _self/                       principal-as-queue-root
//! │   └── channels/<segs>/envelopes/...
//! ├── orgs/<alias>/                org-as-queue-root (same shape)
//! │   └── channels/<segs>/envelopes/...
//! ├── peers/                       cached did:web docs
//! └── bin/                         user-local helper binaries (reserved; unused in v0.5)
//! ```

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
    /// `<self_root>/identity/key` (v0.7+); legacy `<root>/key` for
    /// pre-v0.7 vaults.
    pub signing_key: PathBuf,
    /// DID document scaffold; `did:web` principals upload this to their
    /// `.well-known/did.json`. `<self_root>/identity/did.json` (v0.7+);
    /// legacy `<root>/did.json` for pre-v0.7 vaults.
    pub did_document: PathBuf,
    /// Principal's contact book at `<self_root>/contacts.md`. Markdown
    /// with one `##` section per contact (YAML frontmatter for typed
    /// fields, body for free-form prose).
    pub contacts: PathBuf,
    pub relay_state: PathBuf,
    pub peers_cache: PathBuf,
    /// Principal-as-queue-root. Mirror of an org dir's shape, with
    /// `<self_root>/channels/<segs>/envelopes/...` for the principal's own
    /// channels. Reach the channels root via
    /// [`KeyPaths::personal_channels_root`].
    pub self_root: PathBuf,
    /// Consolidated identity record at `<self_root>/identity.md`. Carries
    /// the DID, display_name, full_name, key metadata, and rotation log
    /// in YAML frontmatter; principal-editable prose body.
    pub identity_md: PathBuf,
    /// Directory holding the raw key + DID document — `<self_root>/identity/`.
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
    /// Append-only ledger of contextification decisions. Lives under
    /// `_self/` so a `tail` over the principal's own tree picks it up
    /// alongside captures.
    pub contextification_log: PathBuf,
    /// Principal's user-editable `contract.local.md` stub. When this
    /// file exists, `save_stub_if_absent` uses its body as the scaffold
    /// for every newly-created channel; otherwise the built-in fallback
    /// applies. Lives at `<self_root>/contract-stub.md`.
    pub contract_stub: PathBuf,
    /// User intent for which queues to sync. List of
    /// `{owner_did, handle, relay_endpoint, subscribed_at}` triples persisted
    /// at `<root>/subscriptions.json`. Daemon enumerates this every tick
    /// and polls each queue via `RelayClient::poll(owner, handle, token, cursor)`.
    /// DMs are subscriptions like any other — `(self_did, "inbox:default",
    /// self_relay)` is auto-added on init when the principal has a relay.
    pub subscriptions: PathBuf,
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
        let self_root = root.join("_self");
        let identity_dir = self_root.join("identity");
        Self {
            signing_key: identity_dir.join("key"),
            did_document: identity_dir.join("did.json"),
            identity_md: self_root.join("identity.md"),
            contacts: self_root.join("contacts.md"),
            relay_state: root.join("relay-state.json"),
            peers_cache: root.join("peers"),
            orgs_root: root.join("orgs"),
            bin: root.join("bin"),
            template: root.join("template.md"),
            preferences: root.join("preferences.toml"),
            legacy_cognition_config: root.join("cognition.json"),
            legacy_cadence: root.join("cadence.toml"),
            contextification_log: self_root.join(".contextification.log"),
            contract_stub: self_root.join("contract-stub.md"),
            subscriptions: root.join("subscriptions.json"),
            identity_dir,
            self_root,
            root,
        }
    }

    /// `<self_root>/channels/` — the principal's own channels root.
    /// Per the v0.5 namespace-collapse pitch.
    pub fn personal_channels_root(&self) -> PathBuf {
        self.self_root.join("channels")
    }

    pub fn ensure_dirs(&self) -> Result<(), KeyError> {
        for dir in [
            &self.root,
            &self.peers_cache,
            &self.self_root,
            &self.identity_dir,
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
        assert!(paths.self_root.is_dir());
        assert!(paths.personal_channels_root().is_dir());
        assert!(paths.orgs_root.is_dir());
        assert!(paths.bin.is_dir());
        assert!(paths.self_root.ends_with("_self"));
        assert!(paths.personal_channels_root().ends_with("_self/channels"));
        assert!(paths.orgs_root.ends_with("orgs"));
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
