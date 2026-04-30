//! Use case: verify a stamped markdown document.
//!
//! Reads the file, parses frontmatter, checks the hash invariant via the
//! aggregate, resolves the signer's DID, and verifies the ed25519 signature.
//! Returns a [`VerifyOutcome`] describing the result.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature as DalekSignature, Verifier, VerifyingKey};
use thiserror::Error;

use crate::domain::{
    AttestedDocument, Did, DocHash, DocumentInvariantError, StampAct,
};
use crate::infrastructure::markdown::{parse_document, MarkdownError};
use crate::ports::{DidResolutionError, DidResolver};

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("markdown error: {0}")]
    Markdown(#[from] MarkdownError),
}

#[derive(Debug, Clone)]
pub enum VerifyOutcome {
    Verified {
        signer: Did,
        stamped_at: DateTime<Utc>,
        act: StampAct,
    },
    Tampered {
        claimed_hash: DocHash,
        computed_hash: DocHash,
    },
    Unsigned,
    SignerUnresolvable {
        signer: Did,
        cause: DidResolutionError,
    },
    SignatureInvalid {
        signer: Did,
    },
}

pub fn verify_document<R: DidResolver>(
    file_path: &Path,
    resolver: &R,
) -> Result<VerifyOutcome, VerifyError> {
    let raw = fs::read_to_string(file_path).map_err(|e| VerifyError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;
    let parsed = parse_document(&raw)?;

    let stamp = match parsed.stamp {
        None => return Ok(VerifyOutcome::Unsigned),
        Some(s) => s,
    };

    // Aggregate invariant: doc hash matches body.
    let aggregate = match AttestedDocument::new(parsed.envelope, stamp.clone(), parsed.body) {
        Ok(a) => a,
        Err(DocumentInvariantError::HashMismatch { claimed, computed }) => {
            return Ok(VerifyOutcome::Tampered {
                claimed_hash: claimed,
                computed_hash: computed,
            });
        }
    };

    // Resolve the signer's DID document.
    let resolved = match resolver.resolve(&stamp.signer) {
        Ok(r) => r,
        Err(e) => {
            return Ok(VerifyOutcome::SignerUnresolvable {
                signer: stamp.signer.clone(),
                cause: e,
            });
        }
    };

    // Try each candidate verifying key. First success wins (decision log #3).
    let dalek_sig = DalekSignature::from_bytes(stamp.signature.as_bytes());
    let payload = aggregate.signed_payload();
    for key_bytes in &resolved.stamp_public_keys {
        let Ok(vk) = VerifyingKey::from_bytes(key_bytes) else {
            continue;
        };
        if vk.verify(payload, &dalek_sig).is_ok() {
            return Ok(VerifyOutcome::Verified {
                signer: stamp.signer.clone(),
                stamped_at: stamp.stamped_at,
                act: stamp.act,
            });
        }
    }

    Ok(VerifyOutcome::SignatureInvalid {
        signer: stamp.signer.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::StampAct;
    use crate::infrastructure::ed25519_signer::{AlwaysAllowGate, Ed25519Signer};
    use crate::infrastructure::keys::generate_keypair;
    use crate::ports::ResolvedDid;
    use std::cell::RefCell;
    use tempfile::TempDir;

    /// Closure-based stub resolver. `RefCell` because `DidResolver::resolve`
    /// takes `&self`, but tests want to inspect/mutate state.
    struct StubResolver<F>
    where
        F: Fn(&Did) -> Result<ResolvedDid, DidResolutionError>,
    {
        f: F,
        calls: RefCell<Vec<Did>>,
    }

    impl<F> StubResolver<F>
    where
        F: Fn(&Did) -> Result<ResolvedDid, DidResolutionError>,
    {
        fn new(f: F) -> Self {
            Self {
                f,
                calls: RefCell::new(vec![]),
            }
        }
    }

    impl<F> DidResolver for StubResolver<F>
    where
        F: Fn(&Did) -> Result<ResolvedDid, DidResolutionError>,
    {
        fn resolve(&self, did: &Did) -> Result<ResolvedDid, DidResolutionError> {
            self.calls.borrow_mut().push(did.clone());
            (self.f)(did)
        }
    }

    fn write_stamped_file(dir: &TempDir, body: &str) -> (PathBuf, [u8; 32]) {
        let path = dir.path().join("doc.md");
        fs::write(&path, body).unwrap();

        let key = generate_keypair();
        let pubkey = key.verifying_key().to_bytes();
        let signer = Ed25519Signer::new(
            Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            key,
            AlwaysAllowGate,
        );
        let _ = crate::application::stamp_document::stamp_document(
            &path,
            &signer,
            StampAct::Attest,
            false,
            Utc::now(),
        )
        .unwrap();
        (path, pubkey)
    }

    #[test]
    fn verified_when_keys_match() {
        let dir = TempDir::new().unwrap();
        let (path, pubkey) = write_stamped_file(&dir, "# Hello\n");

        let resolver = StubResolver::new(move |did| {
            Ok(ResolvedDid {
                did: did.clone(),
                stamp_public_keys: vec![pubkey],
                raw_document: serde_json::Value::Null,
            })
        });

        let outcome = verify_document(&path, &resolver).unwrap();
        assert!(matches!(outcome, VerifyOutcome::Verified { .. }));
    }

    #[test]
    fn unsigned_when_no_stamp() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("doc.md");
        fs::write(&path, "# raw\n").unwrap();

        let resolver = StubResolver::new(|_| {
            Err(DidResolutionError::NotFound {
                url: "n/a".into(),
            })
        });

        let outcome = verify_document(&path, &resolver).unwrap();
        assert!(matches!(outcome, VerifyOutcome::Unsigned));
    }

    #[test]
    fn tampered_when_body_modified() {
        let dir = TempDir::new().unwrap();
        let (path, pubkey) = write_stamped_file(&dir, "# Hello\n");

        // Append text after stamping.
        let mut current = fs::read_to_string(&path).unwrap();
        current.push_str("tampered\n");
        fs::write(&path, current).unwrap();

        let resolver = StubResolver::new(move |did| {
            Ok(ResolvedDid {
                did: did.clone(),
                stamp_public_keys: vec![pubkey],
                raw_document: serde_json::Value::Null,
            })
        });

        let outcome = verify_document(&path, &resolver).unwrap();
        assert!(matches!(outcome, VerifyOutcome::Tampered { .. }));
    }

    #[test]
    fn unresolvable_when_resolver_errors() {
        let dir = TempDir::new().unwrap();
        let (path, _pubkey) = write_stamped_file(&dir, "# Hello\n");

        let resolver = StubResolver::new(|_| {
            Err(DidResolutionError::NotFound {
                url: "https://no.example/.well-known/did.json".into(),
            })
        });

        let outcome = verify_document(&path, &resolver).unwrap();
        assert!(matches!(outcome, VerifyOutcome::SignerUnresolvable { .. }));
    }

    #[test]
    fn invalid_signature_when_wrong_key() {
        let dir = TempDir::new().unwrap();
        let (path, _real_pubkey) = write_stamped_file(&dir, "# Hello\n");

        let other = generate_keypair();
        let other_pub = other.verifying_key().to_bytes();
        let resolver = StubResolver::new(move |did| {
            Ok(ResolvedDid {
                did: did.clone(),
                stamp_public_keys: vec![other_pub],
                raw_document: serde_json::Value::Null,
            })
        });

        let outcome = verify_document(&path, &resolver).unwrap();
        assert!(matches!(outcome, VerifyOutcome::SignatureInvalid { .. }));
    }
}
