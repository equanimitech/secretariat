//! Use case: capture a body of text into a local queue.
//!
//! Where `compose_envelope` addresses a peer (`owner != self_did`) and
//! the principal must later stamp + send, `capture_to_queue` addresses
//! the principal's own queue (`owner == self_did`). Same envelope
//! primitive, different routing: a self-owned envelope stays on disk;
//! a peer-owned one delivers to the peer's relay after stamping.
//!
//! Stamps are still allowed — a tamper-evident self-attestation on a
//! journal entry is a valid use case — but never required.
//!
//! On-disk layout depends on the queue handle's top namespace:
//!
//! - **flat handles** (`inbox:triage`, `area:health`, `project:foo`) land
//!   at `<flat_queues>/<ns>/<slug>/envelopes/YYYY/MM/DD/<timestamp>.md`.
//!   The `envelopes/` subdir is required: `list_inbox_files` walks the
//!   substrate tree collecting `.md` leaves only from inside
//!   `envelopes/` directories. Captures written outside that convention
//!   are invisible to the review surface.
//! - **`channel:` handles** (`channel:secretariat:dev`,
//!   `channel:dommage-corporel:paris-cohort`) land at
//!   `<channel_tree>/<segments-after-channel>/envelopes/YYYY/MM/DD/<timestamp>.md`
//!   (v0.3 channel-dir substrate — time-sharded from day one so a
//!   channel can carry years of correspondence without flat-directory
//!   pathologies).
//!
//! In both cases the envelope inside the file is identical — recipient is
//! `(self_did, QueueHandle)`. Layout is a storage detail.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, Utc};
use rand::Rng;
use thiserror::Error;

use crate::domain::{
    Did, Envelope, EnvelopeBuilder, EnvelopeDepth, EnvelopeUrgency, QueueHandle, Recipient,
};
use crate::infrastructure::channel_def_store::channel_def_exists_in_dir;
use crate::infrastructure::markdown::{embed_stamp, MarkdownError};

/// Top-namespace token that routes a capture into the channel-dir layout.
const CHANNEL_NS: &str = "channel";

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
    #[error(
        "queue handle `{handle}` has top namespace `channel` but only one segment; \
        channel handles require at least `channel:<name>` (two segments)"
    )]
    ChannelHandleTooShallow { handle: String },
    #[error(
        "channel `{handle}` does not exist — search existing channels \
        with `sec channels list` (or the `list_channels` MCP tool) for \
        a relevant one, or create it with `sec channels create {handle}` \
        (or the `create_channel` MCP tool)"
    )]
    ChannelNotFound { handle: String },
}

#[derive(Debug, Clone)]
pub struct CaptureRequest {
    /// The principal's DID — captures always have `from = self`.
    pub from: Did,
    /// Target queue (e.g. `inbox:triage`, `area:health`,
    /// `channel:secretariat:dev`).
    pub queue: QueueHandle,
    /// Raw markdown body. Captures are unstructured by design — no AG
    /// template scaffold, no headline/lede shape obligation.
    pub body: String,
    /// Free-form origin marker (e.g. `"idea-skill"`, `"quick-pane"`,
    /// `"mcp-capture"`). Lets the review session group by source if it
    /// wants.
    pub source: String,
}

/// Storage roots the capture writer chooses between based on the handle's
/// top namespace.
#[derive(Debug, Clone, Copy)]
pub struct CaptureRoots<'a> {
    /// Root for flat-handle captures. Layout:
    /// `<flat_queues>/<namespace>/<slug>/<timestamp>.md`.
    pub flat_queues: &'a Path,
    /// Root for `channel:` handles. Layout:
    /// `<channel_tree>/<segments>/envelopes/YYYY/MM/DD/<timestamp>.md`.
    /// Slice 1: pass `KeyPaths::channel_root(&OrgAlias::me())`.
    pub channel_tree: &'a Path,
}

