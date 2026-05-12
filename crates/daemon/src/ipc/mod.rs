//! IPC — Unix-socket control plane for the running daemon (Slice 1).
//!
//! Why this exists: v0.2.16 had three callers of `sync_now` (the loop,
//! `sec daemon tick`, the Tauri app's "Sync now"). Each ran its own
//! process, which meant they could race against `RelayState` saves and
//! couldn't share warm caches. Slice 1 introduces a single socket the
//! daemon listens on; CLI and Tauri become *clients* of the running
//! daemon when it's available, falling back to in-proc execution when
//! it isn't. v0.2.16 behavior is preserved when the socket is absent.
//!
//! Protocol: line-delimited JSON-RPC 2.0 over `~/.secretariat/daemon.sock`.
//! See [`protocol`] for the request/response types and [`server`] +
//! [`client`] for the two endpoints. Method surface in v0.3:
//!
//! - `version` — returns daemon version string. No params.
//! - `ping`    — health probe; returns `{ok: true}`.
//! - `tick`    — run one sync cycle; returns the [`SyncOutcome`] inline.
//!
//! Future methods land here as new subsystems come online: `status`
//! returning structured health (relays, queued outbox count, last-poll
//! timestamp), `meta_resolve` / `digest` / `agent_run` once the
//! corresponding subsystems exist.
//!
//! Permissions: the socket is `chmod 0600` — same user only. Stale
//! sockets are removed before bind; if another daemon is already
//! listening, the new daemon logs and continues without IPC (the
//! existing one keeps the socket).
//!
//! [`SyncOutcome`]: secretariat_core::application::SyncOutcome

pub mod client;
pub mod protocol;
pub mod server;

pub use client::{call, is_running, tick_via_ipc_or_inproc};
pub use protocol::{Request, Response, RpcError, JSONRPC_VERSION};
pub use server::spawn_server;

use secretariat_core::infrastructure::keys::KeyPaths;
use std::path::PathBuf;

/// Canonical path to the daemon's IPC socket. Currently sits next to
/// other ephemeral state under the principal's `~/.secretariat/` root.
pub fn socket_path(paths: &KeyPaths) -> PathBuf {
    paths.root.join("daemon.sock")
}
