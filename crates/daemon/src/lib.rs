//! Secretariat daemon — the principal's local always-running process.
//!
//! # Today (post git-native teardown)
//!
//! The federation column — relay poll, outbox watcher, IPC tick path,
//! `sync_now` cycle — was removed in the git-native teardown (cut A).
//! Inbound/outbound correspondence over a hosted relay is no longer part
//! of the daemon's job. What survives is the macOS supervision surface:
//!
//! - [`install_launchagent`] / [`uninstall_launchagent`] / [`report_status`]
//!   — the LaunchAgent install/uninstall/status ceremony Tauri wires up
//!   silently on launch (`sec daemon install`).
//! - [`serve`] — a minimal keepalive the installed LaunchAgent plist
//!   targets (`sec daemon serve`). It brings nothing online today; it
//!   simply blocks until SIGTERM/SIGINT so the supervised process has a
//!   valid, well-behaved entry point. Real subsystems re-land here when
//!   the git-native substrate grows its own delivery path.
//! - [`init_tracing`] — shared subscriber bootstrap.
//!
//! # Layering
//!
//! - Domain / use cases stay in `secretariat-core` (no IO in domain).
//! - This crate is infrastructure-flavoured: it owns the LaunchAgent
//!   surface. It is *not* the place for new domain logic.
//! - The CLI (`secretariat-cli`) parses arguments and resolves the
//!   principal's `KeyPaths`, then hands them to this crate.

pub mod launchagent;
pub mod serve;
pub mod tracing_init;

pub use launchagent::{
    install_launchagent, report_status, uninstall_launchagent, LAUNCHAGENT_LABEL,
};
pub use serve::serve;
pub use tracing_init::init_tracing;
