//! Use case: verify a markdown document carrying any combination of the
//! three trust-layer blocks (substrate-for-themia Move 2, 2026-05-21):
//!
//!   - `$signature` — author's ed25519 signature over the canonical body.
//!     Mandatory on post-Move-2 envelopes; legacy back-compat permitted.
//!   - `$attestation` — principal's Touch-ID-gated stamp. Selective.
//!   - (Future) counter-stamps for multi-party process-verbaux.
//!
//! Reads the file, parses frontmatter, layer-checks the hash invariant
//! via [`AttestedDocument`] (for the stamp) and direct hash comparison
//! (for the author signature), resolves DIDs, and verifies ed25519
//! signatures. Returns a [`VerifyOutcome`] describing the stamp result,
//! and a sibling [`SignatureOutcome`] describing the author signature.
//!
//! **Agent-authored signatures.** When `$signature.signer_role == agent`
//! and the signer DID is NOT the local principal's DID, the layered
//! verifier returns [`SignatureOutcome::OkUnverifiedAgent`] — the
//! cryptographic check against the embedded `did:key` succeeds but the
//! agent-manifest cache lookup (verifier chain Phase C of the pitch)
//! is not wired in this slice. A separate slice consults the receiver-
//! cached `agentManifest` envelopes to confirm the principal authorized
//! the agent.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature as DalekSignature, Verifier, VerifyingKey};
use thiserror::Error;

