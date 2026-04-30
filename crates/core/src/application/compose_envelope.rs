//! Use case: scaffold an AG-shaped envelope to the outbox.
//!
//! Reads the user's customizable template at `~/.secretariat/template.md`,
//! prepends a `$envelope:` frontmatter block, and writes the result to
//! `outbox/<sanitized-recipient>/<timestamp>.md`. No stamp is added — the
//! principal stamps later via `sec stamp`.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rand::Rng;
use thiserror::Error;

use crate::domain::{Did, Envelope, EnvelopeBuilder, EnvelopeDepth, EnvelopeUrgency};
use crate::infrastructure::did_web_resolver::sanitize_did_for_filename;
use crate::infrastructure::markdown::{embed_stamp, MarkdownError};

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
    pub to: Option<Did>,
    pub depth: EnvelopeDepth,
    pub urgency: EnvelopeUrgency,
    pub source: String,
    pub cadence_hint: Option<String>,
}

pub fn compose_envelope(
    request: ComposeRequest,
    template_path: &Path,
    outbox_root: &Path,
    now: DateTime<Utc>,
) -> Result<PathBuf, ComposeError> {
    let template = fs::read_to_string(template_path).map_err(|e| ComposeError::Io {
        path: template_path.to_path_buf(),
        source: e,
    })?;

    let envelope = build_envelope(&request);
    let recipient_dir_name = match &request.to {
        Some(did) => sanitize_did_for_filename(did.as_str()),
        None => "_self".to_string(),
    };

    let target_dir = outbox_root.join(recipient_dir_name);
    fs::create_dir_all(&target_dir).map_err(|e| ComposeError::Io {
        path: target_dir.clone(),
        source: e,
    })?;

    let filename = generate_filename(now);
    let target_path = target_dir.join(filename);

    let body = strip_existing_frontmatter(&template);
    let content = embed_stamp(body, Some(&envelope), None)?;

    fs::write(&target_path, content).map_err(|e| ComposeError::Io {
        path: target_path.clone(),
        source: e,
    })?;
    Ok(target_path)
}

fn build_envelope(req: &ComposeRequest) -> Envelope {
    let mut b = EnvelopeBuilder::new(req.from.clone())
        .depth(req.depth)
        .urgency(req.urgency)
        .source(req.source.clone());
    if let Some(to) = &req.to {
        b = b.to(to.clone());
    }
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
    use crate::infrastructure::markdown::parse_document;
    use chrono::TimeZone;
    use tempfile::TempDir;

    #[test]
    fn composes_to_named_recipient_dir() {
        let dir = TempDir::new().unwrap();
        let template = dir.path().join("template.md");
        fs::write(&template, "# Title\n\nBody.\n").unwrap();
        let outbox = dir.path().join("outbox");

        let req = ComposeRequest {
            from: Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            to: Some(Did::parse("did:web:marcelo.ballestiero.com").unwrap()),
            depth: EnvelopeDepth::Subtle,
            urgency: EnvelopeUrgency::Soon,
            source: "test".into(),
            cadence_hint: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 4, 30, 14, 25, 0).unwrap();
        let path = compose_envelope(req.clone(), &template, &outbox, now).unwrap();

        assert!(path
            .parent()
            .unwrap()
            .to_string_lossy()
            .contains("did_web_marcelo.ballestiero.com"));
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
    fn composes_self_addressed_when_no_to() {
        let dir = TempDir::new().unwrap();
        let template = dir.path().join("template.md");
        fs::write(&template, "# Self\n").unwrap();
        let outbox = dir.path().join("outbox");

        let req = ComposeRequest {
            from: Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            to: None,
            depth: EnvelopeDepth::Gross,
            urgency: EnvelopeUrgency::Whenever,
            source: "test".into(),
            cadence_hint: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 4, 30, 9, 0, 0).unwrap();
        let path = compose_envelope(req, &template, &outbox, now).unwrap();
        assert!(path.parent().unwrap().ends_with("_self"));
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
        let outbox = dir.path().join("outbox");

        let req = ComposeRequest {
            from: Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            to: None,
            depth: EnvelopeDepth::Gross,
            urgency: EnvelopeUrgency::Whenever,
            source: "test".into(),
            cadence_hint: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 4, 30, 9, 0, 0).unwrap();
        let path = compose_envelope(req, &template, &outbox, now).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        // The template's `foo: bar` frontmatter must NOT appear in the output.
        assert!(!content.contains("foo: bar"));
        assert!(content.contains("# After"));
        assert!(content.contains("$envelope:"));
    }
}
