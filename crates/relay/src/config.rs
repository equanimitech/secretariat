//! Server configuration. CLI flags map to fields here.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;

use secretariat_core::Did;

/// How the relay restricts who may register.
#[derive(Debug, Clone, Default)]
pub enum RegistrationPolicy {
    /// Anyone with a valid signed registration may register. Default for v0.
    #[default]
    Open,
    /// Only DIDs in the allowlist may register.
    Allowlist(HashSet<Did>),
}

/// TTL for queued envelopes (default: 7 days). Older entries are pruned by
/// the background sweep loop.
#[derive(Debug, Clone, Copy)]
pub struct QueueTtlDays(pub i64);

impl Default for QueueTtlDays {
    fn default() -> Self {
        QueueTtlDays(7)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub registration: RegistrationPolicy,
    pub queue_ttl: QueueTtlDays,
    /// Directory holding `state.json`. When `None`, the relay runs purely
    /// in-memory (acceptable for tests and ephemeral local dev). Railway
    /// deploys mount a volume at `/data` and pass it in.
    pub data_dir: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8443".parse().unwrap(),
            registration: RegistrationPolicy::Open,
            queue_ttl: QueueTtlDays::default(),
            data_dir: None,
        }
    }
}
