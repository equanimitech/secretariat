//! `QueueHandle` — value object naming a queue on its owner's machine.
//!
//! Every `Recipient` is a `(owner, handle)` pair (queues-as-primitive,
//! 2026-05-05). The handle picks which queue on the owner's disk; the
//! owner DID picks whose disk. `owner == self_did` keeps the envelope
//! local; otherwise it routes to the owner's relay.
//!
//! Grammar (v0.5, channel-only): `^<seg>(:<seg>)*$` where each `<seg>`
//! matches `[a-z_][a-z0-9_-]*`. Tree depth = colon depth.
//!
//! Single-segment handles are valid (v0.5+) — colons signal nesting,
//! they're not required.
//!
//! Examples:
//! - `triage` — single-segment handle, e.g. `_self/channels/triage/`
//! - `articles` — single-segment under any root
//! - `dommage-corporel:paris-cohort` — nested channel under an
//!   org's tree
//!
//! The `channel:` / `inbox:` / `area:` / `project:` prefixes from
//! v0.2 / v0.3 are gone — handles no longer carry namespace info.
//! The root (principal vs org) is carried by the `Recipient`, not
//! the handle (see the namespace-collapse pitch, 2026-05-17).

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

    /// First segment of the handle.
    ///
    /// LEGACY: pre-v0.5 callers used this to branch on `"channel"` vs
    /// flat-queue namespaces. v0.5+ the first segment carries no special
    /// meaning. Prefer `segments()` and the `Recipient` root.
    pub fn top_namespace(&self) -> &str {
        self.0.split(':').next().unwrap()
    }

    /// All segments in order.
    pub fn segments(&self) -> Vec<&str> {
        self.0.split(':').collect()
    }

    /// LEGACY alias for `top_namespace()`.
    pub fn namespace(&self) -> &str {
        self.top_namespace()
    }

    /// Everything after the first colon, or `""` for a single-segment handle.
    ///
    /// LEGACY: only meaningful when paired with `top_namespace()` for old
    /// flat-queue path layouts. v0.5+ callers should iterate `segments()`.
    pub fn slug(&self) -> &str {
        self.0.split_once(':').map(|(_, s)| s).unwrap_or("")
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
    fn accepts_single_segment() {
        // v0.5+: single-segment handles are valid (channel-only world, no
        // namespace prefix required). `triage`, `articles`, `journals` all
        // resolve as direct children of `<root>/channels/`.
        let h = QueueHandle::parse("triage").unwrap();
        assert_eq!(h.as_str(), "triage");
        assert_eq!(h.segments(), vec!["triage"]);
        assert_eq!(h.as_path_segment(), "triage");
        assert_eq!(h.slug(), ""); // legacy method: empty for single-segment
    }

    #[test]
    fn accepts_handle_without_separator() {
        // A bare identifier is a legal handle now — `inbox-triage` parses
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
        // Uppercase is structurally invalid under any grammar version.
        let r: Result<QueueHandle, _> = serde_json::from_str("\"NotAValidHandle\"");
        assert!(r.is_err());
    }
}
