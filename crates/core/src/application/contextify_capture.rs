//! Use case: ask a `CognitionPort` where this capture belongs and, if
//! the suggestion is confident enough, re-file it.
//!
//! Background pass invoked after `capture_to_queue` writes a capture to
//! `inbox:triage`. If the principal has wired a cognition adapter (by
//! configuring `[cognition]` in `~/.secretariat/preferences.toml`), this
//! routine asks the adapter for a queue suggestion using only the body. When the
//! suggestion exceeds the configured threshold, the file moves; the
//! envelope's `recipient.handle` is rewritten to match the new
//! location. Every decision (moved or not) appends one line to the
//! contextification ledger.
//!
//! Invariants enforced here, not at the adapter:
//!
//! - Only fires for `inbox:triage` captures. Explicit-queue captures
//!   (`area:health`, `project:autonomous-enterprise`, …) bypass.
//! - Never moves a stamped file. Captures cannot be stamped by the
//!   domain invariant, but defense-in-depth checks the parsed stamp
//!   anyway.
//! - Never moves outside `queues_root`. The new queue handle is parsed
//!   through `QueueHandle::parse`, which forbids path traversal by
//!   construction; the resulting target dir is rooted under
//!   `queues_root`.
//! - Never crashes a capture. Adapter errors, IO errors, or malformed
//!   parses surface as `ContextifyError` so the caller can ledger +
//!   continue.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::domain::{Envelope, QueueHandle, Recipient};
use crate::infrastructure::cognition::{
    append_entry, AnyCognitionAdapter, LedgerEntry, LedgerError,
};
use crate::infrastructure::preferences::CognitionPrefs;
use crate::infrastructure::markdown::{embed_stamp, parse_document, MarkdownError};
use crate::ports::{CognitionError, CognitionPort, RouteSuggestion};

/// Whose queue handle is the wedge — only captures filed here are
/// candidates for re-routing. Explicit-queue captures (filed by an MCP
/// caller who already knew where it belonged) are out of scope.
pub const ROUTABLE_QUEUE: &str = "inbox:triage";

