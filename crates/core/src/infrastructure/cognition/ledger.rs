//! Append-only ledger of contextification decisions.
//!
//! Every routing call (whether or not the file actually moved) writes
//! one JSONL line to `<queues>/.contextification.log`. The principal can
//! `tail` the file to audit what the cognition substrate decided + which
//! decisions were actually applied. This is the only outbound-cognition
//! audit channel — there is no telemetry.
//!
//! Append-only is load-bearing: a missing or shrunk file means evidence
//! of past decisions has been lost, which the threat model treats as a
//! red flag. The writer never rewrites or truncates.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// One row of the ledger. Single source of truth for what the
/// cognition substrate decided and whether it was applied. Field order
/// mirrors the field order in serialized JSONL so a human reading
/// `tail -f` sees src → original → suggested → confidence first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Path of the captured file as written by `capture_to_queue`.
    pub src_path: String,
    /// Queue handle the capture was originally filed in (always
    /// `inbox:triage` in v1; broader as the policy widens).
    pub original_queue: String,
    /// Queue the cognition substrate suggested, or `None` when the
    /// adapter abstained / errored / was unconfigured.
    pub suggested_queue: Option<String>,
    /// `[0.0, 1.0]`. None if no suggestion produced.
    pub confidence: Option<f32>,
    /// Substrate identifier (`claude-opus-4-7`, `local-llama3-8b`, ...).
    pub model: Option<String>,
    /// Bumped by adapters when their prompt changes. Lets retroactive
    /// review reason about old decisions against a known baseline.
    pub prompt_version: Option<String>,
    /// Adapter's free-text rationale. Not used by code; for humans.
    pub rationale: Option<String>,
    pub decided_at: DateTime<Utc>,
    /// True when the file was actually moved as a result of the
    /// suggestion. False when the adapter was unconfigured, the
    /// confidence was below threshold, or the suggestion matched the
    /// original queue.
    pub applied: bool,
    /// Final filesystem location post-move. `None` when `!applied`.
    pub final_path: Option<String>,
}

/// Append one entry to the contextification ledger. Creates the file +
/// parent directory on first call.
pub fn append_entry(ledger_path: &Path, entry: &LedgerEntry) -> Result<(), LedgerError> {
    if let Some(parent) = ledger_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| LedgerError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ledger_path)
        .map_err(|e| LedgerError::Io {
            path: ledger_path.to_path_buf(),
            source: e,
        })?;
    let line = serde_json::to_string(entry)?;
    writeln!(file, "{line}").map_err(|e| LedgerError::Io {
        path: ledger_path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// Read every entry. Returns parse errors lazily so a corrupt line
/// doesn't drop the rest of the ledger. Used by tests + (eventually)
/// the review walker's "what did the AI do" surface.
pub fn read_entries(ledger_path: &Path) -> Result<Vec<LedgerEntry>, LedgerError> {
    if !ledger_path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(ledger_path).map_err(|e| LedgerError::Io {
        path: ledger_path.to_path_buf(),
        source: e,
    })?;
    let mut out = Vec::new();
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        out.push(serde_json::from_str(line)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample(applied: bool) -> LedgerEntry {
        LedgerEntry {
            src_path: "/q/inbox/triage/2026.md".into(),
            original_queue: "inbox:triage".into(),
            suggested_queue: Some("inbox:pain".into()),
            confidence: Some(0.83),
            model: Some("test-stub".into()),
            prompt_version: Some("v1".into()),
            rationale: Some("complains about onboarding".into()),
            decided_at: Utc::now(),
            applied,
            final_path: applied.then_some("/q/inbox/pain/2026.md".into()),
        }
    }

    #[test]
    fn append_then_read_roundtrips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("queues/.contextification.log");
        append_entry(&path, &sample(true)).unwrap();
        append_entry(&path, &sample(false)).unwrap();
        let entries = read_entries(&path).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].applied);
        assert!(!entries[1].applied);
    }

    #[test]
    fn read_missing_file_is_empty_not_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("never-written.log");
        let entries = read_entries(&path).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn append_creates_parent_dir() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a/b/c/.contextification.log");
        append_entry(&path, &sample(true)).unwrap();
        assert!(path.exists());
    }
}
