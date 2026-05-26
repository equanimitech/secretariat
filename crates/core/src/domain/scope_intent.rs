//! `ScopeIntent` — grant-shape for an org invite.
//!
//! Carried on the wire as part of the invite record (signed-create v2,
//! 2026-05-26 Slice A' / live org membership). The value declares HOW the
//! grant should be interpreted by the receiver, not WHICH channels exist:
//!
//! - **`Org`** — live participant. Receiver subscribes to every current
//!   channel and every future channel announced via `channelDef` envelopes
//!   on the org's `<alias>:_meta` queue. The Slack-replacement shape.
//! - **`Subtree(handle)`** — receiver subscribes to that handle plus any
//!   descendant handles announced subsequently. Useful for granting a
//!   team-leaf without org-wide access.
//! - **`Channels`** — explicit enumeration. Uses the invite's
//!   `channel_handles` list; no future channels added.
//!
//! Wire form is a compact lowercase string for canonical signing:
//! - `Org` → `"org"`
//! - `Subtree(h)` → `"subtree:<handle>"`
//! - `Channels` → `"channels"`
//! - legacy invites (no scope_intent) → `""`, treated as `Channels` for
//!   backward compat with v1 wire records.

use thiserror::Error;

use super::{QueueHandle, QueueHandleError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeIntent {
    Org,
    Subtree(QueueHandle),
    Channels,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScopeIntentParseError {
    #[error("unknown scope_intent variant `{0}`")]
    UnknownVariant(String),
    #[error("subtree handle missing after `subtree:`")]
    MissingSubtreeHandle,
    #[error("invalid subtree handle `{handle}`: {source}")]
    InvalidSubtreeHandle {
        handle: String,
        #[source]
        source: QueueHandleError,
    },
}

impl ScopeIntent {
    /// Canonical wire string. The empty string is reserved for legacy v1
    /// invites and is NOT produced by this method.
    pub fn to_wire_string(&self) -> String {
        match self {
            ScopeIntent::Org => "org".to_string(),
            ScopeIntent::Subtree(h) => format!("subtree:{}", h.as_str()),
            ScopeIntent::Channels => "channels".to_string(),
        }
    }

    /// Parse a wire string. Empty input maps to `Channels` (legacy
    /// compat — v1 invites carried no scope_intent field).
    pub fn parse_wire_string(s: &str) -> Result<Self, ScopeIntentParseError> {
        if s.is_empty() || s == "channels" {
            return Ok(ScopeIntent::Channels);
        }
        if s == "org" {
            return Ok(ScopeIntent::Org);
        }
        if let Some(handle_str) = s.strip_prefix("subtree:") {
            if handle_str.is_empty() {
                return Err(ScopeIntentParseError::MissingSubtreeHandle);
            }
            let handle = QueueHandle::parse(handle_str).map_err(|e| {
                ScopeIntentParseError::InvalidSubtreeHandle {
                    handle: handle_str.to_string(),
                    source: e,
                }
            })?;
            return Ok(ScopeIntent::Subtree(handle));
        }
        Err(ScopeIntentParseError::UnknownVariant(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_org() {
        let s = ScopeIntent::Org;
        let wire = s.to_wire_string();
        assert_eq!(wire, "org");
        let back = ScopeIntent::parse_wire_string(&wire).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn roundtrips_subtree() {
        let h = QueueHandle::parse("project:secretariat").unwrap();
        let s = ScopeIntent::Subtree(h);
        let wire = s.to_wire_string();
        assert_eq!(wire, "subtree:project:secretariat");
        let back = ScopeIntent::parse_wire_string(&wire).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn roundtrips_channels() {
        let s = ScopeIntent::Channels;
        assert_eq!(s.to_wire_string(), "channels");
        assert_eq!(
            ScopeIntent::parse_wire_string("channels").unwrap(),
            ScopeIntent::Channels
        );
    }

    #[test]
    fn empty_string_maps_to_channels_for_v1_compat() {
        let back = ScopeIntent::parse_wire_string("").unwrap();
        assert_eq!(back, ScopeIntent::Channels);
    }

    #[test]
    fn unknown_variant_rejected() {
        assert!(matches!(
            ScopeIntent::parse_wire_string("everything"),
            Err(ScopeIntentParseError::UnknownVariant(_))
        ));
    }

    #[test]
    fn subtree_missing_handle_rejected() {
        assert!(matches!(
            ScopeIntent::parse_wire_string("subtree:"),
            Err(ScopeIntentParseError::MissingSubtreeHandle)
        ));
    }

    #[test]
    fn subtree_bad_handle_rejected() {
        assert!(matches!(
            ScopeIntent::parse_wire_string("subtree:NotKebab"),
            Err(ScopeIntentParseError::InvalidSubtreeHandle { .. })
        ));
    }
}
