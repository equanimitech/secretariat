//! Transport state — the persisted relay record other flows still read.
//!
//! Per AGENTS.md invariant #4, transports are *adapters*, not authorities.
//! The federation HTTP client (`RelayClient`) that pushed/polled sealed
//! envelopes over a self-hosted relay was removed in the git-native
//! teardown (cut A); what survives is [`relay::RelayState`], the on-disk
//! `~/.secretariat/relay-state.json` model the invite flow and the
//! Settings → Relay pane consult.
//!
//! Future adapters: Iroh / libp2p direct peer (when both online), Slack
//! workspace (deliberate workplace tradeoff), iMessage (intra-Apple).

pub mod relay;

pub use relay::{QueueCursor, RelayEntry, RelayState, RelayStateError};
