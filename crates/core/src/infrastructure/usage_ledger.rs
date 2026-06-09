//! Local, sovereign usage accounting for scribe dispatches. Each `claude -p
//! --output-format json` run returns its own cost + token usage; we append it
//! to `~/.secretariat/usage.jsonl`. Nothing phones home (invariant #2) — this
//! exists so the principal can see *their own* spend.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use serde::Serialize;

/// One dispatch's footprint.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UsageRecord {
    /// Unix epoch seconds.
    pub at: u64,
    /// What spent it, e.g. `workflow:to-linear`.
    pub source: String,
    pub repo: String,
    pub doc: String,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Pull `(cost_usd, input_tokens, output_tokens)` from a `claude -p
/// --output-format json` result envelope. `None` if the payload isn't JSON;
/// missing fields default to 0 (older CLI / a substrate without cost data).
pub fn parse_cli_usage(json: &str) -> Option<(f64, u64, u64)> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let cost = v.get("total_cost_usd").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let usage = v.get("usage");
    let input = usage
        .and_then(|u| u.get("input_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let output = usage
        .and_then(|u| u.get("output_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    Some((cost, input, output))
}

/// Append one record as a JSON line. Creates the file if absent.
pub fn append(ledger_path: &Path, record: &UsageRecord) -> std::io::Result<()> {
    let line = serde_json::to_string(record)?;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ledger_path)?;
    writeln!(f, "{line}")
}

/// Wall-clock seconds since the epoch (composition-root helper; not domain).
pub fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parses_cost_and_tokens() {
        let json = r#"{"type":"result","result":"ok","total_cost_usd":0.0123,
            "usage":{"input_tokens":1500,"output_tokens":420}}"#;
        assert_eq!(parse_cli_usage(json), Some((0.0123, 1500, 420)));
    }

    #[test]
    fn missing_fields_default_to_zero() {
        assert_eq!(parse_cli_usage("{}"), Some((0.0, 0, 0)));
        assert_eq!(parse_cli_usage("not json"), None);
    }

    #[test]
    fn append_writes_a_jsonl_line() {
        let d = TempDir::new().unwrap();
        let ledger = d.path().join("usage.jsonl");
        let rec = UsageRecord {
            at: 1_700_000_000,
            source: "workflow:to-linear".into(),
            repo: "minerva".into(),
            doc: "docs/pain/x.md".into(),
            cost_usd: 0.01,
            input_tokens: 10,
            output_tokens: 5,
        };
        append(&ledger, &rec).unwrap();
        append(&ledger, &rec).unwrap();
        let body = std::fs::read_to_string(&ledger).unwrap();
        assert_eq!(body.lines().count(), 2);
        assert!(body.contains("\"source\":\"workflow:to-linear\""));
    }
}