/// Capture a body into a local queue. Writes the file and returns the
/// path. Never stamps; never sends. Idempotency is timestamp+suffix —
/// two calls in the same second produce different files.
pub fn capture_to_queue(
    request: CaptureRequest,
    roots: CaptureRoots<'_>,
    now: DateTime<Utc>,
) -> Result<PathBuf, CaptureError> {
    let envelope = build_envelope(&request);
    let target_dir = resolve_target_dir(&request.queue, roots, now)?;

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

fn resolve_target_dir(
    queue: &QueueHandle,
    roots: CaptureRoots<'_>,
    now: DateTime<Utc>,
) -> Result<PathBuf, CaptureError> {
    if queue.top_namespace() == CHANNEL_NS {
        let segments = queue.segments();
        if segments.len() < 2 {
            return Err(CaptureError::ChannelHandleTooShallow {
                handle: queue.as_str().to_string(),
            });
        }
        let mut dir = roots.channel_tree.to_path_buf();
        // Skip the leading `channel:` token — `channel_tree` already encodes it.
        for seg in &segments[1..] {
            dir.push(seg);
        }
        // Existence gate: refuse to capture into a channel whose manifest
        // (`channel.md` or legacy `.channelDef`) is absent. Auto-vivifying
        // the directory tree on capture would let a typo silently spawn a
        // phantom channel that never appears in `list_channels` and has no
        // roster/governance.
        if !channel_def_exists_in_dir(&dir) {
            return Err(CaptureError::ChannelNotFound {
                handle: queue.as_str().to_string(),
            });
        }
        dir.push("envelopes");
        dir.push(format!("{:04}", now.year()));
        dir.push(format!("{:02}", now.month()));
        dir.push(format!("{:02}", now.day()));
        Ok(dir)
    } else {
        // Flat handles share the channel-branch's `envelopes/YYYY/MM/DD/`
        // convention so `list_inbox_files` finds them — its walker only
        // collects `.md` leaves from inside an `envelopes/` directory.
        let mut dir = roots.flat_queues.to_path_buf();
        dir.push(queue.top_namespace());
        for seg in queue.slug().split(':') {
            dir.push(seg);
        }
        dir.push("envelopes");
        dir.push(format!("{:04}", now.year()));
        dir.push(format!("{:02}", now.month()));
        dir.push(format!("{:02}", now.day()));
        Ok(dir)
    }
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

    fn roots_under(dir: &Path) -> (PathBuf, PathBuf) {
        (dir.join("queues"), dir.join("channels"))
    }

    /// Plant a minimal `channel.md` so capture_to_queue's existence
    /// gate clears. Mirrors what `create_channel` would have written.
    fn touch_channel(channel_tree: &Path, handle: &str) {
        let h = QueueHandle::parse(handle).unwrap();
        let mut dir = channel_tree.to_path_buf();
        for seg in h.segments().iter().skip(1) {
            dir.push(seg);
        }
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("channel.md"),
            "---\n$type: tech.equanimi.secretariat.channelDef\n---\n",
        )
        .unwrap();
    }

    #[test]
    fn captures_to_namespace_slug_subdir() {
        let dir = TempDir::new().unwrap();
        let (queues, channel_tree) = roots_under(dir.path());

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("inbox:triage").unwrap(),
            body: "tell dad chapter 3 needs more pressure\n".to_string(),
            source: "idea-skill".to_string(),
        };

        let now = Utc.with_ymd_and_hms(2026, 5, 5, 10, 0, 0).unwrap();
        let path = capture_to_queue(
            req,
            CaptureRoots {
                flat_queues: &queues,
                channel_tree: &channel_tree,
            },
            now,
        )
        .unwrap();

        // <queues>/inbox/triage/envelopes/2026/05/05/<timestamp>.md
        let parent = path.parent().unwrap();
        assert!(
            parent.ends_with("inbox/triage/envelopes/2026/05/05"),
            "expected time-sharded envelopes/ path, got {}",
            parent.display()
        );
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
        let (queues, channel_tree) = roots_under(dir.path());

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("area:health").unwrap(),
            body: "morning walk feels harder this week".into(),
            source: "quick-pane".into(),
        };

        let now = Utc.with_ymd_and_hms(2026, 5, 5, 10, 0, 0).unwrap();
        let path = capture_to_queue(
            req,
            CaptureRoots {
                flat_queues: &queues,
                channel_tree: &channel_tree,
            },
            now,
        )
        .unwrap();
        assert!(
            path.parent()
                .unwrap()
                .ends_with("area/health/envelopes/2026/05/05"),
            "expected time-sharded envelopes/ path, got {}",
            path.parent().unwrap().display()
        );
    }

    #[test]
    fn captures_with_deep_flat_handle_split_slug_into_segments() {
        // `area:articles:equanimitech` is a legal handle whose slug is
        // `articles:equanimitech` — colon-depth becomes tree-depth so
        // the reader can browse nested areas the same way it browses
        // nested channels.
        let dir = TempDir::new().unwrap();
        let (queues, channel_tree) = roots_under(dir.path());

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("area:articles:equanimitech").unwrap(),
            body: "UI navigates; MCP CRUDs".into(),
            source: "test".into(),
        };
        let now = Utc.with_ymd_and_hms(2026, 5, 12, 3, 25, 37).unwrap();
        let path = capture_to_queue(
            req,
            CaptureRoots {
                flat_queues: &queues,
                channel_tree: &channel_tree,
            },
            now,
        )
        .unwrap();

        assert!(
            path.parent()
                .unwrap()
                .ends_with("area/articles/equanimitech/envelopes/2026/05/12"),
            "expected nested area path with envelopes shard, got {}",
            path.parent().unwrap().display()
        );
    }

    #[test]
    fn captures_with_channel_namespace_use_time_sharded_tree() {
        let dir = TempDir::new().unwrap();
        let (queues, channel_tree) = roots_under(dir.path());
        touch_channel(&channel_tree, "channel:secretariat:dev");

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("channel:secretariat:dev").unwrap(),
            body: "v0.3 substrate first run".into(),
            source: "mcp-capture".into(),
        };

        let now = Utc.with_ymd_and_hms(2026, 5, 12, 14, 30, 55).unwrap();
        let path = capture_to_queue(
            req,
            CaptureRoots {
                flat_queues: &queues,
                channel_tree: &channel_tree,
            },
            now,
        )
        .unwrap();

        // <channel_tree>/secretariat/dev/envelopes/2026/05/12/<ts>.md
        assert!(
            path.parent()
                .unwrap()
                .ends_with("secretariat/dev/envelopes/2026/05/12"),
            "expected time-sharded path, got {}",
            path.display()
        );
        // Flat queues dir untouched.
        assert!(
            !queues.join("channel").exists(),
            "flat queues tree must not be polluted by channel captures"
        );

        // Envelope still reads the handle verbatim.
        let parsed = parse_document(&fs::read_to_string(&path).unwrap()).unwrap();
        let env = parsed.envelope.unwrap();
        assert_eq!(env.recipient.handle.as_str(), "channel:secretariat:dev");
        assert!(env.recipient.is_local(&rafa()));
    }

    #[test]
    fn captures_with_nested_channel_handle() {
        let dir = TempDir::new().unwrap();
        let (queues, channel_tree) = roots_under(dir.path());
        touch_channel(&channel_tree, "channel:dommage-corporel:paris-cohort");

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("channel:dommage-corporel:paris-cohort").unwrap(),
            body: "first dossier review note".into(),
            source: "idea-skill".into(),
        };

        let now = Utc.with_ymd_and_hms(2026, 1, 9, 8, 5, 0).unwrap();
        let path = capture_to_queue(
            req,
            CaptureRoots {
                flat_queues: &queues,
                channel_tree: &channel_tree,
            },
            now,
        )
        .unwrap();
        assert!(path
            .parent()
            .unwrap()
            .ends_with("dommage-corporel/paris-cohort/envelopes/2026/01/09"));
    }

    #[test]
    fn channel_handle_with_only_top_token_errors() {
        // Should be rejected at parse time normally (single-segment handle
        // can't parse), but we sanity-check the routing branch independently.
        // Construct a two-segment `channel:foo` to confirm it does NOT error
        // (one segment after `channel` is enough).
        let dir = TempDir::new().unwrap();
        let (queues, channel_tree) = roots_under(dir.path());
        touch_channel(&channel_tree, "channel:dev");

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("channel:dev").unwrap(),
            body: "ok".into(),
            source: "test".into(),
        };
        let now = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let path = capture_to_queue(
            req,
            CaptureRoots {
                flat_queues: &queues,
                channel_tree: &channel_tree,
            },
            now,
        )
        .unwrap();
        assert!(path.parent().unwrap().ends_with("dev/envelopes/2026/05/12"));
    }

    #[test]
    fn capture_to_unknown_channel_errors() {
        // No `channel.md` planted → capture must refuse rather than
        // silently vivify a phantom channel directory.
        let dir = TempDir::new().unwrap();
        let (queues, channel_tree) = roots_under(dir.path());

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("channel:does-not:exist").unwrap(),
            body: "should be rejected".into(),
            source: "test".into(),
        };
        let now = Utc.with_ymd_and_hms(2026, 5, 14, 9, 0, 0).unwrap();
        let err = capture_to_queue(
            req,
            CaptureRoots {
                flat_queues: &queues,
                channel_tree: &channel_tree,
            },
            now,
        )
        .unwrap_err();
        match err {
            CaptureError::ChannelNotFound { handle } => {
                assert_eq!(handle, "channel:does-not:exist");
            }
            other => panic!("expected ChannelNotFound, got {other:?}"),
        }
        // No phantom directory left behind.
        assert!(!channel_tree.join("does-not").exists());
    }
}
