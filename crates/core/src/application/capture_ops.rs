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
//! On-disk layout (v0.5 namespace-collapse —
//! `docs/pitches/2026-05-17-collapse-namespaces.md`):
//!
//! - `Root::Self_` → `<vault>/_self/channels/<segs>/envelopes/YYYY/MM/DD/<ts>.md`
//! - `Root::Org(alias)` → `<vault>/orgs/<alias>/channels/<segs>/envelopes/YYYY/MM/DD/<ts>.md`
//!
//! Handle segments map 1:1 to directory depth — no namespace prefix
//! token. The `envelopes/YYYY/MM/DD/` time-shard is required so a
//! channel can carry years of correspondence without flat-directory
//! pathologies.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, Utc};
use rand::Rng;
use thiserror::Error;

use crate::domain::{
    AgSource, Did, Envelope, EnvelopeBuilder, EnvelopeDepth, EnvelopeUrgency, QueueHandle,
    Recipient, Root,
};
use crate::infrastructure::channel_def_store::{channel_def_exists_in_dir, channel_dir};
use crate::infrastructure::markdown::{embed_stamp, MarkdownError};
use crate::infrastructure::preferences::CognitionPrefs;

use super::ag_extract::{try_extract_ag, AgExtractOutcome, AuthorAgFields};

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
    /// Target queue (a bare handle, e.g. `triage`, `articles`,
    /// `dommage-corporel:paris-cohort`).
    pub queue: QueueHandle,
    /// Raw markdown body. Captures are unstructured by design — no AG
    /// template scaffold, no headline/lede shape obligation.
    pub body: String,
    /// Free-form origin marker (e.g. `"idea-skill"`, `"quick-pane"`,
    /// `"mcp-capture"`). Lets the review session group by source if it
    /// wants.
    pub source: String,
    /// Optional author-supplied AG fields. When any is set, the envelope
    /// is written with the author's values verbatim and the AI auto-fill
    /// pass stands down (see `capture_to_queue_with_ag`).
    pub title: Option<String>,
    pub lede: Option<String>,
    pub summary: Option<String>,
}

/// Compute the channels root for a queue-root under a vault.
/// `_self` → `<vault>/_self/channels`; `Org(alias)` →
/// `<vault>/orgs/<alias>/channels`.
pub fn channels_root_for(vault_root: &Path, root: &Root) -> PathBuf {
    match root {
        Root::Self_ => vault_root.join("_self").join("channels"),
        Root::Org(alias) => vault_root
            .join("orgs")
            .join(alias.as_str())
            .join("channels"),
    }
}

/// Capture a body into a local queue. Writes the file and returns the
/// path. Never stamps; never sends. Idempotency is timestamp+suffix —
/// two calls in the same second produce different files.
pub fn capture_to_queue(
    request: CaptureRequest,
    vault_root: &Path,
    root: &Root,
    now: DateTime<Utc>,
) -> Result<PathBuf, CaptureError> {
    capture_to_queue_inner(request, vault_root, root, now, None)
}

/// Async wrapper that runs an AG-extraction pass before capturing.
///
/// When `request.title` / `lede` / `summary` are all `None` and the
/// body is substantive (see [`super::ag_extract::body_warrants_ag`]),
/// call the configured cognition adapter; populate the request with
/// the result; tag `ag_source = "ai"` so receivers can tell.
///
/// Falls through to a normal [`capture_to_queue`] write when the
/// author supplied any AG field, the body is too short, no adapter is
/// configured, or the adapter call fails. Never crashes a capture.
pub async fn capture_to_queue_with_ag(
    request: CaptureRequest,
    vault_root: &Path,
    root: &Root,
    cognition_prefs: &CognitionPrefs,
    now: DateTime<Utc>,
) -> Result<PathBuf, CaptureError> {
    let (request, ag_source) = enrich_with_ag(request, cognition_prefs).await;
    capture_to_queue_inner(request, vault_root, root, now, ag_source)
}

async fn enrich_with_ag(
    mut request: CaptureRequest,
    cognition_prefs: &CognitionPrefs,
) -> (CaptureRequest, Option<AgSource>) {
    let author = AuthorAgFields {
        title: request.title.clone(),
        lede: request.lede.clone(),
        summary: request.summary.clone(),
    };
    if author.any_set() {
        return (request, None);
    }
    let outcome = try_extract_ag(&request.body, &author, cognition_prefs).await;
    match outcome {
        AgExtractOutcome::Generated(fields) => {
            request.title = Some(fields.title);
            request.lede = Some(fields.lede);
            request.summary = Some(fields.summary);
            (request, Some(AgSource::Ai))
        }
        _ => (request, None),
    }
}