use crate::domain::{
    AttestedDocument, Did, DocHash, DocumentInvariantError, EnvelopeSignature, SignerRole,
    StampAct,
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

/// Per-layer outcome for the author signature (`$signature`).
/// Substrate-for-themia Move 2.
#[derive(Debug, Clone)]
pub enum SignatureOutcome {
    /// `$signature` block absent. Legacy back-compat; post-Move-2 wires
    /// SHOULD reject this for channels that demand author provenance.
    None,
    /// Body hash differs from the signed-over hash — body modified after
    /// signing (or `$signature.docHash` was tampered).
    Tampered {
        claimed_hash: DocHash,
        computed_hash: DocHash,
    },
    /// Signer DID does not resolve to a verifying key.
    SignerUnresolvable {
        signer: Did,
        cause: DidResolutionError,
    },
    /// Signature did not verify under any known key for the signer DID.
    Invalid { signer: Did },
    /// Cryptographic check OK; signer is the recipient's own DID (or a
    /// known principal). Receiver may trust under their policy.
    Ok {
        signer: Did,
        signer_role: SignerRole,
        signed_at: DateTime<Utc>,
    },
    /// Full verifier chain succeeded for an agent-signed envelope:
    /// envelope crypto verified AND the agent DID is listed in a
    /// cached `agentManifest` snapshot signed by `principal`. The
    /// receiver can attribute the envelope to `principal` via `agent`.
    VerifiedAgent {
        agent: Did,
        principal: Did,
        signed_at: DateTime<Utc>,
    },
    /// Cryptographic check OK against the signer's embedded `did:key`,
    /// BUT the signer claims `agent` role and no cached `agentManifest`
    /// snapshot binds the agent to a principal yet. Transitional
    /// outcome — once the daemon has ingested at least one manifest
    /// from the relevant principal, subsequent verifies upgrade to
    /// [`SignatureOutcome::VerifiedAgent`].
    OkUnverifiedAgent {
        signer: Did,
        signed_at: DateTime<Utc>,
    },
}

/// Layered verify result: author signature + principal stamp, each
/// reported independently. Substrate-for-themia Move 2 — see module
/// docs for the trust model.
#[derive(Debug, Clone)]
pub struct LayeredVerifyOutcome {
    pub signature: SignatureOutcome,
    pub stamp: VerifyOutcome,
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

/// Layered verifier — checks both the author `$signature` and the
/// principal `$attestation` independently. Use this in receiver-side
/// code (UI badges, daemon inbox-write) where the substrate-for-themia
/// trust model needs per-layer reporting.
///
/// `manifest_cache_root` enables verifier chain hop 3: when present
/// and the envelope is agent-signed, the layered verifier walks the
/// cached `agentManifest` snapshots under that root to bind the agent
/// to a principal — promoting `OkUnverifiedAgent` to `VerifiedAgent`
/// when a binding exists. Pass `None` to skip the hop (legacy
/// callers; tests that don't care about agent binding).
pub fn verify_document_layered<R: DidResolver>(
    file_path: &Path,
    resolver: &R,
    local_principal_did: Option<&Did>,
    manifest_cache_root: Option<&Path>,
) -> Result<LayeredVerifyOutcome, VerifyError> {
    let raw = fs::read_to_string(file_path).map_err(|e| VerifyError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;
    let parsed = parse_document(&raw)?;
    let body = parsed.body.clone();

    let signature_outcome = verify_signature_layer(
        parsed.signature.as_ref(),
        &body,
        resolver,
        local_principal_did,
        manifest_cache_root,
    );

    // Stamp layer — reuse the legacy path by writing a minimal
    // ParsedDocument-like view. The aggregate enforces the body-hash
    // invariant for the stamp; we don't want to re-read the file.
    let stamp_outcome = match parsed.stamp {
        None => VerifyOutcome::Unsigned,
        Some(stamp) => {
            match AttestedDocument::new(parsed.envelope, stamp.clone(), body) {
                Err(DocumentInvariantError::HashMismatch { claimed, computed }) => {
                    VerifyOutcome::Tampered {
                        claimed_hash: claimed,
                        computed_hash: computed,
                    }
                }
                Ok(aggregate) => match resolver.resolve(&stamp.signer) {
                    Err(e) => VerifyOutcome::SignerUnresolvable {
                        signer: stamp.signer.clone(),
                        cause: e,
                    },
                    Ok(resolved) => {
                        let dalek_sig =
                            DalekSignature::from_bytes(stamp.signature.as_bytes());
                        let payload = aggregate.signed_payload();
                        let mut verified = false;
                        for key_bytes in &resolved.stamp_public_keys {
                            let Ok(vk) = VerifyingKey::from_bytes(key_bytes) else {
                                continue;
                            };
                            if vk.verify(payload, &dalek_sig).is_ok() {
                                verified = true;
                                break;
                            }
                        }
                        if verified {
                            VerifyOutcome::Verified {
                                signer: stamp.signer.clone(),
                                stamped_at: stamp.stamped_at,
                                act: stamp.act,
                            }
                        } else {
                            VerifyOutcome::SignatureInvalid {
                                signer: stamp.signer.clone(),
                            }
                        }
                    }
                },
            }
        }
    };

    Ok(LayeredVerifyOutcome {
        signature: signature_outcome,
        stamp: stamp_outcome,
    })
}

fn verify_signature_layer<R: DidResolver>(
    sig: Option<&EnvelopeSignature>,
    body: &str,
    resolver: &R,
    local_principal_did: Option<&Did>,
    manifest_cache_root: Option<&Path>,
) -> SignatureOutcome {
    let sig = match sig {
        None => return SignatureOutcome::None,
        Some(s) => s,
    };

    // Body-hash invariant first (cheap, no IO needed).
    let computed = crate::domain::canonical_body_hash(body);
    if computed != sig.doc_hash {
        return SignatureOutcome::Tampered {
            claimed_hash: sig.doc_hash.clone(),
            computed_hash: computed,
        };
    }

    let resolved = match resolver.resolve(&sig.signer) {
        Ok(r) => r,
        Err(e) => {
            return SignatureOutcome::SignerUnresolvable {
                signer: sig.signer.clone(),
                cause: e,
            };
        }
    };

    let dalek_sig = DalekSignature::from_bytes(sig.signature.as_bytes());
    let payload = sig.doc_hash.as_bytes();
    let mut verified = false;
    for key_bytes in &resolved.stamp_public_keys {
        let Ok(vk) = VerifyingKey::from_bytes(key_bytes) else {
            continue;
        };
        if vk.verify(payload, &dalek_sig).is_ok() {
            verified = true;
            break;
        }
    }
    if !verified {
        return SignatureOutcome::Invalid {
            signer: sig.signer.clone(),
        };
    }

    // Verifier chain hop 3 — agent-role signer: bind to a principal
    // via the cached `agentManifest` snapshots.
    match sig.signer_role {
        SignerRole::Agent => {
            if local_principal_did
                .map(|p| p == &sig.signer)
                .unwrap_or(false)
            {
                // Agent DID == local principal DID — a self-loop edge
                // case (test fixtures, mis-configured vault). Trust as
                // principal-signed and let the higher layer notice.
                return SignatureOutcome::Ok {
                    signer: sig.signer.clone(),
                    signer_role: sig.signer_role,
                    signed_at: sig.signed_at,
                };
            }
            // Consult the receiver's manifest cache. Cache errors are
            // treated as cache miss — verifier MUST stay decisive.
            if let Some(root) = manifest_cache_root {
                if let Ok(Some(principal)) =
                    crate::infrastructure::manifest_cache::lookup_principal_for_agent(
                        root,
                        &sig.signer,
                    )
                {
                    return SignatureOutcome::VerifiedAgent {
                        agent: sig.signer.clone(),
                        principal,
                        signed_at: sig.signed_at,
                    };
                }
            }
            SignatureOutcome::OkUnverifiedAgent {
                signer: sig.signer.clone(),
                signed_at: sig.signed_at,
            }
        }
        SignerRole::Principal => SignatureOutcome::Ok {
            signer: sig.signer.clone(),
            signer_role: sig.signer_role,
            signed_at: sig.signed_at,
        },
    }
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
