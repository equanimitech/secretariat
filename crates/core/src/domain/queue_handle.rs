//! `QueueHandle` — value object naming a queue on its owner's machine.
//!
//! Every `Recipient` is a `(owner, handle)` pair (queues-as-primitive,
//! 2026-05-05). The handle picks which queue on the owner's disk; the
//! owner DID picks whose disk.
//!
//! Grammar (bare slugs, 2026-05-21 substrate-for-themia collapse):
//! `^<seg>(:<seg>)*$` where each `<seg>` matches `[a-z_][a-z0-9_-]*`.
//! Tree depth = colon depth. The handle is a path-shaped slug — no
//! namespace prefix, no recognized vocabulary of top-level tokens.
//!
//! The root the handle resolves under (org-scoped vs principal-scoped)
//! is carried by the `Recipient`'s owner DID and the org-membership
//! index, NOT by the handle itself. `assemblee_generale` is the handle;
//! whether it lives at `orgs/themia/channels/assemblee_generale/` or
//! `channels/assemblee_generale/` is decided by the queue-dir resolver.
//!
//! Examples:
//! - `triage` — single-segment handle
//! - `articles` — single-segment, any root
//! - `dommage-corporel:paris-cohort` — nested handle (colon = path slash)
//!
//! The legacy `channel:` / `inbox:` / `peer:` / `area:` / `project:`
//! prefixes from v0.2 / v0.3 are gone. See
//! `docs/pitches/2026-05-21-substrate-for-themia.md` element §2.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueHandleError {
    #[error("queue handle is empty")]
    Empty,
    #[error("queue handle has empty segment between `:` separators")]
    EmptySegment,
    #[error("queue handle segment must start with a lowercase letter or `_` and contain only `[a-z0-9_-]`")]
    InvalidChars,
    #[error("queue handle exceeds 64-byte length")]
    TooLong,
}

/// Maximum total length of a handle, including all segments + separators.
const MAX_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueueHandle(String);

impl QueueHandle {
    /// Parse a handle string with full validation.
    ///
    /// Bare slug grammar: `^<seg>(:<seg>)*$`. Colons are path nesting
    /// separators, NOT namespace markers — the first segment carries no
    /// special meaning.
    pub fn parse(s: &str) -> Result<Self, QueueHandleError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(QueueHandleError::Empty);
        }
        if s.len() > MAX_LEN {
            return Err(QueueHandleError::TooLong);
        }

        for seg in s.split(':') {
            if seg.is_empty() {
                return Err(QueueHandleError::EmptySegment);
            }
            validate_segment(seg)?;
        }

        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// All colon-separated segments in order.
    pub fn segments(&self) -> Vec<&str> {
        self.0.split(':').collect()
    }

    /// Path-safe form: `triage` → `triage`,
    /// `dommage-corporel:paris-cohort` → `dommage-corporel/paris-cohort`.
    pub fn as_path_segment(&self) -> String {
        self.0.replace(':', "/")
    }
}

fn validate_segment(seg: &str) -> Result<(), QueueHandleError> {
    let mut chars = seg.chars();
    let Some(first) = chars.next() else {
        return Err(QueueHandleError::EmptySegment);
    };
    // First char of a segment: lowercase letter only.
    // Leading underscore allowed for substrate-private segments (`_meta`, `_org`).
    if !(first.is_ascii_lowercase() || first == '_') {
        return Err(QueueHandleError::InvalidChars);
    }
    for c in chars {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
            return Err(QueueHandleError::InvalidChars);
        }
    }
    Ok(())
}

impl fmt::Display for QueueHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for QueueHandle {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for QueueHandle {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        QueueHandle::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_segment() {
        let h = QueueHandle::parse("triage").unwrap();
        assert_eq!(h.as_str(), "triage");
        assert_eq!(h.segments(), vec!["triage"]);
        assert_eq!(h.as_path_segment(), "triage");
    }

    #[test]
    fn parses_nested_handle() {
        let h = QueueHandle::parse("dommage-corporel:paris-cohort").unwrap();
        assert_eq!(h.segments(), vec!["dommage-corporel", "paris-cohort"]);
        assert_eq!(h.as_path_segment(), "dommage-corporel/paris-cohort");
    }

    #[test]
    fn parses_meta_segment_with_leading_underscore() {
        // Substrate-private segments allowed (`_meta`, `_org`).
        let h = QueueHandle::parse("secretariat:_meta").unwrap();
        assert_eq!(h.segments(), vec!["secretariat", "_meta"]);
    }

