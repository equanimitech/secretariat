//! Cadence policy — when does the daemon poll, when does it deliver?
//!
//! Pure decision functions. The daemon owns the actual sleep loop; this
//! module just answers "should I poll right now?" given the current time and
//! when we last polled.
//!
//! ## Anti-compulsion default
//!
//! v0 defaults to **hourly** polling, with a hard floor of 15 minutes.
//! Tighter cadences are not exposed: even self-discipline cannot fight a
//! substrate that delivers in 30 seconds. See `AGENTS.md` invariant
//! "Equanimity by default" and the milestone doc's "Anti-compulsion rituals"
//! section.
//!
//! ## Config
//!
//! `~/.secretariat/cadence.toml`:
//!
//! ```toml
//! poll_interval_minutes = 60   # default 60, min 15
//! ```
//!
//! Missing file is fine — the daemon uses [`CadenceConfig::default`].

use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_POLL_INTERVAL_MIN: i64 = 60;
const MIN_POLL_INTERVAL_MIN: i64 = 15;

#[derive(Debug, Error)]
pub enum CadenceConfigError {
    #[error("io error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed cadence.toml: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("poll_interval_minutes = {got} is below the minimum {MIN_POLL_INTERVAL_MIN}")]
    BelowMinimum { got: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CadenceConfig {
    /// Minutes between relay polls. Floored at 15.
    pub poll_interval_minutes: i64,
}

impl Default for CadenceConfig {
    fn default() -> Self {
        Self {
            poll_interval_minutes: DEFAULT_POLL_INTERVAL_MIN,
        }
    }
}

impl CadenceConfig {
    /// Load from disk. Missing file → defaults. Below-minimum → error.
    pub fn load_or_default(path: &Path) -> Result<Self, CadenceConfigError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path).map_err(|e| CadenceConfigError::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        let cfg: Self = toml::from_str(&raw)?;
        if cfg.poll_interval_minutes < MIN_POLL_INTERVAL_MIN {
            return Err(CadenceConfigError::BelowMinimum {
                got: cfg.poll_interval_minutes,
            });
        }
        Ok(cfg)
    }

    pub fn poll_interval(&self) -> Duration {
        Duration::minutes(self.poll_interval_minutes)
    }
}

/// What the daemon should do right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollDecision {
    /// Poll now and update `last_poll_at`.
    PollNow,
    /// Wait until at least this instant before polling again.
    WaitUntil(DateTime<Utc>),
}

/// Pure: given the cadence config, the current wall clock, and when we last
/// polled (or `None` for "never"), should we poll right now?
pub fn decide_poll(
    config: &CadenceConfig,
    now: DateTime<Utc>,
    last_poll_at: Option<DateTime<Utc>>,
) -> PollDecision {
    match last_poll_at {
        None => PollDecision::PollNow,
        Some(last) => {
            let next = last + config.poll_interval();
            if now >= next {
                PollDecision::PollNow
            } else {
                PollDecision::WaitUntil(next)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 2, h, m, 0).unwrap()
    }

    #[test]
    fn default_is_hourly() {
        assert_eq!(CadenceConfig::default().poll_interval_minutes, 60);
    }

    #[test]
    fn first_poll_fires_immediately() {
        let cfg = CadenceConfig::default();
        let d = decide_poll(&cfg, t(10, 0), None);
        assert_eq!(d, PollDecision::PollNow);
    }

    #[test]
    fn second_poll_waits_for_interval() {
        let cfg = CadenceConfig::default();
        let last = t(10, 0);
        // 30 minutes later: still under the hourly window.
        let d = decide_poll(&cfg, t(10, 30), Some(last));
        assert_eq!(d, PollDecision::WaitUntil(t(11, 0)));
    }

    #[test]
    fn poll_fires_at_or_after_interval() {
        let cfg = CadenceConfig::default();
        let last = t(10, 0);
        // exactly at interval boundary
        assert_eq!(decide_poll(&cfg, t(11, 0), Some(last)), PollDecision::PollNow);
        // and after
        assert_eq!(decide_poll(&cfg, t(11, 1), Some(last)), PollDecision::PollNow);
    }

    #[test]
    fn loads_default_when_file_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("cadence.toml");
        let cfg = CadenceConfig::load_or_default(&path).unwrap();
        assert_eq!(cfg, CadenceConfig::default());
    }

    #[test]
    fn loads_and_parses_user_config() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("cadence.toml");
        std::fs::write(&path, "poll_interval_minutes = 120\n").unwrap();
        let cfg = CadenceConfig::load_or_default(&path).unwrap();
        assert_eq!(cfg.poll_interval_minutes, 120);
    }

    #[test]
    fn rejects_below_minimum() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("cadence.toml");
        std::fs::write(&path, "poll_interval_minutes = 5\n").unwrap();
        let r = CadenceConfig::load_or_default(&path);
        assert!(matches!(r, Err(CadenceConfigError::BelowMinimum { got: 5 })));
    }
}
