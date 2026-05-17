//! Use case: scaffold an AG-shaped envelope into a per-queue outbox.
//!
//! Reads the user's customizable template at `~/.secretariat/template.md`,
//! prepends a `$envelope:` frontmatter block, and writes the result to
//! `<root>/<alias-of-to>/<namespace>/<segments>/outbox/<timestamp>.md` —
//! one outbox per queue, derived from the recipient via the
//! `queue_dir` resolver. No stamp is added — the principal stamps
//! later via `sec stamp`.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rand::Rng;
use thiserror::Error;

use crate::domain::{
    Did, Envelope, EnvelopeBuilder, EnvelopeDepth, EnvelopeUrgency, Recipient,
};
use crate::infrastructure::markdown::{embed_stamp, MarkdownError};
use crate::infrastructure::queue_dir::AliasMap;

#[derive(Debug, Error)]
pub enum ComposeError {
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
pub struct ComposeRequest {
    pub from: Did,
    pub recipient: Recipient,
    pub depth: EnvelopeDepth,
    pub urgency: EnvelopeUrgency,
    pub source: String,
    pub cadence_hint: Option<String>,
    /// Raw markdown body. When `Some`, it replaces the AG template entirely —
    /// caller is responsible for shape. When `None`, the user's template at
    /// `~/.secretariat/template.md` is used as a scaffold.
    pub body: Option<String>,
}

pub fn compose_envelope(
    request: ComposeRequest,
    template_path: &Path,
    root: &Path,
    aliases: &AliasMap,
    now: DateTime<Utc>,
) -> Result<PathBuf, ComposeError> {
    let envelope = build_envelope(&request);
    let queue_root = crate::infrastructure::queue_dir::queue_dir(aliases, &request.recipient, root);
    let target_dir = queue_root.join("outbox");
    fs::create_dir_all(&target_dir).map_err(|e| ComposeError::Io {
        path: target_dir.clone(),
        source: e,
    })?;

    let filename = generate_filename(now);
    let target_path = target_dir.join(filename);

    let body_owned: String;
    let body: &str = match &request.body {
        Some(b) => b.as_str(),
        None => {
            // Per-channel template override (AGENTS.md rule #5): prefer
            // `<channel-dir>/template.md` when present; fall back to the
            // principal's global template.
            let channel_template = queue_root.join("template.md");
            let chosen = if channel_template.is_file() {
                &channel_template
            } else {
                template_path
            };
            body_owned = fs::read_to_string(chosen).map_err(|e| ComposeError::Io {
                path: chosen.to_path_buf(),
                source: e,
            })?;
            strip_existing_frontmatter(&body_owned)
        }
    };
    let content = embed_stamp(body, Some(&envelope), None)?;

    fs::write(&target_path, content).map_err(|e| ComposeError::Io {
        path: target_path.clone(),
        source: e,
    })?;
    Ok(target_path)
}

fn build_envelope(req: &ComposeRequest) -> Envelope {
    let mut b = EnvelopeBuilder::new(req.from.clone(), req.recipient.clone())
        .depth(req.depth)
        .urgency(req.urgency)
        .source(req.source.clone());
    if let Some(hint) = &req.cadence_hint {
        b = b.cadence_hint(hint.clone());
    }
    b.build()
}

/// Decision log #7: `<utc-iso8601>-<6-char-base32-suffix>.md`.
fn generate_filename(now: DateTime<Utc>) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut rng = rand::thread_rng();
    let suffix: String = (0..6)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect();
    format!("{}-{}.md", now.format("%Y%m%dT%H%M%SZ"), suffix)
}

