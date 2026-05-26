//! `ChannelDef` — value object describing a channel's metadata.
//!
//! v0.3 stores this as a `channel.md` markdown file (YAML frontmatter +
//! optional body prose) alongside `envelopes/` inside each channel
//! directory. The frontmatter mirrors the
//! `tech.equanimi.secretariat.channelDef` lexicon; the signed-envelope
//! variant lands when relay sync ships.

use chrono::{DateTime, Utc};

use super::QueueHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelDef {
    /// Canonical handle, e.g. `channel:product:data:baux-commerciaux`.
    pub handle: QueueHandle,
    /// Human-readable display name. Empty if unset.
    pub name: String,
    /// Free-form description / purpose. Empty if unset.
    pub description: String,
    pub created_at: DateTime<Utc>,
    /// Channel-governance policy: if `true`, receivers MUST treat
    /// unstamped envelopes on this channel as *ambient* (informational),
    /// not authoritative. Agents acting on received envelopes MUST NOT
    /// rely on signed-only traffic when this is set. Substrate-for-themia
    /// slice — see `docs/pitches/2026-05-21-substrate-for-themia.md`
    /// element §5. Receiver-side discipline; relay-side enforcement
    /// deferred per `[[role-tamper-proof]]`. Default `false` (ambient
    /// channels — most traffic).
    pub requires_stamp: bool,
    /// Tombstone marker. When `true`, this channelDef announces the
    /// channel's removal — receiving subscribers drop the channel from
    /// their sidebar (delete local `channel.md` manifest) but preserve
    /// any `envelopes/` history already on disk. Distinct from
    /// `retired_at` (soft retire — still readable + listed). Default
    /// `false`. Slice A' (live org membership).
    pub tombstoned: bool,
}

impl ChannelDef {
    pub fn new(
        handle: QueueHandle,
        name: impl Into<String>,
        description: impl Into<String>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            handle,
            name: name.into(),
            description: description.into(),
            created_at,
            requires_stamp: false,
            tombstoned: false,
        }
    }

    /// Builder-style: opt the channel into stamp-required governance.
    /// Used for channels carrying authoritative records (e.g.
    /// `assemblee_generale`, board decisions, contracts).
    pub fn with_requires_stamp(mut self, requires_stamp: bool) -> Self {
        self.requires_stamp = requires_stamp;
        self
    }

    /// Builder-style: mark this channelDef as a tombstone (channel removed).
    pub fn with_tombstoned(mut self, tombstoned: bool) -> Self {
        self.tombstoned = tombstoned;
        self
    }
}
