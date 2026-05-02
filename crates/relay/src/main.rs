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
#[command(name = "sec-relay", version, about = "Secretariat relay — federation node, not a central server.")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Start the relay server.
    Serve {
        /// `host:port` to bind. Default: `127.0.0.1:8443` (loopback only).
        #[arg(long, default_value = "127.0.0.1:8443")]
        bind: SocketAddr,

        /// Restrict registration to this comma-separated list of DIDs.
        /// Omit for open registration.
        #[arg(long)]
        allowlist: Option<String>,

        /// Days to keep queued envelopes before pruning. Default: 7.
        #[arg(long, default_value_t = 7)]
        queue_ttl_days: i64,
    },
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
        } => serve(bind, allowlist, queue_ttl_days).await,
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

async fn serve(bind: SocketAddr, allowlist: Option<String>, queue_ttl_days: i64) -> Result<()> {
    let registration = match allowlist {
        None => RegistrationPolicy::Open,
        Some(s) => parse_allowlist(&s)?,
    };

    let config = Config {
        bind,
        registration: registration.clone(),
        queue_ttl: QueueTtlDays(queue_ttl_days),
    };
    let state = AppState::new(config);

    spawn_prune_loop(state.clone(), queue_ttl_days);

    info!(addr = %bind, ?registration, "starting sec-relay");
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