fn capture_to_queue_inner(
    request: CaptureRequest,
    vault_root: &Path,
    root: &Root,
    now: DateTime<Utc>,
    ag_source: Option<AgSource>,
) -> Result<PathBuf, CaptureError> {
    let mut envelope = build_envelope(&request);
    if let Some(src) = ag_source {
        envelope.ag_source = Some(src);
    }
    let target_dir = resolve_target_dir(&request.queue, vault_root, root, now)?;

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
    vault_root: &Path,
    root: &Root,
    now: DateTime<Utc>,
) -> Result<PathBuf, CaptureError> {
    let channels_root = channels_root_for(vault_root, root);
    let dir = channel_dir(&channels_root, queue);
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
    let mut shard = dir;
    shard.push("envelopes");
    shard.push(format!("{:04}", now.year()));
    shard.push(format!("{:02}", now.month()));
    shard.push(format!("{:02}", now.day()));
    Ok(shard)
}

fn build_envelope(req: &CaptureRequest) -> Envelope {
    let mut b = EnvelopeBuilder::new(
        req.from.clone(),
        Recipient::new(req.from.clone(), req.queue.clone()),
    )
    .depth(EnvelopeDepth::Subtle)
    .urgency(EnvelopeUrgency::Whenever)
    .source(req.source.clone());
    if let Some(t) = &req.title {
        b = b.title(t.clone());
    }
    if let Some(l) = &req.lede {
        b = b.lede(l.clone());
    }
    if let Some(s) = &req.summary {
        b = b.summary(s.clone());
    }
    b.build()
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

    /// Plant a minimal `channel.md` so capture_to_queue's existence
    /// gate clears. Mirrors what `create_channel` would have written.
    fn touch_channel(vault_root: &Path, root: &Root, handle: &str) {
        let h = QueueHandle::parse(handle).unwrap();
        let channels_root = channels_root_for(vault_root, root);
        let dir = channel_dir(&channels_root, &h);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("channel.md"),
            "---\n$type: tech.equanimi.secretariat.channelDef\n---\n",
        )
        .unwrap();
    }

    #[test]
    fn captures_self_handle_under_self_channels() {
        let dir = TempDir::new().unwrap();
        let root = Root::Self_;
        touch_channel(dir.path(), &root, "triage");

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("triage").unwrap(),
            body: "tell dad chapter 3 needs more pressure\n".to_string(),
            source: "idea-skill".to_string(),
            title: None,
            lede: None,
            summary: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 5, 5, 10, 0, 0).unwrap();
        let path = capture_to_queue(req, dir.path(), &root, now).unwrap();

        let parent = path.parent().unwrap();
        assert!(
            parent.ends_with("_self/channels/triage/envelopes/2026/05/05"),
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
        assert_eq!(env.recipient.handle.as_str(), "triage");
        assert!(env.recipient.is_local(&rafa()));
    }

    #[test]
    fn captures_deep_self_handle_splits_segments() {
        let dir = TempDir::new().unwrap();
        let root = Root::Self_;
        touch_channel(dir.path(), &root, "articles:equanimitech");

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("articles:equanimitech").unwrap(),
            body: "UI navigates; MCP CRUDs".into(),
            source: "test".into(),
            title: None,
            lede: None,
            summary: None,
        };
        let now = Utc.with_ymd_and_hms(2026, 5, 12, 3, 25, 37).unwrap();
        let path = capture_to_queue(req, dir.path(), &root, now).unwrap();

        assert!(
            path.parent()
                .unwrap()
                .ends_with("_self/channels/articles/equanimitech/envelopes/2026/05/12"),
            "expected nested self-channel path with envelopes shard, got {}",
            path.parent().unwrap().display()
        );
    }

    #[test]
    fn captures_under_org_root() {
        let dir = TempDir::new().unwrap();
        let alias = crate::domain::OrgAlias::parse("themia.pro").unwrap();
        let root = Root::Org(alias);
        touch_channel(dir.path(), &root, "dommage-corporel:paris-cohort");

        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("dommage-corporel:paris-cohort").unwrap(),
            body: "first dossier review note".into(),
            source: "idea-skill".into(),
            title: None,
            lede: None,
            summary: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 1, 9, 8, 5, 0).unwrap();
        let path = capture_to_queue(req, dir.path(), &root, now).unwrap();
        assert!(
            path.parent().unwrap().ends_with(
                "orgs/themia.pro/channels/dommage-corporel/paris-cohort/envelopes/2026/01/09"
            ),
            "got {}",
            path.parent().unwrap().display(),
        );
    }

    #[test]
    fn capture_to_unknown_channel_errors() {
        let dir = TempDir::new().unwrap();
        let root = Root::Self_;
        // No `channel.md` planted → capture must refuse rather than
        // silently vivify a phantom channel directory.
        let req = CaptureRequest {
            from: rafa(),
            queue: QueueHandle::parse("does-not:exist").unwrap(),
            body: "should be rejected".into(),
            source: "test".into(),
            title: None,
            lede: None,
            summary: None,
        };
        let now = Utc.with_ymd_and_hms(2026, 5, 14, 9, 0, 0).unwrap();
        let err = capture_to_queue(req, dir.path(), &root, now).unwrap_err();
        match err {
            CaptureError::ChannelNotFound { handle } => {
                assert_eq!(handle, "does-not:exist");
            }
            other => panic!("expected ChannelNotFound, got {other:?}"),
        }
        // No phantom directory left behind.
        assert!(!dir.path().join("_self/channels/does-not").exists());
    }
}
