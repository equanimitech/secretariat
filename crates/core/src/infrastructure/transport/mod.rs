//! Transport adapters — pluggable concrete implementations of the wire
//! between two daemons.
//!
//! Per AGENTS.md invariant #4, transports are *adapters*, not authorities.
//! They see signed-and-encrypted bytes only. The body of every envelope
//! moved over a transport is sealed to the recipient (see
//! `crate::infrastructure::crypto::sealed`).
//!
//! v0 ships one adapter:
//! - [`relay::RelayClient`] — HTTP client for our self-hostable axum relay
//!
//! Future adapters: Iroh / libp2p direct peer (when both online), Slack
//! workspace (deliberate workplace tradeoff), iMessage (intra-Apple).

pub mod relay;

pub use relay::{
    ClaimedInviteWire, RelayClient, RelayClientError, RelayInbound, RelayState, RelayStateError,
};
