//! Value objects for cryptographic identity.
//!
//! Each is an immutable newtype that validates its invariants at construction.
//! Once constructed, a value of any of these types is by definition well-formed.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::codec::{decode_ed25519_multibase, encode_ed25519_multibase, CodecError};

// ---------------------------------------------------------------------------
// Did
// ---------------------------------------------------------------------------

/// A decentralized identifier. Two methods are supported:
///
/// - **`did:web:<host>[:<path>...]`** — resolves to a static JSON document
///   hosted at the implied HTTPS URL. Stable identity tied to a domain.
///   Survives key rotation (you republish the document).
/// - **`did:key:z<multibase>`** — embeds the public key directly in the DID
///   string. Resolution is purely cryptographic (no network). Zero hosting.
///   Rotating keys means a new DID; previously-issued stamps stay verifiable.
///
/// Construction validates the format. Construction may fail with
/// [`DidParseError`]; once a `Did` exists, it is guaranteed parseable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Did(String);

/// Which DID method a value uses. Returned by [`Did::method`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DidMethod {
    Web,
    Key,
}

#[derive(Debug, Error)]
pub enum DidParseError {
    #[error("DID must start with `did:`")]
    MissingDidPrefix,
    #[error("only the `did:web` and `did:key` methods are supported (got `{0}`)")]
    UnsupportedMethod(String),
    #[error("DID is missing the host component")]
    MissingHost,
    #[error("DID host is invalid: `{0}`")]
    InvalidHost(String),
    #[error("did:key body must be a non-empty multibase string")]
    EmptyKeyBody,
    #[error("did:key body is not a valid ed25519-pub multibase: {0}")]
    InvalidKeyEncoding(#[source] CodecError),
}

impl Did {
    /// Construct a `Did` from a string. Validates the method and shape.
    pub fn parse(s: impl Into<String>) -> Result<Self, DidParseError> {
        let s: String = s.into();

        let body = s
            .strip_prefix("did:")
            .ok_or(DidParseError::MissingDidPrefix)?;

        let (method, rest) = body
            .split_once(':')
            .ok_or_else(|| DidParseError::UnsupportedMethod(body.to_string()))?;

        match method {
            "web" => Self::validate_web(rest)?,
            "key" => Self::validate_key(rest)?,
            other => return Err(DidParseError::UnsupportedMethod(other.to_string())),
        }
        Ok(Did(s))
    }

    /// Build a `did:key` from a raw 32-byte ed25519 verifying key.
    pub fn from_ed25519_public_key(public_key: &[u8; 32]) -> Self {
        let mb = encode_ed25519_multibase(public_key);
        Did(format!("did:key:{mb}"))
    }

    fn validate_web(rest: &str) -> Result<(), DidParseError> {
        let host = rest.split(':').next().unwrap_or("");
        if host.is_empty() {
            return Err(DidParseError::MissingHost);
        }
        if host
            .chars()
            .any(|c| !(c.is_ascii_alphanumeric() || c == '.' || c == '-'))
        {
            return Err(DidParseError::InvalidHost(host.to_string()));
        }
        Ok(())
    }

