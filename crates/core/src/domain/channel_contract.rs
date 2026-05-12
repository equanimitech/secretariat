//! `ChannelContract` — value object describing per-channel governance.
//!
//! Mirrors `tech.equanimi.secretariat.channelContract` lexicon fields.
//! Carried at any level of the channel tree (org-root, trunk, or leaf);
//! the accumulate resolver (future slice) walks org-root → leaf and
//! merges field-by-field per per-field merge rules.
//!
//! v0.3 slice 1a: value object + storage only. Accumulate resolver,
//! CLI/MCP set/get verbs, and signed-envelope variant land in later
//! slices per `docs/pitches/2026-05-12-channel-contracts-mcp.md`.
//!
//! Scalar fields are `Option<T>` — `None` means "contribute nothing to
//! the merge" (let ancestors decide). Vector fields are always present;
//! empty means "contribute nothing."

use crate::domain::Did;

/// Trust gate enforced by the receiving side before acting on an
/// envelope's payload.
///
/// `SignedOnly` — author DID signature verified; ambient context ok.
/// `StampRequired` — additionally requires a principal Touch-ID stamp.
///
/// Merge rule: MAX-RESTRICTIVE (`StampRequired` > `SignedOnly`). Children
/// inherit ancestor's stricter gate; loosening at a leaf is intentionally
/// not possible — sovereignty flows top-down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustGate {
    SignedOnly,
    StampRequired,
}

impl TrustGate {
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustGate::SignedOnly => "signed-only",
            TrustGate::StampRequired => "stamp-required",
        }
    }

    /// Parse the on-wire string. Returns `None` for unknown values.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "signed-only" => Some(TrustGate::SignedOnly),
            "stamp-required" => Some(TrustGate::StampRequired),
            _ => None,
        }
    }

    /// MAX-RESTRICTIVE merge: stamp-required dominates signed-only.
    pub fn max_restrictive(a: TrustGate, b: TrustGate) -> TrustGate {
        match (a, b) {
            (TrustGate::StampRequired, _) | (_, TrustGate::StampRequired) => {
                TrustGate::StampRequired
            }
            _ => TrustGate::SignedOnly,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChannelContract {
    /// Minimum minutes between polls at this level. None = inherit.
    /// Merge rule: MAX (tightest floor wins).
    pub cadence_floor_minutes: Option<u32>,
    /// Required trust level at this level. None = inherit.
    /// Merge rule: MAX-RESTRICTIVE.
    pub trust_gate: Option<TrustGate>,
    /// DIDs admitted to read/publish at this level. Always additive.
    /// Merge rule: UNION.
    pub roster: Vec<Did>,
    /// Preferred transports as opaque URIs (e.g. `relay:themia.pro`).
    /// Merge rule: UNION.
    pub preferred_transports: Vec<String>,
}

impl ChannelContract {
    /// Empty contract — contributes nothing to a merge. Equivalent to
    /// an empty-frontmatter `contract.md`.
    pub fn empty() -> Self {
        Self::default()
    }

    /// True if every field is at its no-contribution default. The stub
    /// `contract.md` auto-written on `create_channel` lands here.
    pub fn is_empty(&self) -> bool {
        self.cadence_floor_minutes.is_none()
            && self.trust_gate.is_none()
            && self.roster.is_empty()
            && self.preferred_transports.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_contract_is_empty() {
        assert!(ChannelContract::empty().is_empty());
    }

    #[test]
    fn any_field_set_makes_not_empty() {
        let c = ChannelContract {
            cadence_floor_minutes: Some(15),
            ..Default::default()
        };
        assert!(!c.is_empty());
    }

    #[test]
    fn trust_gate_parses_known_values() {
        assert_eq!(TrustGate::parse("signed-only"), Some(TrustGate::SignedOnly));
        assert_eq!(
            TrustGate::parse("stamp-required"),
            Some(TrustGate::StampRequired)
        );
        assert_eq!(TrustGate::parse("nonsense"), None);
    }

    #[test]
    fn trust_gate_max_restrictive_picks_stamp() {
        assert_eq!(
            TrustGate::max_restrictive(TrustGate::SignedOnly, TrustGate::StampRequired),
            TrustGate::StampRequired
        );
        assert_eq!(
            TrustGate::max_restrictive(TrustGate::SignedOnly, TrustGate::SignedOnly),
            TrustGate::SignedOnly
        );
    }
}
