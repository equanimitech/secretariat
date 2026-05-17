//! `Root` — the queue-root that owns a queue on disk.
//!
//! Per the v0.5 namespace-collapse pitch
//! (`docs/pitches/2026-05-17-collapse-namespaces.md`), every channel-bearing
//! directory lives under one of two roots:
//!
//! - `Root::Self_` → `<vault>/_self/channels/<segments>/`
//! - `Root::Org(alias)` → `<vault>/orgs/<alias>/channels/<segments>/`
//!
//! `Root` is *which* root, not *where* — the vault path comes in separately
//! at resolve time. Keeping the two factored means tests can swap vaults
//! without rewriting recipient state, and the same value object describes
//! both writer (capture) and reader (channels listing) sides.

use super::OrgAlias;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Root {
    Self_,
    Org(OrgAlias),
}

impl Root {
    /// Self root — principal's own queue-root.
    pub fn self_() -> Self {
        Self::Self_
    }

    /// Org root — queue-root for a subscribed org by alias.
    pub fn org(alias: OrgAlias) -> Self {
        Self::Org(alias)
    }

    /// True when this is the principal's own root.
    pub fn is_self(&self) -> bool {
        matches!(self, Self::Self_)
    }
}