/// If the template starts with frontmatter, drop it (we'll write our own).
/// Templates are user-customizable; many users will keep examples or prior
/// envelopes around. Stripping makes composition idempotent.
fn strip_existing_frontmatter(s: &str) -> &str {
    let s = s.strip_prefix('\u{FEFF}').unwrap_or(s);
    if !(s.starts_with("---\n") || s.starts_with("---\r\n")) {
        return s;
    }
    let after_open = if let Some(r) = s.strip_prefix("---\r\n") {
        r
    } else {
        s.strip_prefix("---\n").unwrap_or(s)
    };
    let mut start = 0usize;
    while let Some(rel) = after_open[start..].find("\n---") {
        let abs = start + rel;
        let tail = &after_open[abs + 4..];
        if let Some(rest) = tail.strip_prefix('\n') {
            return rest;
        }
        if let Some(rest) = tail.strip_prefix("\r\n") {
            return rest;
        }
        if tail.is_empty() {
            return "";
        }
        start = abs + 1;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::QueueHandle;
    use crate::infrastructure::markdown::parse_document;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn rafa_did() -> Did {
        Did::parse("did:web:rafa.equanimi.tech").unwrap()
    }

    fn marcelo_did() -> Did {
        Did::parse("did:web:marcelo.ballestiero.com").unwrap()
    }

    #[test]
    fn composes_to_peer_queue_outbox() {
        let dir = TempDir::new().unwrap();
        let template = dir.path().join("template.md");
        fs::write(&template, "# Title\n\nBody.\n").unwrap();
        let root = dir.path();
        let mut aliases = AliasMap::new(rafa_did());
        aliases.insert(marcelo_did(), "marcelo");

        let req = ComposeRequest {
            from: rafa_did(),
            recipient: Recipient::new(
                marcelo_did(),
                QueueHandle::parse("inbox:default").unwrap(),
            ),
            depth: EnvelopeDepth::Subtle,
            urgency: EnvelopeUrgency::Soon,
            source: "test".into(),
            cadence_hint: None,
            body: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 4, 30, 14, 25, 0).unwrap();
        let path = compose_envelope(req, &template, root, &aliases, now).unwrap();

        // Lives under <root>/marcelo/channels/inbox/default/outbox/.
        assert_eq!(
            path.parent().unwrap(),
            root.join("marcelo/channels/inbox/default/outbox"),
        );
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("20260430T142500Z-"));

        let parsed = parse_document(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(parsed.envelope.is_some());
        assert!(parsed.stamp.is_none());
        assert!(parsed.body.contains("Body."));
    }

    #[test]
    fn composes_self_letter_under_self_alias() {
        // Self-addressed envelope — owner == from. The resolver maps
        // to `_self`, the handle's namespace + segments give the
        // rest, and the file lands in that queue's `outbox/`.
        let dir = TempDir::new().unwrap();
        let template = dir.path().join("template.md");
        fs::write(&template, "# Self\n").unwrap();
        let root = dir.path();
        let aliases = AliasMap::new(rafa_did());

        let req = ComposeRequest {
            from: rafa_did(),
            recipient: Recipient::new(
                rafa_did(),
                QueueHandle::parse("inbox:default").unwrap(),
            ),
            depth: EnvelopeDepth::Gross,
            urgency: EnvelopeUrgency::Whenever,
            source: "test".into(),
            cadence_hint: None,
            body: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 4, 30, 9, 0, 0).unwrap();
        let path = compose_envelope(req, &template, root, &aliases, now).unwrap();
        assert_eq!(
            path.parent().unwrap(),
            root.join("_self/channels/inbox/default/outbox"),
        );
    }

    #[test]
    fn per_channel_template_overrides_global() {
        let dir = TempDir::new().unwrap();
        let global_template = dir.path().join("template.md");
        fs::write(&global_template, "# GLOBAL\nGlobal body.\n").unwrap();
        let root = dir.path();
        let aliases = AliasMap::new(rafa_did());

        // Plant a per-channel template at the recipient's queue dir.
        let channel_dir = root.join("_self/channels/secretariat/dev");
        fs::create_dir_all(&channel_dir).unwrap();
        fs::write(
            channel_dir.join("template.md"),
            "# CHANNEL\nChannel-specific body.\n",
        )
        .unwrap();

        let req = ComposeRequest {
            from: rafa_did(),
            recipient: Recipient::new(
                rafa_did(),
                QueueHandle::parse("secretariat:dev").unwrap(),
            ),
            depth: EnvelopeDepth::Subtle,
            urgency: EnvelopeUrgency::Whenever,
            source: "test".into(),
            cadence_hint: None,
            body: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 5, 18, 12, 0, 0).unwrap();
        let path = compose_envelope(req, &global_template, root, &aliases, now).unwrap();
        let parsed = parse_document(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(parsed.body.contains("Channel-specific body"));
        assert!(!parsed.body.contains("Global body"));
    }

    #[test]
    fn strips_existing_frontmatter_from_template() {
        let dir = TempDir::new().unwrap();
        let template = dir.path().join("template.md");
        fs::write(
            &template,
            "---\nfoo: bar\n---\n# After\nbody\n",
        )
        .unwrap();
        let root = dir.path();
        let aliases = AliasMap::new(rafa_did());

        let req = ComposeRequest {
            from: rafa_did(),
            recipient: Recipient::new(
                rafa_did(),
                QueueHandle::parse("inbox:scratch").unwrap(),
            ),
            depth: EnvelopeDepth::Gross,
            urgency: EnvelopeUrgency::Whenever,
            source: "test".into(),
            cadence_hint: None,
            body: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 4, 30, 9, 0, 0).unwrap();
        let path = compose_envelope(req, &template, root, &aliases, now).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("foo: bar"));
        assert!(content.contains("# After"));
        assert!(content.contains("$envelope:"));
    }
}