    #[test]
    fn accepts_freeform_slugs() {
        // No recognized namespace vocabulary — any bare slug is legal.
        assert!(QueueHandle::parse("writing").is_ok());
        assert!(QueueHandle::parse("secretariat").is_ok());
        assert!(QueueHandle::parse("marcelo").is_ok());
        assert!(QueueHandle::parse("christophe-marchand").is_ok());
        assert!(QueueHandle::parse("assemblee_generale").is_ok());
    }

    #[test]
    fn allows_digits_and_hyphens_in_segments() {
        assert!(QueueHandle::parse("to-self").is_ok());
        assert!(QueueHandle::parse("slug-1").is_ok());
        assert!(QueueHandle::parse("abc123").is_ok());
        assert!(QueueHandle::parse("cohort-2026:q2").is_ok());
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(
            QueueHandle::parse(""),
            Err(QueueHandleError::Empty)
        ));
        assert!(matches!(
            QueueHandle::parse("   "),
            Err(QueueHandleError::Empty)
        ));
    }

    #[test]
    fn accepts_handle_without_separator() {
        // A bare identifier is a legal handle — `inbox-triage` parses
        // as a single segment, not as a pre-colon namespace.
        assert!(QueueHandle::parse("inbox-triage").is_ok());
    }

    #[test]
    fn rejects_empty_leading_segment() {
        // A leading `:` produces an empty first segment, which is invalid.
        assert!(matches!(
            QueueHandle::parse(":triage"),
            Err(QueueHandleError::EmptySegment)
        ));
    }

    #[test]
    fn rejects_empty_trailing_segment() {
        assert!(matches!(
            QueueHandle::parse("triage:"),
            Err(QueueHandleError::EmptySegment)
        ));
    }

    #[test]
    fn rejects_empty_middle_segment() {
        assert!(matches!(
            QueueHandle::parse("dommage-corporel::paris"),
            Err(QueueHandleError::EmptySegment)
        ));
    }

    #[test]
    fn rejects_segment_starting_with_digit_or_hyphen() {
        assert!(matches!(
            QueueHandle::parse("1triage"),
            Err(QueueHandleError::InvalidChars)
        ));
        assert!(matches!(
            QueueHandle::parse("-triage"),
            Err(QueueHandleError::InvalidChars)
        ));
    }

    #[test]
    fn rejects_uppercase_anywhere() {
        assert!(matches!(
            QueueHandle::parse("Triage"),
            Err(QueueHandleError::InvalidChars)
        ));
        assert!(matches!(
            QueueHandle::parse("dommage:Corporel"),
            Err(QueueHandleError::InvalidChars)
        ));
        assert!(matches!(
            QueueHandle::parse("dept:SubTeam"),
            Err(QueueHandleError::InvalidChars)
        ));
    }

    #[test]
    fn rejects_invalid_segment_chars() {
        assert!(matches!(
            QueueHandle::parse("to self"),
            Err(QueueHandleError::InvalidChars)
        ));
        assert!(matches!(
            QueueHandle::parse("foo.bar"),
            Err(QueueHandleError::InvalidChars)
        ));
    }

    #[test]
    fn rejects_overlong() {
        let s = format!("triage:{}", "x".repeat(60));
        assert!(matches!(
            QueueHandle::parse(&s),
            Err(QueueHandleError::TooLong)
        ));
    }

    #[test]
    fn display_roundtrip() {
        for s in [
            "triage",
            "writing",
            "secretariat",
            "dommage-corporel:paris-cohort",
            "secretariat:_meta",
        ] {
            let h = QueueHandle::parse(s).unwrap();
            assert_eq!(h.to_string(), s);
        }
    }

    #[test]
    fn serde_roundtrip() {
        let h = QueueHandle::parse("triage").unwrap();
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, "\"triage\"");
        let back: QueueHandle = serde_json::from_str(&json).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn serde_roundtrip_nested() {
        let h = QueueHandle::parse("dommage-corporel:paris-cohort").unwrap();
        let json = serde_json::to_string(&h).unwrap();
        let back: QueueHandle = serde_json::from_str(&json).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn serde_rejects_malformed_string() {
        // Uppercase is structurally invalid under any grammar version.
        let r: Result<QueueHandle, _> = serde_json::from_str("\"NotAValidHandle\"");
        assert!(r.is_err());
    }
}
