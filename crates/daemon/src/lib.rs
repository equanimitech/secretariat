//! Secretariat daemon — the principal's local always-running process.
//!
//! # Today (v0.2.x)
//!
//! Plumbing: poll registered relays, file inbound envelopes into
//! the per-queue `envelopes/` tree, drain claim notifications (auto-add bilateral
//! contacts), drain stamped self-authored envelopes pending send. Cadence is governed by
//! [`secretariat_core::application::decide_poll`] (15-min floor by default,
//! see `cadence.toml`).
//!
//! Three callers share one tick body via
//! [`secretariat_core::application::sync_now`]:
//! - this daemon's [`serve`] loop (LaunchAgent-supervised)
//! - the CLI's `sec daemon tick` one-shot ([`tick_once`])
//! - the Tauri app's "Sync now" affordance (called in-proc today; Slice 1
//!   will route it through the daemon's IPC socket)
//!
//! Decryption is **not** done here. Bodies stay ciphertext on disk; `sec
//! read` decrypts on demand. The signing key is held only as long as a
//! tick needs it. This was the v0.2.x security posture, kept until Slice 4
//! migrates to eager-decrypt (see
//! `docs/ideas/2026-05-12-daemon-evolution.md`).
//!
//! # Direction (v0.3+)
//!
//! This crate is the eventual home of nine subsystems —
//! `RelayServer` / `RelayClient` / `EnvelopeWatcher` / `InboxWriter` /
//! `MetaResolver` / `AgentSupervisor` / `RoutingEngine` /
//! `ScheduleTicker` / `IPC`. See the daemon-evolution doc for the
//! ship order. Slice 0 (this commit) extracts today's logic out of
//! the CLI without behavior change so subsequent slices have a place
//! to grow.
//!
//! # Layering
//!
//! - Domain / use cases stay in `secretariat-core` (no IO in domain;
//!   `application::sync_now` orchestrates the IO).
//! - This crate is infrastructure-flavoured: it owns the supervision
//!   loop, the LaunchAgent surface, and (soon) IPC / FS-notify /
//!   WebSocket. It is *not* the place for new domain logic.
//! - The CLI (`secretariat-cli`) parses arguments and resolves the
//!   principal's `KeyPaths` / `Did` / `SigningKey`, then hands them
//!   to this crate. Keeping path discovery in the CLI preserves the
//!   `SECRETARIAT_HOME` test override without leaking it into the
//!   daemon library.

pub mod ipc;
pub mod launchagent;
pub mod outbox_watcher;
pub mod relay_register;
pub mod serve;
pub mod tracing_init;

pub use ipc::{tick_via_ipc_or_inproc, Request as IpcRequest, Response as IpcResponse};
pub use launchagent::{install_launchagent, report_status, uninstall_launchagent, LAUNCHAGENT_LABEL};
pub use relay_register::register;
pub use serve::{run_tick, serve, tick_once};
pub use tracing_init::init_tracing;
