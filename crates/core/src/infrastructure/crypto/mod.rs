//! Cryptographic primitives beyond ed25519 signing.
//!
//! The signing layer (ed25519) lives in `infrastructure::ed25519_signer`.
//! This module adds the *encryption* layer needed for envelope bodies that
//! traverse a transport (relay, future Iroh, etc.) — see invariant #4 in
//! `AGENTS.md`: transports see signed *and* encrypted bytes only.

pub mod sealed;

pub use sealed::{
    open, pubkey_to_x25519, seal, signing_to_x25519, OpenError, SealError, SealedBox,
    SealedBoxParseError,
};
