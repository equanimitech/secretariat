//! Contact — a known peer the principal can correspond with.
//!
//! A contact links a [`Did`] (cryptographic identity) to the operational
//! address needed to reach that peer over a transport. v0 carries an
//! optional [`RelayEndpoint`]:
//!
//! - For `did:web` peers, the endpoint is `None` — the recipient's relay
//!   is discovered live from their DID document's `serviceEndpoint`.
//! - For `did:key` peers (no published document), the endpoint is `Some(_)`
//!   — exchanged out-of-band when the contact is added.
//!
//! Future versions will add transport-specific addresses (Iroh peer-id,
//! Slack workspace+user, etc.) as additional optional fields on the same
//! aggregate, and the `Transport` trait will pick the best mutual one.
//!
//! The contact book is the consistency boundary: at most one entry per DID,
//! at most one entry per (lowercased) display-name slug. See
//! `infrastructure::contact_store` for persistence.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::Did;

// ---------------------------------------------------------------------------
// RelayEndpoint
// ---------------------------------------------------------------------------

/// A Secretariat relay URL — where to POST envelopes for / poll envelopes
/// from a peer. Validates scheme + non-empty authority + no whitespace.
///
/// Production schemes: `wss://`, `https://`. Dev-only schemes (`ws://`,
/// `http://`) are accepted by the parser; the transport layer is responsible
/// for refusing insecure schemes outside of dev/loopback.
///
/// This is *not* full URL parsing — when the relay client actually opens a
/// connection it'll parse with the `url` crate (transitively via reqwest /
/// tokio-tungstenite). The value object rejects only obvious garbage so
/// errors happen at construction, not three layers down a network call.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RelayEndpoint(String);

#[derive(Debug, Error)]
pub enum RelayEndpointParseError {
    #[error("relay endpoint is empty")]
    Empty,
    #[error("relay endpoint must use wss/https/ws/http scheme (got `{0}`)")]
    UnsupportedScheme(String),
    #[error("relay endpoint missing authority (host) after scheme")]
    MissingAuthority,
    #[error("relay endpoint contains whitespace or control character")]
    InvalidChar,
    #[error("relay endpoint exceeds {0} characters")]
    TooLong(usize),
}

const RELAY_ENDPOINT_MAX: usize = 1024;

impl RelayEndpoint {
    pub fn parse(s: impl Into<String>) -> Result<Self, RelayEndpointParseError> {
        let s: String = s.into();
        let s = s.trim().to_string();

        if s.is_empty() {
            return Err(RelayEndpointParseError::Empty);
        }
        if s.len() > RELAY_ENDPOINT_MAX {
            return Err(RelayEndpointParseError::TooLong(RELAY_ENDPOINT_MAX));
        }
        if s.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(RelayEndpointParseError::InvalidChar);
        }

        let scheme_end = s
            .find("://")
            .ok_or_else(|| RelayEndpointParseError::UnsupportedScheme(s.clone()))?;
        let scheme = &s[..scheme_end];
        match scheme {
            "wss" | "https" | "ws" | "http" => {}
            _ => return Err(RelayEndpointParseError::UnsupportedScheme(scheme.to_string())),
        }

        let after_scheme = &s[scheme_end + 3..];
        // Authority ends at first `/`, `?`, or `#`.
        let authority_end = after_scheme
            .find(['/', '?', '#'])
            .unwrap_or(after_scheme.len());
        let authority = &after_scheme[..authority_end];
        if authority.is_empty() {
            return Err(RelayEndpointParseError::MissingAuthority);
        }

