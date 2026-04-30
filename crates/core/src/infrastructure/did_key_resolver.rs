//! `did:key` resolver. Resolution is purely cryptographic — the DID *is* the
//! public key (multibase-encoded with the `ed25519-pub` multicodec prefix).
//! No network, no cache, no IO.

use crate::domain::{Did, DidMethod};
use crate::ports::{DidResolutionError, DidResolver, ResolvedDid};

#[derive(Debug, Default, Clone, Copy)]
pub struct DidKeyResolver;

impl DidResolver for DidKeyResolver {
    fn resolve(&self, did: &Did) -> Result<ResolvedDid, DidResolutionError> {
        if did.method() != DidMethod::Key {
            return Err(DidResolutionError::Malformed(format!(
                "DidKeyResolver cannot resolve {} (use CompositeDidResolver)",
                did.as_str()
            )));
        }
        let key = did.embedded_ed25519_key().ok_or(DidResolutionError::NoEd25519Key)?;
        Ok(ResolvedDid {
            did: did.clone(),
            stamp_public_keys: vec![key],
            // For did:key the "document" is implicit; emit a synthesized one so
            // downstream code with `raw_document` access still works.
            raw_document: serde_json::json!({
                "@context": ["https://www.w3.org/ns/did/v1"],
                "id": did.as_str(),
                "verificationMethod": [{
                    "id": format!("{}#0", did.as_str()),
                    "type": "Ed25519VerificationKey2020",
                    "controller": did.as_str(),
                    "publicKeyMultibase": did.as_str().strip_prefix("did:key:").unwrap_or(""),
                }],
                "assertionMethod": [format!("{}#0", did.as_str())],
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_a_did_key() {
        let pk = [0xab_u8; 32];
        let did = Did::from_ed25519_public_key(&pk);
        let r = DidKeyResolver.resolve(&did).unwrap();
        assert_eq!(r.stamp_public_keys.len(), 1);
        assert_eq!(r.stamp_public_keys[0], pk);
    }

    #[test]
    fn rejects_non_did_key() {
        let did = Did::parse("did:web:rafa.equanimi.tech").unwrap();
        let r = DidKeyResolver.resolve(&did);
        assert!(matches!(r, Err(DidResolutionError::Malformed(_))));
    }
}
