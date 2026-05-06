//! Key + did:web document file management at `~/.secretariat/`.
//!
//! Filesystem layout (decision log #6, #9 and the plan):
//!
//! ```text
//! ~/.secretariat/
//! ├── key                          ed25519 PKCS#8 PEM, mode 0600
//! ├── did.json                     DID document scaffold (user hosts)
//! ├── contacts.json                known peers (Contact aggregate, mode 0600)
//! ├── attention-envelope.md        principal's signed bounds
//! ├── template.md                  user-customizable AG template
//! ├── inbox/                       incoming stamped envelopes
//! ├── outbox/                      drafts awaiting principal stamp
//! ├── peers/                       cached did:web docs
//! └── bin/                         user-local helper binaries (e.g. touchid-prompt)
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
    pub signing_key: PathBuf,
    pub did_document: PathBuf,
    pub contacts: PathBuf,
    pub relay_state: PathBuf,
    pub peers_cache: PathBuf,
    pub inbox: PathBuf,
    pub outbox: PathBuf,
    /// Local-queue captures (substrate v0.3 — `Recipient::LocalQueue`).
    /// Files land at `<queues>/<namespace>/<slug>/<timestamp>.md`.
    pub queues: PathBuf,
    pub bin: PathBuf,
    pub template: PathBuf,
    pub attention_envelope: PathBuf,
    pub profile: PathBuf,
    /// BYOK config for the cognition adapter. Default-off: missing file
    /// = no contextification.
    pub cognition_config: PathBuf,
    /// Append-only ledger of contextification decisions. Lives under
    /// `queues/` so a `tail` over the queues tree picks it up alongside
    /// captures.
    pub contextification_log: PathBuf,
}

impl KeyPaths {
    pub fn discover() -> Result<Self, KeyError> {
        let home = dirs::home_dir().ok_or(KeyError::NoHome)?;
        Ok(Self::under(home.join(".secretariat")))
    }

    pub fn under(root: PathBuf) -> Self {
        Self {
            signing_key: root.join("key"),
            did_document: root.join("did.json"),
            contacts: root.join("contacts.json"),
            relay_state: root.join("relay-state.json"),
            peers_cache: root.join("peers"),
            inbox: root.join("inbox"),
            outbox: root.join("outbox"),
            queues: root.join("queues"),
            bin: root.join("bin"),
            template: root.join("template.md"),
            attention_envelope: root.join("attention-envelope.md"),
            profile: root.join("profile.json"),
            cognition_config: root.join("cognition.json"),
            contextification_log: root.join("queues").join(".contextification.log"),
            root,
        }
    }

    pub fn ensure_dirs(&self) -> Result<(), KeyError> {
        for dir in [
            &self.root,
            &self.peers_cache,
            &self.inbox,
            &self.outbox,
            &self.queues,
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
        assert!(paths.inbox.is_dir());
        assert!(paths.outbox.is_dir());
        assert!(paths.bin.is_dir());
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
