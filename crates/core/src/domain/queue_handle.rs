//! `QueueHandle` — value object addressing a *local* queue.
//!
//! Local queues are the destination side of `Recipient::LocalQueue`. Where
//! `Recipient::Peer(Did)` addresses another principal (H↔H, requires stamp
//! eventually), `Recipient::LocalQueue(QueueHandle)` addresses a local
//! collection inside the principal's own state — no transport, no stamp.
//!
//! Format: `^[a-z]+:[a-z0-9-]+$` — a lowercase namespace prefix, a colon,
//! then a slug. Namespaces are **free-form** (principal-defined). Common
//! conventions: `inbox:triage`, `area:writing`, `project:secretariat`,
//! `client:marcelo`. The parser validates the *shape* but does not gate on
//! a recognized list — that aligns with equanimitech "holistic control"
//! (principal owns their own taxonomy).
//!
//! See `docs/pitches/2026-05-05-event-sourced-envelope-substrate.md` and
//! `docs/milestones/2026-05-05-substrate-and-menubar.md`.

use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QueueHandleError {
    #[error("queue handle is empty")]
    Empty,
    #[error("queue handle missing `:` separator (expected `<namespace>:<slug>`)")]
    MissingSeparator,
    #[error("queue handle has empty namespace before `:`")]
    EmptyNamespace,
    #[error("queue handle has empty slug after `:`")]
    EmptySlug,
    #[error("queue handle contains invalid characters (must match `^[a-z]+:[a-z0-9-]+$`)")]
    InvalidChars,
    #[error("queue handle exceeds 64-byte length")]
    TooLong,
}

/// Maximum total length of a handle, including namespace + colon + slug.
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

        let (namespace, slug) = s
            .split_once(':')
            .ok_or(QueueHandleError::MissingSeparator)?;

        if namespace.is_empty() {
            return Err(QueueHandleError::EmptyNamespace);
        }
        if slug.is_empty() {
            return Err(QueueHandleError::EmptySlug);
        }

        // Namespace must match `[a-z]+` (lowercase letters only).
        if !namespace.chars().all(|c| c.is_ascii_lowercase()) {
            return Err(QueueHandleError::InvalidChars);
        }
        // Slug must match `[a-z0-9-]+`.
        if !slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(QueueHandleError::InvalidChars);
        }

        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `inbox` for `inbox:triage`. The namespace component before the
    /// `:`. Free-form (validated for shape only, not against a fixed
    /// list).
    pub fn namespace(&self) -> &str {
        // Validated at construction — split_once always Some.
        self.0.split_once(':').map(|(n, _)| n).unwrap()
    }

    /// `triage` for `inbox:triage`.
    pub fn slug(&self) -> &str {
        self.0.split_once(':').map(|(_, s)| s).unwrap()
    }

    /// Path-safe form: `inbox:triage` → `inbox/triage`. Used by the
    /// filesystem layout to mirror `outbox/<recipient-did>/` shape.
    pub fn as_path_segment(&self) -> String {
        self.0.replace(':', "/")
    }
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
        assert_eq!(h.namespace(), "inbox");
        assert_eq!(h.slug(), "triage");
        assert_eq!(h.as_path_segment(), "inbox/triage");
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
    fn allows_digits_and_hyphens_in_slug() {
        assert!(QueueHandle::parse("inbox:to-self").is_ok());
        assert!(QueueHandle::parse("inbox:slug-1").is_ok());
        assert!(QueueHandle::parse("inbox:abc123").is_ok());
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
    fn rejects_empty_slug() {
        assert!(matches!(
            QueueHandle::parse("inbox:"),
            Err(QueueHandleError::EmptySlug)
        ));
    }

    #[test]
    fn rejects_invalid_namespace_chars() {
        // Namespace must be lowercase letters only.
        assert!(matches!(
            QueueHandle::parse("Inbox:triage"),
            Err(QueueHandleError::InvalidChars)
        ));
        assert!(matches!(
            QueueHandle::parse("inbox-area:triage"),
            Err(QueueHandleError::InvalidChars)
        ));
    }

    #[test]
    fn rejects_invalid_slug_chars() {
        // Slug must be lowercase letters / digits / hyphens.
        assert!(matches!(
            QueueHandle::parse("inbox:Triage"),
            Err(QueueHandleError::InvalidChars)
        ));
        assert!(matches!(
            QueueHandle::parse("inbox:to self"),
            Err(QueueHandleError::InvalidChars)
        ));
        assert!(matches!(
            QueueHandle::parse("inbox:foo_bar"),
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
    fn serde_roundtrip() {
        let h = QueueHandle::parse("inbox:triage").unwrap();
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, "\"inbox:triage\"");
        let back: QueueHandle = serde_json::from_str(&json).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn serde_rejects_malformed_string() {
        // Now: shape-only validation. Free-form namespaces accepted.
        let r: Result<QueueHandle, _> = serde_json::from_str("\"no-colon-here\"");
        assert!(r.is_err());
    }
}
