//! Dispatches DID resolution by method. The CLI / Tauri shell wires this up
//! once at startup; tests prefer the per-method resolvers directly.

use crate::domain::{Did, DidMethod};
use crate::ports::{DidResolutionError, DidResolver, ResolvedDid};

use super::did_key_resolver::DidKeyResolver;
use super::did_web_resolver::DidWebResolver;

pub struct CompositeDidResolver {
    web: DidWebResolver,
    key: DidKeyResolver,
}

impl CompositeDidResolver {
    pub fn new(web: DidWebResolver) -> Self {
        Self {
            web,
            key: DidKeyResolver,
        }
    }
}

impl DidResolver for CompositeDidResolver {
    fn resolve(&self, did: &Did) -> Result<ResolvedDid, DidResolutionError> {
        match did.method() {
            DidMethod::Web => self.web.resolve(did),
            DidMethod::Key => self.key.resolve(did),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::keys::{generate_keypair, write_did_document};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn dispatches_did_key() {
        let dir = TempDir::new().unwrap();
        let resolver = CompositeDidResolver::new(DidWebResolver::new(dir.path().to_path_buf()));

        let pk = [0xab_u8; 32];
        let did = Did::from_ed25519_public_key(&pk);
        let r = resolver.resolve(&did).unwrap();
        assert_eq!(r.stamp_public_keys[0], pk);
    }

    #[test]
    fn dispatches_did_web_via_cache() {
        let dir = TempDir::new().unwrap();
        let web = DidWebResolver::new(dir.path().to_path_buf());
        let did = Did::parse("did:web:rafa.equanimi.tech").unwrap();
        let key = generate_keypair();

        // Pre-populate the cache.
        let cache_path = web.cache_path(&did);
        write_did_document(&cache_path, &did, &key.verifying_key()).unwrap();
        // Sanity-check the cache file exists.
        assert!(fs::metadata(&cache_path).is_ok());

        let resolver = CompositeDidResolver::new(web);
        let r = resolver.resolve(&did).unwrap();
        assert_eq!(r.stamp_public_keys[0], key.verifying_key().to_bytes());
    }
}
