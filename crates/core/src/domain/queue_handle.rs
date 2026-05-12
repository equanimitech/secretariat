//! `QueueHandle` — value object naming a queue on its owner's machine.
//!
//! Every `Recipient` is a `(owner, handle)` pair (queues-as-primitive,
//! 2026-05-05). The handle picks which queue on the owner's disk; the
//! owner DID picks whose disk. `owner == self_did` keeps the envelope
//! local; otherwise it routes to the owner's relay.
//!
//! Grammar (v0.3, nested): `^<seg>(:<seg>)+$` where each `<seg>` matches
//! `[a-z][a-z0-9-]*`. Tree depth = colon depth.
//!
//! Examples:
//! - `inbox:triage` — flat local capture queue (v0.2 style, still valid)
//! - `area:writing`, `project:secretariat` — flat principal-defined
//! - `channel:dommage-corporel:paris-cohort` — nested channel under an
//!   org's tree (v0.3)
//! - `channel:secretariat:dev:_meta` — meta-queue colocated with a
//!   channel (leading-underscore segments are substrate-private)
//!
//! Namespaces are **free-form** — the parser validates shape only.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueHandleError {
    #[error("queue handle is empty")]
    Empty,
    #[error("queue handle missing `:` separator (expected `<namespace>:<segment>[...]`)")]
    MissingSeparator,
    #[error("queue handle has empty namespace before `:`")]
    EmptyNamespace,
    #[error("queue handle has empty segment between `:` separators")]
    EmptySegment,
    #[error("queue handle segment must start with a lowercase letter and contain only `[a-z0-9-]`")]
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
    pub fn parse(s: &str) -> Result<Self, QueueHandleError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(QueueHandleError::Empty);
        }
        if s.len() > MAX_LEN {
            return Err(QueueHandleError::TooLong);
        }
        if !s.contains(':') {
            return Err(QueueHandleError::MissingSeparator);
        }

        let segments: Vec<&str> = s.split(':').collect();
        if segments.len() < 2 {
            return Err(QueueHandleError::MissingSeparator);
        }

        for (i, seg) in segments.iter().enumerate() {
            if seg.is_empty() {
                return Err(if i == 0 {
                    QueueHandleError::EmptyNamespace
                } else {
                    QueueHandleError::EmptySegment
                });
            }
            validate_segment(seg)?;
        }

        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// First segment — `inbox` for `inbox:triage`,
    /// `channel` for `channel:dommage-corporel:paris-cohort`.
    pub fn top_namespace(&self) -> &str {
        self.0.split(':').next().unwrap()
    }

    /// All segments in order (top namespace first).
    pub fn segments(&self) -> Vec<&str> {
        self.0.split(':').collect()
    }

    /// Backward-compat alias for `top_namespace()`.
    pub fn namespace(&self) -> &str {
        self.top_namespace()
    }

    /// Everything after the first colon — `triage` for `inbox:triage`,
    /// `dommage-corporel:paris-cohort` for the nested example.
    pub fn slug(&self) -> &str {
        self.0.split_once(':').map(|(_, s)| s).unwrap()
    }

    /// Path-safe form: `inbox:triage` → `inbox/triage`,
    /// `channel:dommage-corporel:paris-cohort` →
    /// `channel/dommage-corporel/paris-cohort`.
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
    fn parses_inbox_triage() {
        let h = QueueHandle::parse("inbox:triage").unwrap();
        assert_eq!(h.as_str(), "inbox:triage");
        assert_eq!(h.top_namespace(), "inbox");
        assert_eq!(h.slug(), "triage");
        assert_eq!(h.segments(), vec!["inbox", "triage"]);
        assert_eq!(h.as_path_segment(), "inbox/triage");
    }

    #[test]
    fn parses_nested_channel_handle() {
        let h = QueueHandle::parse("channel:dommage-corporel:paris-cohort").unwrap();
        assert_eq!(h.top_namespace(), "channel");
        assert_eq!(h.slug(), "dommage-corporel:paris-cohort");
        assert_eq!(
            h.segments(),
            vec!["channel", "dommage-corporel", "paris-cohort"]
        );
        assert_eq!(
            h.as_path_segment(),
            "channel/dommage-corporel/paris-cohort"
        );
    }

    #[test]
    fn parses_meta_segment_with_leading_underscore() {
        // Substrate-private segments allowed (`_meta`, `_org`).
        let h = QueueHandle::parse("channel:secretariat:_meta").unwrap();
        assert_eq!(h.segments(), vec!["channel", "secretariat", "_meta"]);
    }

    #[test]
    fn accepts_freeform_namespaces() {
        // Free-form: principal-defined taxonomy, no recognized list.
        assert!(QueueHandle::parse("area:writing").is_ok());
        assert!(QueueHandle::parse("project:secretariat").is_ok());
        assert!(QueueHandle::parse("client:marcelo").is_ok());
        assert!(QueueHandle::parse("peer:christophe-marchand").is_ok());
    }

    #[test]
    fn allows_digits_and_hyphens_in_segments() {
        assert!(QueueHandle::parse("inbox:to-self").is_ok());
        assert!(QueueHandle::parse("inbox:slug-1").is_ok());
        assert!(QueueHandle::parse("inbox:abc123").is_ok());
        // Nested with digits in deeper segments.
        assert!(QueueHandle::parse("channel:cohort-2026:q2").is_ok());
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
    fn rejects_missing_colon() {
        assert!(matches!(
            QueueHandle::parse("inbox-triage"),
            Err(QueueHandleError::MissingSeparator)
        ));
    }

    #[test]
    fn rejects_empty_namespace() {
        assert!(matches!(
            QueueHandle::parse(":triage"),
            Err(QueueHandleError::EmptyNamespace)
        ));
    }

    #[test]
    fn rejects_empty_trailing_segment() {
        assert!(matches!(
            QueueHandle::parse("inbox:"),
            Err(QueueHandleError::EmptySegment)
        ));
    }

    #[test]
    fn rejects_empty_middle_segment() {
        assert!(matches!(
            QueueHandle::parse("channel::paris"),
            Err(QueueHandleError::EmptySegment)
        ));
    }

    #[test]
    fn rejects_segment_starting_with_digit_or_hyphen() {
        assert!(matches!(
            QueueHandle::parse("inbox:1triage"),
            Err(QueueHandleError::InvalidChars)
        ));
        assert!(matches!(
            QueueHandle::parse("inbox:-triage"),
            Err(QueueHandleError::InvalidChars)
        ));
    }

    #[test]
    fn rejects_uppercase_anywhere() {
        assert!(matches!(
            QueueHandle::parse("Inbox:triage"),
            Err(QueueHandleError::InvalidChars)
        ));
        assert!(matches!(
            QueueHandle::parse("inbox:Triage"),
            Err(QueueHandleError::InvalidChars)
        ));
        assert!(matches!(
            QueueHandle::parse("channel:dept:SubTeam"),
            Err(QueueHandleError::InvalidChars)
        ));
    }

    #[test]
    fn rejects_invalid_segment_chars() {
        assert!(matches!(
            QueueHandle::parse("inbox:to self"),
            Err(QueueHandleError::InvalidChars)
        ));
        assert!(matches!(
            QueueHandle::parse("inbox:foo.bar"),
            Err(QueueHandleError::InvalidChars)
        ));
    }

    #[test]
    fn rejects_overlong() {
        let s = format!("inbox:{}", "x".repeat(60));
        assert!(matches!(
            QueueHandle::parse(&s),
            Err(QueueHandleError::TooLong)
        ));
    }

    #[test]
    fn display_roundtrip() {
        // parse(s).to_string() == s for the existing fixtures + nested forms.
        for s in [
            "inbox:triage",
            "area:writing",
            "project:secretariat",
            "channel:dommage-corporel:paris-cohort",
            "channel:secretariat:_meta",
        ] {
            let h = QueueHandle::parse(s).unwrap();
            assert_eq!(h.to_string(), s);
        }
    }

    #[test]
    fn serde_roundtrip() {
        let h = QueueHandle::parse("inbox:triage").unwrap();
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, "\"inbox:triage\"");
        let back: QueueHandle = serde_json::from_str(&json).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn serde_roundtrip_nested() {
        let h = QueueHandle::parse("channel:dommage-corporel:paris-cohort").unwrap();
        let json = serde_json::to_string(&h).unwrap();
        let back: QueueHandle = serde_json::from_str(&json).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn serde_rejects_malformed_string() {
        let r: Result<QueueHandle, _> = serde_json::from_str("\"no-colon-here\"");
        assert!(r.is_err());
    }
}
