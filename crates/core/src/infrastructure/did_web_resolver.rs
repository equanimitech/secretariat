//! `did:web` resolver with on-disk cache.
//!
//! Decision log #9: trust-on-first-use, no TTL at MVP. Cache lives at
//! `~/.secretariat/peers/<sanitized-did>.json`. Manual delete to refresh.
//!
//! For tests, [`DidWebResolver::resolve`] reads the cache before fetching,
//! so a test that pre-populates `peers/` never touches the network.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

use crate::codec::decode_ed25519_multibase;
use crate::domain::{Did, DidMethod};
use crate::ports::{DidResolutionError, DidResolver, ResolvedDid};

const ED25519_VERIFICATION_METHOD_TYPE: &str = "Ed25519VerificationKey2020";

pub struct DidWebResolver {
    cache_dir: PathBuf,
    http: reqwest::blocking::Client,
}

impl DidWebResolver {
    pub fn new(cache_dir: PathBuf) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("secretariat/0.1")
            .build()
            .expect("reqwest client build cannot fail with these options");
        Self { cache_dir, http }
    }

    pub fn cache_path(&self, did: &Did) -> PathBuf {
        let safe = sanitize_did_for_filename(did.as_str());
        self.cache_dir.join(format!("{safe}.json"))
    }
}

impl DidResolver for DidWebResolver {
    fn resolve(&self, did: &Did) -> Result<ResolvedDid, DidResolutionError> {
        if did.method() != DidMethod::Web {
            return Err(DidResolutionError::Malformed(format!(
                "DidWebResolver cannot resolve {} (use CompositeDidResolver)",
                did.as_str()
            )));
        }
        // 1. Cache first.
        let cache = self.cache_path(did);
        let raw = if cache.exists() {
            fs::read_to_string(&cache)
                .map_err(|e| DidResolutionError::Network(format!("cache read: {e}")))?
        } else {
            // 2. Fetch over HTTPS.
            let url = did
                .web_document_url()
                .expect("method() == Web implies web_document_url() == Some");
            let resp = self
                .http
                .get(&url)
                .send()
                .map_err(|e| DidResolutionError::Network(e.to_string()))?;
            if resp.status() == reqwest::StatusCode::NOT_FOUND {
                return Err(DidResolutionError::NotFound { url });
            }
            if !resp.status().is_success() {
                return Err(DidResolutionError::Network(format!(
                    "GET {} returned {}",
                    url,
                    resp.status()
                )));
            }
            let body = resp
                .text()
                .map_err(|e| DidResolutionError::Network(e.to_string()))?;

            // 3. Persist cache.
            fs::create_dir_all(&self.cache_dir)
                .map_err(|e| DidResolutionError::Network(format!("cache mkdir: {e}")))?;
            fs::write(&cache, &body)
                .map_err(|e| DidResolutionError::Network(format!("cache write: {e}")))?;
            body
        };

        let doc: Value = serde_json::from_str(&raw)
            .map_err(|e| DidResolutionError::Malformed(format!("json: {e}")))?;

        let keys = extract_ed25519_keys(&doc)?;
        if keys.is_empty() {
            return Err(DidResolutionError::NoEd25519Key);
        }

        Ok(ResolvedDid {
            did: did.clone(),
            stamp_public_keys: keys,
            raw_document: doc,
        })
    }
}

fn extract_ed25519_keys(doc: &Value) -> Result<Vec<[u8; 32]>, DidResolutionError> {
    let methods = doc
        .get("verificationMethod")
        .and_then(|v| v.as_array())
        .ok_or_else(|| DidResolutionError::Malformed("missing verificationMethod".into()))?;

    let mut out = Vec::new();
    for vm in methods {
        let ty = vm.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if ty != ED25519_VERIFICATION_METHOD_TYPE {
            continue;
        }
        let mb = vm
            .get("publicKeyMultibase")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                DidResolutionError::Malformed(
                    "verificationMethod missing publicKeyMultibase".into(),
                )
            })?;
        let raw = decode_ed25519_multibase(mb)
            .map_err(|e| DidResolutionError::Malformed(format!("multibase: {e}")))?;
        out.push(raw);
    }

    Ok(out)
}

/// Filesystem-safe encoding of a DID. Replaces `:` and `/` with `_`.
pub fn sanitize_did_for_filename(did: &str) -> String {
    did.chars()
        .map(|c| match c {
            ':' | '/' | '\\' => '_',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::keys::{generate_keypair, write_did_document};
    use tempfile::TempDir;

    #[test]
    fn resolves_from_local_cache_without_network() {
        let dir = TempDir::new().unwrap();
        let cache = dir.path().to_path_buf();
        let did = Did::parse("did:web:rafa.equanimi.tech").unwrap();
        let key = generate_keypair();

        // Pre-populate the cache by writing a did.json there directly.
        let resolver = DidWebResolver::new(cache.clone());
        write_did_document(&resolver.cache_path(&did), &did, &key.verifying_key()).unwrap();

        let resolved = resolver.resolve(&did).unwrap();
        assert_eq!(resolved.did, did);
        assert_eq!(resolved.stamp_public_keys.len(), 1);
        assert_eq!(resolved.stamp_public_keys[0], key.verifying_key().to_bytes());
    }

    #[test]
    fn rejects_doc_with_no_ed25519_key() {
        let dir = TempDir::new().unwrap();
        let cache = dir.path().to_path_buf();
        let did = Did::parse("did:web:rafa.equanimi.tech").unwrap();

        let resolver = DidWebResolver::new(cache);
        let path = resolver.cache_path(&did);
        let doc = serde_json::json!({
            "id": did.as_str(),
            "verificationMethod": [{
                "id": "x",
                "type": "RsaVerificationKey2018",
                "controller": did.as_str(),
            }],
        });
        fs::write(&path, doc.to_string()).unwrap();

        let r = resolver.resolve(&did);
        assert!(matches!(r, Err(DidResolutionError::NoEd25519Key)));
    }
}
