//! Tracing subscriber bootstrap. Called once at process start by every
//! daemon entry point (`serve`, `tick_once`, `register`). Idempotent —
//! subsequent calls are no-ops via `try_init`.
//!
//! Honors `RUST_LOG` for the principal's debugging convenience; defaults
//! to `info,sec=info` when unset.

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sec=info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        // Diagnostic output belongs on stderr; the LaunchAgent plist
        // already routes stderr to ~/.secretariat/logs/daemon.stderr.log.
        // Keeping tracing off stdout also leaves stdout free for any
        // future JSON-on-stdout subcommand without interleaving.
        .with(fmt::layer().with_writer(std::io::stderr).compact())
        .try_init();
}
