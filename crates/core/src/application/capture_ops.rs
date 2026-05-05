//! Use case: capture a body of text into a local queue.
//!
//! The local-queue side of the v0.3 substrate (see
//! `docs/pitches/2026-05-05-event-sourced-envelope-substrate.md`).
//!
//! Where `compose_envelope` produces a peer-addressed letter draft that
//! the principal must later stamp + send, `capture_to_queue` produces a
//! `Recipient::LocalQueue(handle)` envelope: a thought, an idea, a
//! future-self note. By construction it cannot be stamped (the domain
//! invariant on `Recipient` rejects it), and it never leaves the
//! principal's machine.
//!
//! Files land at `<queues>/<namespace>/<slug>/<timestamp>.md` so the
//! filesystem layout mirrors the queue handle and a `find` over the
//! tree is a usable secondary access path.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rand::Rng;
use thiserror::Error;

use crate::domain::{
    Did, Envelope, EnvelopeBuilder, EnvelopeDepth, EnvelopeUrgency, QueueHandle, Recipient,
};
use crate::infrastructure::markdown::{embed_stamp, MarkdownError};

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("markdown error: {0}")]
    Markdown(#[from] MarkdownError),
}

#[derive(Debug, Clone)]
pub struct CaptureRequest {
    /// The principal's DID — captures always have `from = self`.
    pub from: Did,
    /// Target queue (e.g. `inbox:triage`, `area:health`).
    pub queue: QueueHandle,
    /// Raw markdown body. Captures are unstructured by design — no AG
    /// template scaffold, no headline/lede shape obligation.
    pub body: String,
    /// Free-form origin marker (e.g. `"idea-skill"`, `"quick-pane"`,
    /// `"mcp-capture"`). Lets the review session group by source if it
    /// wants.
    pub source: String,
}

/// Capture a body into a local queue. Writes the file and returns the
/// path. Never stamps; never sends. Idempotency is timestamp+suffix —
/// two calls in the same second produce different files.
pub fn capture_to_queue(
    request: CaptureRequest,
    queues_root: &Path,
    now: DateTime<Utc>,
) -> Result<PathBuf, CaptureError> {
    let envelope = build_envelope(&request);

    // <queues_root>/<namespace>/<slug>/<timestamp>.md
    let target_dir = queues_root
        .join(request.queue.namespace())
        .join(request.queue.slug());
    fs::create_dir_all(&target_dir).map_err(|e| CaptureError::Io {
        path: target_dir.clone(),
        source: e,
    })?;

    let filename = generate_filename(now);
    let target_path = target_dir.join(filename);

    let content = embed_stamp(&request.body, Some(&envelope), None)?;
    fs::write(&target_path, content).map_err(|e| CaptureError::Io {
        path: target_path.clone(),
        source: e,
    })?;
    Ok(target_path)
}

fn build_envelope(req: &CaptureRequest) -> Envelope {
    EnvelopeBuilder::new(
        req.from.clone(),
        Recipient::new(req.from.clone(), req.queue.clone()),
    )
    .depth(EnvelopeDepth::Subtle)
    .urgency(EnvelopeUrgency::Whenever)
    .source(req.source.clone())
    .build()
}

/// Same shape as `compose_envelope`: `<utc-iso8601>-<6-char-base32>.md`.
fn generate_filename(now: DateTime<Utc>) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut rng = rand::thread_rng();
    let suffix: String = (0..6)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect();
    format!("{}-{}.md", now.format("%Y%m%dT%H%M%SZ"), suffix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::markdown::parse_document;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn rafa() -> Did {
        Did::parse("did:web:rafa.equanimi.tech").unwrap()
    }

    #[test]
    fn captures_to_namespace_slug_subdir() {
        let dir = TempDir::new().unwrap();
        let queues = dir.path().join("queues");

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("inbox:triage").unwrap(),
            body: "tell dad chapter 3 needs more pressure\n".to_string(),
            source: "idea-skill".to_string(),
        };

        let now = Utc.with_ymd_and_hms(2026, 5, 5, 10, 0, 0).unwrap();
        let path = capture_to_queue(req, &queues, now).unwrap();

        // <queues>/inbox/triage/<timestamp>.md
        let parent = path.parent().unwrap();
        assert!(parent.ends_with("inbox/triage"));
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("20260505T100000Z-"));

        let parsed = parse_document(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(parsed.envelope.is_some());
        assert!(parsed.stamp.is_none(), "captures start unstamped");
        assert!(parsed.body.contains("chapter 3"));

        let env = parsed.envelope.unwrap();
        assert_eq!(env.recipient.handle.as_str(), "inbox:triage");
        assert!(env.recipient.is_local(&rafa()));
    }

    #[test]
    fn captures_to_arbitrary_namespace() {
        let dir = TempDir::new().unwrap();
        let queues = dir.path().join("queues");

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("area:health").unwrap(),
            body: "morning walk feels harder this week".into(),
            source: "quick-pane".into(),
        };

        let now = Utc.with_ymd_and_hms(2026, 5, 5, 10, 0, 0).unwrap();
        let path = capture_to_queue(req, &queues, now).unwrap();
        assert!(path.parent().unwrap().ends_with("area/health"));
    }
}