#[derive(Debug, Error)]
pub enum ContextifyError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("markdown: {0}")]
    Markdown(#[from] MarkdownError),
    #[error("ledger: {0}")]
    Ledger(#[from] LedgerError),
    #[error("capture has no envelope frontmatter — refusing to contextify")]
    NoEnvelope,
    #[error("capture is already stamped — refusing to contextify")]
    AlreadyStamped,
    #[error("queue discovery failed at {path}: {source}")]
    QueueDiscovery {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Outcome of a single contextification attempt. Always returned as
/// `Ok` from `contextify_capture` unless the call hit an IO/parse error
/// (those are exceptional). Adapter failures (NotConfigured, Network,
/// Abstained) are normal and produce an `applied: false` outcome.
#[derive(Debug, Clone)]
pub struct ContextifyOutcome {
    /// Where the capture file lives now. Equal to the input path when
    /// `!applied`, otherwise the new location.
    pub final_path: PathBuf,
    pub applied: bool,
    pub suggestion: Option<RouteSuggestion>,
    /// Reason the call did not move the file, when `!applied`. `None`
    /// when `applied`.
    pub skip_reason: Option<ContextifySkipReason>,
}

#[derive(Debug, Clone)]
pub enum ContextifySkipReason {
    /// Capture was not in `inbox:triage`. Filed explicitly; bypass.
    NotRoutable { current_queue: String },
    AdapterNotConfigured,
    AdapterAbstained,
    AdapterError(String),
    BelowThreshold { confidence: f32, threshold: f32 },
    SameQueueAsCurrent,
}

/// Run one contextification pass on a single capture file. Always
/// writes a ledger row (success or skip) so the principal can audit.
pub async fn contextify_capture<P: CognitionPort>(
    capture_path: &Path,
    queues_root: &Path,
    ledger_path: &Path,
    port: &P,
    threshold: f32,
    now: DateTime<Utc>,
) -> Result<ContextifyOutcome, ContextifyError> {
    // 1. Parse capture.
    let raw = std::fs::read_to_string(capture_path).map_err(|e| ContextifyError::Io {
        path: capture_path.to_path_buf(),
        source: e,
    })?;
    let parsed = parse_document(&raw)?;
    let envelope = parsed.envelope.clone().ok_or(ContextifyError::NoEnvelope)?;
    if parsed.stamp.is_some() {
        return Err(ContextifyError::AlreadyStamped);
    }

    let original_queue = envelope.recipient.handle.clone();
    let original_queue_str = original_queue.as_str().to_string();

    // 2. Eligibility gate. Explicit-queue captures bypass entirely.
    if original_queue.as_str() != ROUTABLE_QUEUE {
        let entry = LedgerEntry {
            src_path: capture_path.display().to_string(),
            original_queue: original_queue_str.clone(),
            suggested_queue: None,
            confidence: None,
            model: None,
            prompt_version: None,
            rationale: None,
            decided_at: now,
            applied: false,
            final_path: None,
        };
        append_entry(ledger_path, &entry)?;
        return Ok(ContextifyOutcome {
            final_path: capture_path.to_path_buf(),
            applied: false,
            suggestion: None,
            skip_reason: Some(ContextifySkipReason::NotRoutable {
                current_queue: original_queue_str,
            }),
        });
    }

    // 3. Discover existing queues so the adapter can constrain its
    // suggestion to vocabulary the principal already uses.
    let existing = discover_queues(queues_root)?;

    // 4. Ask the cognition substrate.
    let route_result = port.route_capture(&parsed.body, &existing).await;

    // 5. Branch on the answer.
    match route_result {
        Ok(suggestion) => {
            if suggestion.confidence < threshold {
                let entry = ledger_skip(
                    capture_path,
                    &original_queue,
                    Some(&suggestion),
                    now,
                );
                append_entry(ledger_path, &entry)?;
                return Ok(ContextifyOutcome {
                    final_path: capture_path.to_path_buf(),
                    applied: false,
                    skip_reason: Some(ContextifySkipReason::BelowThreshold {
                        confidence: suggestion.confidence,
                        threshold,
                    }),
                    suggestion: Some(suggestion),
                });
            }
            if suggestion.queue == original_queue {
                let entry = ledger_skip(
                    capture_path,
                    &original_queue,
                    Some(&suggestion),
                    now,
                );
                append_entry(ledger_path, &entry)?;
                return Ok(ContextifyOutcome {
                    final_path: capture_path.to_path_buf(),
                    applied: false,
                    skip_reason: Some(ContextifySkipReason::SameQueueAsCurrent),
                    suggestion: Some(suggestion),
                });
            }

            // Apply the move.
            let final_path =
                relocate(capture_path, queues_root, &suggestion.queue, &envelope, &parsed.body)?;
            let entry = LedgerEntry {
                src_path: capture_path.display().to_string(),
                original_queue: original_queue_str,
                suggested_queue: Some(suggestion.queue.as_str().to_string()),
                confidence: Some(suggestion.confidence),
                model: Some(suggestion.model.clone()),
                prompt_version: Some(suggestion.prompt_version.clone()),
                rationale: Some(suggestion.rationale.clone()),
                decided_at: now,
                applied: true,
                final_path: Some(final_path.display().to_string()),
            };
            append_entry(ledger_path, &entry)?;
            Ok(ContextifyOutcome {
                final_path,
                applied: true,
                suggestion: Some(suggestion),
                skip_reason: None,
            })
        }
        Err(err) => {
            let skip = match &err {
                CognitionError::NotConfigured => ContextifySkipReason::AdapterNotConfigured,
                CognitionError::Abstained => ContextifySkipReason::AdapterAbstained,
                other => ContextifySkipReason::AdapterError(other.to_string()),
            };
            let entry = LedgerEntry {
                src_path: capture_path.display().to_string(),
                original_queue: original_queue_str,
                suggested_queue: None,
                confidence: None,
                model: None,
                prompt_version: None,
                rationale: Some(err.to_string()),
                decided_at: now,
                applied: false,
                final_path: None,
            };
            append_entry(ledger_path, &entry)?;
            Ok(ContextifyOutcome {
                final_path: capture_path.to_path_buf(),
                applied: false,
                suggestion: None,
                skip_reason: Some(skip),
            })
        }
    }
}

/// Best-effort post-capture hook callers wire after `capture_to_queue`.
/// Loads the Claude adapter (if configured) and runs the contextify
/// pass. The capture call has already returned success; this routine
/// must never propagate an error back to the principal — failures are
/// observable only through (a) the file not being moved and (b) the
/// ledger row written explaining why.
///
/// Returns `Ok(None)` when no adapter is configured (the default
/// state). Returns `Ok(Some(outcome))` when the pass ran. Returns
/// `Err` only on hard IO/parse errors that prevent even writing a
/// ledger row — caller can log and discard.
pub async fn try_contextify_after_capture(
    capture_path: &Path,
    queues_root: &Path,
    ledger_path: &Path,
    cognition_prefs: &CognitionPrefs,
    now: DateTime<Utc>,
) -> Result<Option<ContextifyOutcome>, ContextifyError> {
    let Some(adapter) = AnyCognitionAdapter::from_prefs(cognition_prefs) else {
        // No adapter configured = feature off.
        return Ok(None);
    };
    let threshold = adapter.threshold();
    let outcome =
        contextify_capture(capture_path, queues_root, ledger_path, &adapter, threshold, now)
            .await?;
    Ok(Some(outcome))
}

/// One level deep into `<queues_root>` for namespaces, then one more
/// level for slugs. Yields every `<ns>:<slug>` pair the principal has
/// already used. Quiet on missing root (returns empty).
fn discover_queues(queues_root: &Path) -> Result<Vec<QueueHandle>, ContextifyError> {
    let mut out = Vec::new();
    if !queues_root.exists() {
        return Ok(out);
    }
    let ns_iter = std::fs::read_dir(queues_root).map_err(|e| ContextifyError::QueueDiscovery {
        path: queues_root.to_path_buf(),
        source: e,
    })?;
    for ns_entry in ns_iter {
        let ns_entry = ns_entry.map_err(|e| ContextifyError::QueueDiscovery {
            path: queues_root.to_path_buf(),
            source: e,
        })?;
        let ns_path = ns_entry.path();
        if !ns_path.is_dir() {
            continue;
        }
        let Some(ns) = ns_path.file_name().and_then(|x| x.to_str()) else {
            continue;
        };
        // Skip dotfiles like `.contextification.log` if it ever lands here.
        if ns.starts_with('.') {
            continue;
        }
        let slug_iter = std::fs::read_dir(&ns_path).map_err(|e| ContextifyError::QueueDiscovery {
            path: ns_path.clone(),
            source: e,
        })?;
        for slug_entry in slug_iter {
            let slug_entry = slug_entry.map_err(|e| ContextifyError::QueueDiscovery {
                path: ns_path.clone(),
                source: e,
            })?;
            let slug_path = slug_entry.path();
            if !slug_path.is_dir() {
                continue;
            }
            let Some(slug) = slug_path.file_name().and_then(|x| x.to_str()) else {
                continue;
            };
            if let Ok(handle) = QueueHandle::parse(&format!("{ns}:{slug}")) {
                out.push(handle);
            }
        }
    }
    Ok(out)
}

fn ledger_skip(
    capture_path: &Path,
    original: &QueueHandle,
    suggestion: Option<&RouteSuggestion>,
    now: DateTime<Utc>,
) -> LedgerEntry {
    LedgerEntry {
        src_path: capture_path.display().to_string(),
        original_queue: original.as_str().to_string(),
        suggested_queue: suggestion.map(|s| s.queue.as_str().to_string()),
        confidence: suggestion.map(|s| s.confidence),
        model: suggestion.map(|s| s.model.clone()),
        prompt_version: suggestion.map(|s| s.prompt_version.clone()),
        rationale: suggestion.map(|s| s.rationale.clone()),
        decided_at: now,
        applied: false,
        final_path: None,
    }
}

/// Move the capture into the new queue and rewrite the envelope's
/// recipient handle to match. The `from` DID is preserved (captures are
/// always self-addressed) — only the queue handle changes. Filename is
/// preserved so the ledger's `src_path → final_path` link is obvious.
fn relocate(
    capture_path: &Path,
    queues_root: &Path,
    new_queue: &QueueHandle,
    envelope: &Envelope,
    body: &str,
) -> Result<PathBuf, ContextifyError> {
    let new_dir = queues_root.join(new_queue.namespace()).join(new_queue.slug());
    std::fs::create_dir_all(&new_dir).map_err(|e| ContextifyError::Io {
        path: new_dir.clone(),
        source: e,
    })?;
    let filename = capture_path
        .file_name()
        .ok_or_else(|| ContextifyError::Io {
            path: capture_path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "capture path has no filename",
            ),
        })?;
    let final_path = new_dir.join(filename);

    // Rewrite envelope to point at the new queue.
    let mut updated_envelope = envelope.clone();
    updated_envelope.recipient = Recipient::new(envelope.from.clone(), new_queue.clone());
    let new_content = embed_stamp(body, Some(&updated_envelope), None)?;
    std::fs::write(&final_path, new_content).map_err(|e| ContextifyError::Io {
        path: final_path.clone(),
        source: e,
    })?;

    // Remove the original only after the new file is committed to disk.
    // If this remove fails the file is duplicated rather than lost,
    // which is the safer side of the dilemma — the principal can spot
    // and clean up; data loss would be silent.
    std::fs::remove_file(capture_path).map_err(|e| ContextifyError::Io {
        path: capture_path.to_path_buf(),
        source: e,
    })?;

    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Did, EnvelopeBuilder, EnvelopeDepth, EnvelopeUrgency};
    use crate::infrastructure::cognition::read_entries;
    use crate::infrastructure::markdown::embed_stamp;
    use tempfile::TempDir;

    /// Synthetic DID for tests. Per `feedback_no_real_dids_in_tests`,
    /// derive from a deterministic seed.
    fn rafa() -> Did {
        Did::from_ed25519_public_key(&[0xa1; 32])
    }

    /// Test adapter — answer scripted at construction time. Avoids
    /// network and gives the test full control over confidence +
    /// suggested queue.
    struct ScriptedAdapter {
        answer: Result<RouteSuggestion, CognitionError>,
    }

    impl CognitionPort for ScriptedAdapter {
        async fn route_capture(
            &self,
            _body: &str,
            _existing_queues: &[QueueHandle],
        ) -> Result<RouteSuggestion, CognitionError> {
            match &self.answer {
                Ok(s) => Ok(s.clone()),
                Err(CognitionError::NotConfigured) => Err(CognitionError::NotConfigured),
                Err(CognitionError::Abstained) => Err(CognitionError::Abstained),
                Err(other) => Err(CognitionError::Internal(other.to_string())),
            }
        }
    }

    /// Write a capture file with envelope frontmatter directly into the
    /// `inbox/triage` queue under the given root. Bypasses
    /// `capture_to_queue` (which requires a channel manifest) so the
    /// contextify tests can isolate routing logic from capture-side
    /// existence gates. Mirrors the on-disk shape capture_to_queue would
    /// have produced.
    fn write_inbox_triage_capture(queues_root: &Path, body: &str) -> PathBuf {
        let me = rafa();
        let handle = QueueHandle::parse("inbox:triage").unwrap();
        let envelope = EnvelopeBuilder::new(me.clone(), Recipient::new(me, handle))
            .depth(EnvelopeDepth::Subtle)
            .urgency(EnvelopeUrgency::Whenever)
            .source("test".to_string())
            .build();
        let dir = queues_root
            .join("inbox")
            .join("triage")
            .join("envelopes")
            .join("2026")
            .join("05")
            .join("06");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("20260506T120000Z-abcdef.md");
        let content = embed_stamp(body, Some(&envelope), None).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }

    #[tokio::test]
    async fn applies_when_above_threshold_and_different_queue() {
        let dir = TempDir::new().unwrap();
        let queues = dir.path().join("queues");
        let ledger = queues.join(".contextification.log");
        let capture_path = write_inbox_triage_capture(&queues, "this is a complaint about onboarding\n");
        // Pre-create a `pains` queue so discovery finds it.
        std::fs::create_dir_all(queues.join("inbox/pain")).unwrap();

        let adapter = ScriptedAdapter {
            answer: Ok(RouteSuggestion {
                queue: QueueHandle::parse("inbox:pain").unwrap(),
                confidence: 0.9,
                rationale: "complaint pattern".into(),
                model: "test-stub".into(),
                prompt_version: "v1".into(),
            }),
        };
        let now = Utc::now();
        let outcome =
            contextify_capture(&capture_path, &queues, &ledger, &adapter, 0.7, now)
                .await
                .unwrap();
        assert!(outcome.applied);
        assert_ne!(outcome.final_path, capture_path);
        assert!(outcome.final_path.starts_with(queues.join("inbox/pain")));
        assert!(!capture_path.exists(), "original removed");
        let entries = read_entries(&ledger).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].applied);
        assert_eq!(entries[0].suggested_queue.as_deref(), Some("inbox:pain"));
    }

    #[tokio::test]
    async fn skips_when_below_threshold() {
        let dir = TempDir::new().unwrap();
        let queues = dir.path().join("queues");
        let ledger = queues.join(".contextification.log");
        let capture_path = write_inbox_triage_capture(&queues, "ambiguous body");

        let adapter = ScriptedAdapter {
            answer: Ok(RouteSuggestion {
                queue: QueueHandle::parse("inbox:pain").unwrap(),
                confidence: 0.4,
                rationale: "uncertain".into(),
                model: "test-stub".into(),
                prompt_version: "v1".into(),
            }),
        };
        let outcome = contextify_capture(&capture_path, &queues, &ledger, &adapter, 0.7, Utc::now())
            .await
            .unwrap();
        assert!(!outcome.applied);
        assert!(matches!(
            outcome.skip_reason,
            Some(ContextifySkipReason::BelowThreshold { .. })
        ));
        assert!(capture_path.exists());
        let entries = read_entries(&ledger).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].applied);
        assert_eq!(entries[0].confidence, Some(0.4));
    }

    #[tokio::test]
    async fn no_op_when_adapter_not_configured() {
        let dir = TempDir::new().unwrap();
        let queues = dir.path().join("queues");
        let ledger = queues.join(".contextification.log");
        let capture_path = write_inbox_triage_capture(&queues, "anything");

        let adapter = ScriptedAdapter {
            answer: Err(CognitionError::NotConfigured),
        };
        let outcome = contextify_capture(&capture_path, &queues, &ledger, &adapter, 0.7, Utc::now())
            .await
            .unwrap();
        assert!(!outcome.applied);
        assert!(matches!(
            outcome.skip_reason,
            Some(ContextifySkipReason::AdapterNotConfigured)
        ));
        assert!(capture_path.exists());
        let entries = read_entries(&ledger).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].suggested_queue, None);
    }

    #[tokio::test]
    async fn explicit_queue_captures_bypass() {
        let dir = TempDir::new().unwrap();
        let queues = dir.path().join("queues");
        let ledger = queues.join(".contextification.log");

        // Capture goes to `area:health`, not `inbox:triage`.
        let me = rafa();
        let handle = QueueHandle::parse("area:health").unwrap();
        let envelope = EnvelopeBuilder::new(me.clone(), Recipient::new(me, handle))
            .depth(EnvelopeDepth::Subtle)
            .urgency(EnvelopeUrgency::Whenever)
            .source("test".to_string())
            .build();
        let target_dir = queues
            .join("area")
            .join("health")
            .join("envelopes")
            .join("2026")
            .join("05")
            .join("06");
        std::fs::create_dir_all(&target_dir).unwrap();
        let capture_path = target_dir.join("20260506T120000Z-abcdef.md");
        let content = embed_stamp("morning walk", Some(&envelope), None).unwrap();
        std::fs::write(&capture_path, content).unwrap();

        let adapter = ScriptedAdapter {
            answer: Ok(RouteSuggestion {
                queue: QueueHandle::parse("inbox:pain").unwrap(),
                confidence: 1.0,
                rationale: "irrelevant".into(),
                model: "test".into(),
                prompt_version: "v1".into(),
            }),
        };
        let outcome = contextify_capture(&capture_path, &queues, &ledger, &adapter, 0.7, Utc::now())
            .await
            .unwrap();
        assert!(!outcome.applied);
        assert!(matches!(
            outcome.skip_reason,
            Some(ContextifySkipReason::NotRoutable { .. })
        ));
        assert!(capture_path.exists());
    }

    #[tokio::test]
    async fn rewrites_envelope_recipient_on_move() {
        let dir = TempDir::new().unwrap();
        let queues = dir.path().join("queues");
        let ledger = queues.join(".contextification.log");
        let capture_path = write_inbox_triage_capture(&queues, "x");

        let adapter = ScriptedAdapter {
            answer: Ok(RouteSuggestion {
                queue: QueueHandle::parse("inbox:idea").unwrap(),
                confidence: 0.95,
                rationale: "y".into(),
                model: "stub".into(),
                prompt_version: "v1".into(),
            }),
        };
        let outcome = contextify_capture(&capture_path, &queues, &ledger, &adapter, 0.7, Utc::now())
            .await
            .unwrap();
        assert!(outcome.applied);
        let raw = std::fs::read_to_string(&outcome.final_path).unwrap();
        let parsed = parse_document(&raw).unwrap();
        let env = parsed.envelope.unwrap();
        assert_eq!(env.recipient.handle.as_str(), "inbox:idea");
    }

    #[test]
    fn discover_queues_finds_existing_handles() {
        let dir = TempDir::new().unwrap();
        let queues = dir.path().join("queues");
        std::fs::create_dir_all(queues.join("inbox/triage")).unwrap();
        std::fs::create_dir_all(queues.join("inbox/pain")).unwrap();
        std::fs::create_dir_all(queues.join("area/health")).unwrap();
        // Should ignore dotfiles.
        std::fs::write(queues.join(".contextification.log"), "").unwrap();

        let mut found: Vec<String> = discover_queues(&queues)
            .unwrap()
            .into_iter()
            .map(|h| h.as_str().to_string())
            .collect();
        found.sort();
        assert_eq!(
            found,
            vec!["area:health", "inbox:pain", "inbox:triage"]
        );
    }
}
