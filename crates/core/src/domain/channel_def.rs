//! `ChannelDef` — value object describing a channel's metadata.
//!
//! v0.3 stores this as a `.channelDef` JSON file alongside `envelopes/`
//! inside each channel directory. The signed-envelope variant (per
//! `tech.equanimi.secretariat.channelDef` lexicon) lands when relay
//! sync ships — this JSON is the v0 placeholder.

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
        }
    }
}