    fn validate_key(rest: &str) -> Result<(), DidParseError> {
        if rest.is_empty() {
            return Err(DidParseError::EmptyKeyBody);
        }
        decode_ed25519_multibase(rest).map_err(DidParseError::InvalidKeyEncoding)?;
        Ok(())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn method(&self) -> DidMethod {
        if self.0.starts_with("did:key:") {
            DidMethod::Key
        } else {
            DidMethod::Web
        }
    }

    /// For `did:web`: the URL where the DID document should be hosted.
    /// Spec: `did:web:example.com` → `https://example.com/.well-known/did.json`.
    /// `did:web:example.com:user:rafa` → `https://example.com/user/rafa/did.json`.
    /// Returns `None` for non-`did:web` values.
    pub fn web_document_url(&self) -> Option<String> {
        let body = self.0.strip_prefix("did:web:")?;
        let segments: Vec<&str> = body.split(':').collect();
        Some(if segments.len() == 1 {
            format!("https://{}/.well-known/did.json", segments[0])
        } else {
            format!("https://{}/{}/did.json", segments[0], segments[1..].join("/"))
        })
    }

    /// For `did:key`: extract the embedded ed25519 verifying key.
    /// Returns `None` for non-`did:key` values.
    pub fn embedded_ed25519_key(&self) -> Option<[u8; 32]> {
        let mb = self.0.strip_prefix("did:key:")?;
        // Re-decoding here is cheap and the constructor already validated.
        decode_ed25519_multibase(mb).ok()
    }
}

impl From<Did> for String {
    fn from(d: Did) -> String {
        d.0
    }
}

impl TryFrom<String> for Did {
    type Error = DidParseError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Did::parse(s)
    }
}

impl std::fmt::Display for Did {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// DocHash
// ---------------------------------------------------------------------------

/// SHA-256 digest of a document's canonical body (32 bytes).
///
/// Serializes as `sha256:<hex>`, the on-disk representation. Constructing from
/// raw bytes never fails; constructing from the prefixed string fails on
/// malformed input.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DocHash([u8; 32]);

#[derive(Debug, Error)]
pub enum DocHashParseError {
    #[error("doc hash must start with `sha256:`")]
    MissingAlgorithmPrefix,
    #[error("doc hash hex must decode to exactly 32 bytes")]
    WrongLength,
    #[error("doc hash hex is invalid: {0}")]
    InvalidHex(#[from] hex::FromHexError),
}

impl DocHash {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        DocHash(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn parse(s: &str) -> Result<Self, DocHashParseError> {
        let hex = s
            .strip_prefix("sha256:")
            .ok_or(DocHashParseError::MissingAlgorithmPrefix)?;
        let bytes = hex::decode(hex)?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| DocHashParseError::WrongLength)?;
        Ok(DocHash(arr))
    }
}

impl Serialize for DocHash {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for DocHash {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        DocHash::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for DocHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sha256:{}", hex::encode(self.0))
    }
}

// ---------------------------------------------------------------------------
// Signature
// ---------------------------------------------------------------------------

/// Detached ed25519 signature (64 bytes), serialized as `ed25519:<base64>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature([u8; 64]);

#[derive(Debug, Error)]
pub enum SignatureParseError {
    #[error("signature must start with `ed25519:`")]
    MissingAlgorithmPrefix,
    #[error("signature must decode to exactly 64 bytes")]
    WrongLength,
    #[error("signature base64 is invalid: {0}")]
    InvalidBase64(#[from] base64::DecodeError),
}

impl Signature {
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Signature(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 64] {
        &self.0
    }

    pub fn parse(s: &str) -> Result<Self, SignatureParseError> {
        let b64 = s
            .strip_prefix("ed25519:")
            .ok_or(SignatureParseError::MissingAlgorithmPrefix)?;
        let bytes = B64.decode(b64)?;
        let arr: [u8; 64] = bytes
            .try_into()
            .map_err(|_| SignatureParseError::WrongLength)?;
        Ok(Signature(arr))
    }
}

impl Serialize for Signature {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Signature::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ed25519:{}", B64.encode(self.0))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn did_web_simple() {
        let d = Did::parse("did:web:rafa.equanimi.tech").unwrap();
        assert_eq!(d.method(), DidMethod::Web);
        assert_eq!(
            d.web_document_url().unwrap(),
            "https://rafa.equanimi.tech/.well-known/did.json"
        );
        assert!(d.embedded_ed25519_key().is_none());
    }

    #[test]
    fn did_web_path() {
        let d = Did::parse("did:web:equanimi.tech:user:rafa").unwrap();
        assert_eq!(d.method(), DidMethod::Web);
        assert_eq!(
            d.web_document_url().unwrap(),
            "https://equanimi.tech/user/rafa/did.json"
        );
    }

    #[test]
    fn did_rejects_unsupported_method() {
        assert!(matches!(
            Did::parse("did:plc:abc"),
            Err(DidParseError::UnsupportedMethod(_))
        ));
    }

    #[test]
    fn did_key_from_public_key_roundtrip() {
        let pk = [0xab_u8; 32];
        let d = Did::from_ed25519_public_key(&pk);
        assert_eq!(d.method(), DidMethod::Key);
        assert!(d.as_str().starts_with("did:key:z"));
        assert_eq!(d.embedded_ed25519_key().unwrap(), pk);
        // did:key has no document URL.
        assert!(d.web_document_url().is_none());

        // Round-trip through Did::parse.
        let reparsed = Did::parse(d.as_str()).unwrap();
        assert_eq!(reparsed, d);
    }

    #[test]
    fn did_key_rejects_empty_body() {
        assert!(matches!(
            Did::parse("did:key:"),
            Err(DidParseError::EmptyKeyBody)
        ));
    }

    #[test]
    fn did_key_rejects_malformed_multibase() {
        assert!(matches!(
            Did::parse("did:key:not-a-real-multibase"),
            Err(DidParseError::InvalidKeyEncoding(_))
        ));
    }

    #[test]
    fn doc_hash_roundtrip() {
        let h = DocHash::from_bytes([0xab; 32]);
        let s = h.to_string();
        let parsed = DocHash::parse(&s).unwrap();
        assert_eq!(h, parsed);
    }

    #[test]
    fn signature_roundtrip() {
        let sig = Signature::from_bytes([0x42; 64]);
        let s = sig.to_string();
        let parsed = Signature::parse(&s).unwrap();
        assert_eq!(sig, parsed);
    }
}
