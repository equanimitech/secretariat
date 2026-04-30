//! Value objects for cryptographic identity.
//!
//! Each is an immutable newtype that validates its invariants at construction.
//! Once constructed, a value of any of these types is by definition well-formed.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Did
// ---------------------------------------------------------------------------

/// A decentralized identifier, restricted at MVP to `did:web:<host>[:<path>]`.
///
/// Construction validates the format. Construction may fail with
/// [`DidParseError`]; once a `Did` exists, it is guaranteed parseable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Did(String);

#[derive(Debug, Error)]
pub enum DidParseError {
    #[error("DID must start with `did:`")]
    MissingDidPrefix,
    #[error("Only the `did:web` method is supported in this build (got `{0}`)")]
    UnsupportedMethod(String),
    #[error("DID is missing the host component")]
    MissingHost,
    #[error("DID host is invalid: `{0}`")]
    InvalidHost(String),
}

impl Did {
    /// Construct a `Did` from a string. Validates `did:web:<host>` shape.
    pub fn parse(s: impl Into<String>) -> Result<Self, DidParseError> {
        let s: String = s.into();

        let body = s
            .strip_prefix("did:")
            .ok_or(DidParseError::MissingDidPrefix)?;

        let (method, rest) = body
            .split_once(':')
            .ok_or_else(|| DidParseError::UnsupportedMethod(body.to_string()))?;

        if method != "web" {
            return Err(DidParseError::UnsupportedMethod(method.to_string()));
        }

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

        Ok(Did(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the URL where the DID document should be hosted (`did:web` resolution).
    /// Spec: `did:web:example.com` → `https://example.com/.well-known/did.json`.
    /// `did:web:example.com:user:rafa` → `https://example.com/user/rafa/did.json`.
    pub fn to_did_document_url(&self) -> String {
        let body = self.0.strip_prefix("did:web:").expect("validated at parse");
        let segments: Vec<&str> = body.split(':').collect();
        if segments.len() == 1 {
            format!("https://{}/.well-known/did.json", segments[0])
        } else {
            format!("https://{}/{}/did.json", segments[0], segments[1..].join("/"))
        }
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
        assert_eq!(
            d.to_did_document_url(),
            "https://rafa.equanimi.tech/.well-known/did.json"
        );
    }

    #[test]
    fn did_web_path() {
        let d = Did::parse("did:web:equanimi.tech:user:rafa").unwrap();
        assert_eq!(d.to_did_document_url(), "https://equanimi.tech/user/rafa/did.json");
    }

    #[test]
    fn did_rejects_unsupported_method() {
        assert!(matches!(
            Did::parse("did:plc:abc"),
            Err(DidParseError::UnsupportedMethod(_))
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