        Ok(RelayEndpoint(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn scheme(&self) -> &str {
        let end = self.0.find("://").unwrap();
        &self.0[..end]
    }

    /// Returns true if this endpoint uses an encrypted scheme (`wss`, `https`).
    pub fn is_secure(&self) -> bool {
        matches!(self.scheme(), "wss" | "https")
    }
}

impl Serialize for RelayEndpoint {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RelayEndpoint {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        RelayEndpoint::parse(s).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for RelayEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// DisplayName
// ---------------------------------------------------------------------------

/// Human-friendly nickname for a contact (e.g. "Marcelo"). Non-empty after
/// trim, no control characters, bounded length. UTF-8 friendly — nicknames
/// in any script are fine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DisplayName(String);

#[derive(Debug, Error)]
pub enum DisplayNameParseError {
    #[error("display name is empty")]
    Empty,
    #[error("display name exceeds {0} characters")]
    TooLong(usize),
    #[error("display name contains a control character")]
    ControlChar,
}

const DISPLAY_NAME_MAX: usize = 200;

impl DisplayName {
    pub fn parse(s: impl Into<String>) -> Result<Self, DisplayNameParseError> {
        let s: String = s.into();
        let s = s.trim().to_string();
        if s.is_empty() {
            return Err(DisplayNameParseError::Empty);
        }
        if s.chars().count() > DISPLAY_NAME_MAX {
            return Err(DisplayNameParseError::TooLong(DISPLAY_NAME_MAX));
        }
        if s.chars().any(|c| c.is_control()) {
            return Err(DisplayNameParseError::ControlChar);
        }
        Ok(DisplayName(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Lowercased ASCII slug for uniqueness checks and CLI lookups.
    /// Non-ASCII characters pass through lowercased; this is identity, not
    /// security, so collision risk is acceptable.
    pub fn slug(&self) -> String {
        self.0
            .to_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    }
}

impl Serialize for DisplayName {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for DisplayName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        DisplayName::parse(s).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for DisplayName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Contact
// ---------------------------------------------------------------------------

/// A known peer: cryptographic identity + transport address.
///
/// `relay_endpoint` is `None` when the peer's relay can be discovered live
/// from their DID document (`did:web` with a `serviceEndpoint`), or
/// `Some(_)` when it was exchanged out-of-band (`did:key` peers, or
/// `did:web` peers whose relay hasn't been published yet).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    pub did: Did,
    pub display_name: DisplayName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_endpoint: Option<RelayEndpoint>,
}

impl Contact {
    pub fn new(
        did: Did,
        display_name: DisplayName,
        relay_endpoint: Option<RelayEndpoint>,
    ) -> Self {
        Contact {
            did,
            display_name,
            relay_endpoint,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_endpoint_wss() {
        let e = RelayEndpoint::parse("wss://relay.rafa.equanimi.tech").unwrap();
        assert_eq!(e.as_str(), "wss://relay.rafa.equanimi.tech");
        assert_eq!(e.scheme(), "wss");
        assert!(e.is_secure());
    }

    #[test]
    fn relay_endpoint_https() {
        let e = RelayEndpoint::parse("https://relay.rafa.equanimi.tech:8443/v0").unwrap();
        assert_eq!(e.scheme(), "https");
        assert!(e.is_secure());
    }

    #[test]
    fn relay_endpoint_ws_dev() {
        let e = RelayEndpoint::parse("ws://localhost:8080").unwrap();
        assert_eq!(e.scheme(), "ws");
        assert!(!e.is_secure());
    }

    #[test]
    fn relay_endpoint_http_dev() {
        let e = RelayEndpoint::parse("http://127.0.0.1:8080/inbox").unwrap();
        assert_eq!(e.scheme(), "http");
        assert!(!e.is_secure());
    }

    #[test]
    fn relay_endpoint_trims() {
        let e = RelayEndpoint::parse("  wss://relay.example.com  ").unwrap();
        assert_eq!(e.as_str(), "wss://relay.example.com");
    }

    #[test]
    fn relay_endpoint_rejects_empty() {
        assert!(matches!(
            RelayEndpoint::parse(""),
            Err(RelayEndpointParseError::Empty)
        ));
    }

    #[test]
    fn relay_endpoint_rejects_unknown_scheme() {
        assert!(matches!(
            RelayEndpoint::parse("ftp://relay.example.com"),
            Err(RelayEndpointParseError::UnsupportedScheme(_))
        ));
    }

    #[test]
    fn relay_endpoint_rejects_missing_scheme() {
        assert!(matches!(
            RelayEndpoint::parse("relay.example.com"),
            Err(RelayEndpointParseError::UnsupportedScheme(_))
        ));
    }

    #[test]
    fn relay_endpoint_rejects_missing_authority() {
        assert!(matches!(
            RelayEndpoint::parse("wss:///"),
            Err(RelayEndpointParseError::MissingAuthority)
        ));
    }

    #[test]
    fn relay_endpoint_rejects_whitespace() {
        assert!(matches!(
            RelayEndpoint::parse("wss://re lay.example.com"),
            Err(RelayEndpointParseError::InvalidChar)
        ));
    }

    #[test]
    fn relay_endpoint_rejects_too_long() {
        let huge = format!("wss://{}", "x".repeat(RELAY_ENDPOINT_MAX));
        assert!(matches!(
            RelayEndpoint::parse(huge),
            Err(RelayEndpointParseError::TooLong(_))
        ));
    }

    #[test]
    fn relay_endpoint_serde_roundtrip() {
        let e = RelayEndpoint::parse("wss://relay.rafa.equanimi.tech").unwrap();
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(json, r#""wss://relay.rafa.equanimi.tech""#);
        let back: RelayEndpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn display_name_basic() {
        let n = DisplayName::parse("Marcelo").unwrap();
        assert_eq!(n.as_str(), "Marcelo");
        assert_eq!(n.slug(), "marcelo");
    }

    #[test]
    fn display_name_trims() {
        let n = DisplayName::parse("  Marcelo  ").unwrap();
        assert_eq!(n.as_str(), "Marcelo");
    }

    #[test]
    fn display_name_unicode_ok() {
        let n = DisplayName::parse("Ramón").unwrap();
        assert_eq!(n.as_str(), "Ramón");
    }

    #[test]
    fn display_name_slug_strips_whitespace() {
        let n = DisplayName::parse("Marcelo Ballestiero").unwrap();
        assert_eq!(n.slug(), "marceloballestiero");
    }

    #[test]
    fn display_name_rejects_empty() {
        assert!(matches!(
            DisplayName::parse(""),
            Err(DisplayNameParseError::Empty)
        ));
    }

    #[test]
    fn display_name_rejects_whitespace_only() {
        assert!(matches!(
            DisplayName::parse("   "),
            Err(DisplayNameParseError::Empty)
        ));
    }

    #[test]
    fn display_name_rejects_too_long() {
        let huge = "x".repeat(DISPLAY_NAME_MAX + 1);
        assert!(matches!(
            DisplayName::parse(huge),
            Err(DisplayNameParseError::TooLong(_))
        ));
    }

    #[test]
    fn display_name_rejects_control_char() {
        assert!(matches!(
            DisplayName::parse("Marcelo\nBallestiero"),
            Err(DisplayNameParseError::ControlChar)
        ));
    }

    #[test]
    fn contact_serde_roundtrip_with_endpoint() {
        let c = Contact::new(
            Did::parse("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK").unwrap(),
            DisplayName::parse("Marcelo").unwrap(),
            Some(RelayEndpoint::parse("wss://relay.rafa.equanimi.tech").unwrap()),
        );
        let json = serde_json::to_string(&c).unwrap();
        let back: Contact = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn contact_serde_roundtrip_without_endpoint() {
        // did:web peer — relay discovered live from DID document
        let c = Contact::new(
            Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            DisplayName::parse("Rafa").unwrap(),
            None,
        );
        let json = serde_json::to_string(&c).unwrap();
        // Optional field with None should be skipped in serialization.
        assert!(!json.contains("relay_endpoint"));
        let back: Contact = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
        assert!(back.relay_endpoint.is_none());
    }
}
