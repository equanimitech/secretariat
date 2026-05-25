//! `sec-relay` — Secretariat relay binary.
//!
//! ```text
//! sec-relay serve --bind 0.0.0.0:8443
//! sec-relay serve --bind 0.0.0.0:8443 --allowlist did:web:rafa.equanimi.tech,did:key:z...
//! ```

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use secretariat_core::Did;
use secretariat_relay::config::{QueueTtlDays, RegistrationPolicy};
use secretariat_relay::queue::ttl_cutoff;
use secretariat_relay::{router, AppState, Config};
use tokio::net::TcpListener;
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser, Debug)]
#[command(
    name = "sec-relay",
    version,
    about = "Secretariat relay — federation node, not a central server."
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Start the relay server.
    ///
    /// Default bind: `0.0.0.0:$PORT` if `PORT` is set (Railway, Render, Fly,
    /// etc. all set it), else `0.0.0.0:8443`. Override with `--bind`.
    Serve {
        /// `host:port` to bind. Overrides the `PORT` env var.
        #[arg(long)]
        bind: Option<SocketAddr>,

        /// Restrict registration to this comma-separated list of DIDs.
        /// Omit for open registration.
        #[arg(long)]
        allowlist: Option<String>,

        /// Days to keep queued envelopes before pruning. Default: 7.
        #[arg(long, default_value_t = 7)]
        queue_ttl_days: i64,

        /// Directory holding `state.json` (registry, queues, invites).
        /// On Railway, mount a persistent volume here. Falls back to the
        /// `DATA_DIR` env var; if neither is set, the relay runs purely
        /// in-memory (state lost on restart — fine for tests, not prod).
        #[arg(long)]
        data_dir: Option<std::path::PathBuf>,
    },
}

/// Resolve the bind address. Order: explicit flag → `PORT` env var → 8443.
fn resolve_bind(explicit: Option<SocketAddr>) -> Result<SocketAddr> {
    if let Some(addr) = explicit {
        return Ok(addr);
    }
    if let Ok(port) = std::env::var("PORT") {
        let addr = format!("0.0.0.0:{port}");
        return addr
            .parse::<SocketAddr>()
            .with_context(|| format!("invalid PORT env var value `{port}`"));
    }
    Ok("0.0.0.0:8443".parse().unwrap())
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Serve {
            bind,
            allowlist,
            queue_ttl_days,
            data_dir,
        } => {
            let bind = resolve_bind(bind)?;
            let data_dir = data_dir.or_else(|| std::env::var("DATA_DIR").ok().map(Into::into));
            serve(bind, allowlist, queue_ttl_days, data_dir).await
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info,sec_relay=debug"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().compact())
        .init();
}

async fn serve(
    bind: SocketAddr,
    allowlist: Option<String>,
    queue_ttl_days: i64,
    data_dir: Option<std::path::PathBuf>,
) -> Result<()> {
    let registration = match allowlist {
        None => RegistrationPolicy::Open,
        Some(s) => parse_allowlist(&s)?,
    };

    let config = Config {
        bind,
        registration: registration.clone(),
        queue_ttl: QueueTtlDays(queue_ttl_days),
        data_dir: data_dir.clone(),
    };

    let state = AppState::load(config).context("loading relay state from disk")?;
    spawn_prune_loop(state.clone(), queue_ttl_days);

    info!(
        addr = %bind,
        ?registration,
        data_dir = ?data_dir,
        "starting sec-relay"
    );
    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("binding {bind}"))?;
    let app = router(state);
    axum::serve(listener, app).await?;
    Ok(())
}

fn parse_allowlist(raw: &str) -> Result<RegistrationPolicy> {
    let mut set = HashSet::new();
    for piece in raw.split(',') {
        let trimmed = piece.trim();
        if trimmed.is_empty() {
            continue;
        }
        let did = Did::parse(trimmed)
            .with_context(|| format!("invalid did in --allowlist: `{trimmed}`"))?;
        set.insert(did);
    }
    if set.is_empty() {
        warn!("--allowlist supplied but parsed to zero DIDs; treating as open registration");
        return Ok(RegistrationPolicy::Open);
    }
    Ok(RegistrationPolicy::Allowlist(set))
}

/// Background task that prunes expired entries from every tenant's queue.
/// Runs once an hour. v0 acceptable; finer cadence is not worth the wakeups.
fn spawn_prune_loop(state: Arc<AppState>, ttl_days: i64) {
    tokio::spawn(async move {
        let mut tick = interval(TokioDuration::from_secs(60 * 60));
        tick.tick().await; // skip the immediate first tick
        loop {
            tick.tick().await;
            let cutoff = ttl_cutoff(chrono::Utc::now(), ttl_days);
            let pruned = state.prune_all(cutoff);
            if pruned > 0 {
                info!(pruned, ttl_days, "pruned expired envelopes");
            }
        }
    });
}
