//! `ChannelContract` — per-principal consumption overrides for a channel.
//!
//! Per AGENTS.md rule #6: `<channel-dir>/contract.md` is **the
//! principal's private consumption contract for that channel** — how
//! *they* approach it. Cadence they poll at, minimum trust they
//! require before surfacing, depth/urgency filters, notify rules.
//! Private file on the subscriber's disk. Never sent on wire. Never
//! shared with other roster members.
//!
//! Distinct from **channel governance** (roster, "this channel only
//! accepts stamped envelopes" policy) — that lives in `.channelDef` or
//! a future signed `channelDef` envelope owned by the channel owner.
//! Don't conflate the two: governance is shared & public to the
//! roster; consumption is private & per-principal.
//!
//! Accumulate semantics ([[project-contracts-accumulate]]) still apply,
//! but **within a single principal's own chain** — their org-root
//! contract, ancestor channels, leaf — never across principals. Mirrors
//! Claude Code's `CLAUDE.md` walk: my chain, my settings, not yours.
//!
//! v0.3 slice 1a fields:
//! - `cadence_floor_minutes` — my poll-floor for this channel
//! - `min_trust` — receiver-side filter: surface only envelopes with
//!   this trust level or higher (`signed-only` → also `stamp-required`;
//!   `stamp-required` → stamped only).
//!
//! Additional consumption fields (`depth_filter`, `urgency_filter`,
//! `notify`) land when the routing daemon ships and demands them.
//! Governance fields (`roster`, `accepts_only`, `cadence_max`) don't
//! belong here at all — they extend `.channelDef`.

/// Trust level requirements applied as a receiver-side filter.
///
/// `SignedOnly` — surface any envelope whose author signature verifies
/// (ambient context allowed).
/// `StampRequired` — only surface envelopes carrying a principal Touch-ID
/// stamp (treat ambient signed-only traffic as background).
///
/// Merge rule when this principal's contracts accumulate up their own
/// chain: MAX-RESTRICTIVE. A stricter floor at any ancestor level
/// applies. Loosening at a leaf is intentionally not possible — the
/// principal's strictest preference along the chain wins.
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
    /// Merge rule: MAX (tightest floor wins along the principal's
    /// accumulate chain).
    pub cadence_floor_minutes: Option<u32>,
    /// Receiver-side filter: surface only envelopes meeting this trust
    /// level or higher. None = inherit. Merge rule: MAX-RESTRICTIVE.
    pub min_trust: Option<TrustGate>,
}

impl ChannelContract {
    /// Empty contract — contributes nothing to a merge. Equivalent to
    /// an empty-frontmatter `contract.md`.
    pub fn empty() -> Self {
        Self::default()
    }

    /// True if every field is at its no-contribution default.
    pub fn is_empty(&self) -> bool {
        self.cadence_floor_minutes.is_none() && self.min_trust.is_none()
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
        let c = ChannelContract {
            min_trust: Some(TrustGate::StampRequired),
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
