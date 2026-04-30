//! Markdown frontmatter parsing and embedding.
//!
//! Wire format (per the plan's "Stamped envelope file" spec):
//!
//! ```text
//! ---
//! $envelope:
//!   $type: app.equanimi.secretariat.envelope
//!   ...
//! $attestation:
//!   $type: app.equanimi.secretariat.stamp
//!   ...
//! ---
//!
//! # Body
//! ```
//!
//! The frontmatter block is delimited by `^---\n` ... `\n---\n`. Either or both
//! of `$envelope` / `$attestation` may be absent. The body starts immediately
//! after the closing `---\n`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{Envelope, Stamp};

#[derive(Debug, Error)]
pub enum MarkdownError {
    #[error("frontmatter is malformed: {0}")]
    MalformedFrontmatter(String),
    #[error("frontmatter YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
    #[error("frontmatter YAML emit error: {0}")]
    YamlEmit(String),
}

/// Parsed view of a markdown document, separated into frontmatter records
/// and body. The `raw_frontmatter` field preserves the original block bytes
/// for round-trip diagnostics — round-tripping through `embed_stamp`
/// re-emits canonical YAML, not the original.
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub envelope: Option<Envelope>,
    pub stamp: Option<Stamp>,
    pub body: String,
    pub raw_frontmatter: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct FrontmatterShape {
    #[serde(rename = "$envelope", default, skip_serializing_if = "Option::is_none")]
    envelope: Option<Envelope>,
    #[serde(rename = "$attestation", default, skip_serializing_if = "Option::is_none")]
    attestation: Option<Stamp>,
}

const DELIM: &str = "---";

/// Parse a markdown document, separating frontmatter (if any) from body.
///
/// Strips a leading BOM. Does not normalize line endings — that's the
/// responsibility of [`crate::domain::canonical_body_hash`] when computing
/// the hash. The body string is preserved as-is otherwise.
pub fn parse_document(content: &str) -> Result<ParsedDocument, MarkdownError> {
    let stripped = content.strip_prefix('\u{FEFF}').unwrap_or(content);

    // Detect frontmatter: file starts with `---` followed by newline (LF or CRLF).
    if !starts_with_delim(stripped) {
        return Ok(ParsedDocument {
            envelope: None,
            stamp: None,
            body: stripped.to_string(),
            raw_frontmatter: None,
        });
    }

    // Skip the opening delimiter line.
    let after_open = skip_delim_line(stripped).ok_or_else(|| {
        MarkdownError::MalformedFrontmatter("opening delimiter missing newline".into())
    })?;

    // Find the closing delimiter line.
    let (yaml_block, after_close) = split_at_closing_delim(after_open).ok_or_else(|| {
        MarkdownError::MalformedFrontmatter("closing `---` line not found".into())
    })?;

    let parsed: FrontmatterShape = if yaml_block.trim().is_empty() {
        FrontmatterShape::default()
    } else {
        serde_yaml::from_str(yaml_block)?
    };

    Ok(ParsedDocument {
        envelope: parsed.envelope,
        stamp: parsed.attestation,
        body: after_close.to_string(),
        raw_frontmatter: Some(yaml_block.to_string()),
    })
}

/// Rebuild a markdown document with the given envelope and stamp embedded
/// in frontmatter. The body is preserved byte-for-byte. If both records
/// are `None`, returns the body unchanged (no frontmatter block written).
pub fn embed_stamp(
    body: &str,
    envelope: Option<&Envelope>,
    stamp: Option<&Stamp>,
) -> Result<String, MarkdownError> {
    if envelope.is_none() && stamp.is_none() {
        return Ok(body.to_string());
    }

    let shape = FrontmatterShape {
        envelope: envelope.cloned(),
        attestation: stamp.cloned(),
    };
    let yaml = serde_yaml::to_string(&shape).map_err(|e| MarkdownError::YamlEmit(e.to_string()))?;

    // serde_yaml emits a trailing newline already; ensure exactly one.
    let yaml = yaml.trim_end_matches('\n');

    // Body is preserved byte-for-byte. The `parse → embed → parse` round-trip
    // must yield equal `body` strings, which means we cannot inject visual
    // whitespace here. If the user wants a blank line between frontmatter and
    // body, they include it as a leading `\n` in the body itself.
    Ok(format!("{DELIM}\n{yaml}\n{DELIM}\n{body}"))
}

// -- helpers ------------------------------------------------------------------

fn starts_with_delim(s: &str) -> bool {
    s.starts_with("---\n") || s.starts_with("---\r\n")
}

fn skip_delim_line(s: &str) -> Option<&str> {
    if let Some(rest) = s.strip_prefix("---\r\n") {
        return Some(rest);
    }
    s.strip_prefix("---\n")
}

/// Returns (yaml_between_delims, body_after_closing_delim).
fn split_at_closing_delim(s: &str) -> Option<(&str, &str)> {
    // The closing delimiter is a `---` line on its own. We look for `\n---\n`
    // or `\n---\r\n` or `\n---` at end of file.
    let mut search_start = 0usize;
    while let Some(rel) = s[search_start..].find("\n---") {
        let abs = search_start + rel;
        // Position immediately after `\n---`:
        let after_dashes = abs + 4;
        let tail = &s[after_dashes..];
        if let Some(after_lf) = tail.strip_prefix('\n') {
            return Some((&s[..abs], after_lf));
        }
        if let Some(after_crlf) = tail.strip_prefix("\r\n") {
            return Some((&s[..abs], after_crlf));
        }
        if tail.is_empty() {
            return Some((&s[..abs], ""));
        }
        // Otherwise `---` was prefix of something else (e.g. `----`); keep searching.
        search_start = abs + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        canonical_body_hash, AttestedDocument, Did, DocHash, EnvelopeBuilder, EnvelopeDepth,
        EnvelopeUrgency, Signature, StampAct,
    };
    use chrono::TimeZone;
    use chrono::Utc;

    fn rafa_did() -> Did {
        Did::parse("did:web:rafa.equanimi.tech").unwrap()
    }

    fn fixture_envelope() -> crate::domain::Envelope {
        EnvelopeBuilder::new(rafa_did())
            .to(Did::parse("did:web:marcelo.ballestiero.com").unwrap())
            .depth(EnvelopeDepth::Subtle)
            .urgency(EnvelopeUrgency::Soon)
            .source("claude-code-2026-04-30T14:22:00Z")
            .build()
    }

    fn fixture_stamp_for(hash: DocHash) -> Stamp {
        Stamp::new(
            rafa_did(),
            StampAct::Attest,
            hash,
            None,
            Utc.with_ymd_and_hms(2026, 4, 30, 14, 25, 0).unwrap(),
            Signature::from_bytes([0u8; 64]),
        )
    }

    #[test]
    fn parses_unsigned_doc() {
        let p = parse_document("# hello\n").unwrap();
        assert!(p.envelope.is_none());
        assert!(p.stamp.is_none());
        assert_eq!(p.body, "# hello\n");
    }

    #[test]
    fn parses_doc_with_only_envelope() {
        let env = fixture_envelope();
        let yaml = serde_yaml::to_string(&FrontmatterShape {
            envelope: Some(env.clone()),
            attestation: None,
        })
        .unwrap();
        let doc = format!("---\n{}---\n# hello\n", yaml);
        let p = parse_document(&doc).unwrap();
        assert_eq!(p.envelope, Some(env));
        assert!(p.stamp.is_none());
        assert_eq!(p.body, "# hello\n");
    }

    #[test]
    fn parses_doc_with_envelope_and_stamp() {
        let env = fixture_envelope();
        let body_text = "# hello\n";
        let stamp = fixture_stamp_for(canonical_body_hash(body_text));
        let combined = embed_stamp(body_text, Some(&env), Some(&stamp)).unwrap();

        let p = parse_document(&combined).unwrap();
        assert_eq!(p.envelope, Some(env.clone()));
        assert_eq!(p.stamp, Some(stamp.clone()));

        // The aggregate accepts the round-tripped pieces.
        let _ = AttestedDocument::new(p.envelope, p.stamp.unwrap(), p.body).unwrap();
    }

    #[test]
    fn rejects_unterminated_frontmatter() {
        let r = parse_document("---\nfoo: bar\nno closing delimiter\n");
        assert!(r.is_err());
    }

    #[test]
    fn parses_doc_with_bom() {
        let doc = "\u{FEFF}# hello\n";
        let p = parse_document(doc).unwrap();
        // BOM is stripped from the parsed body.
        assert_eq!(p.body, "# hello\n");
    }

    #[test]
    fn embed_then_parse_preserves_body_bytes() {
        let env = fixture_envelope();
        let body = "# Title\n\nParagraph with **bold** and a list:\n- one\n- two\n";
        let stamp = fixture_stamp_for(canonical_body_hash(body));
        let out = embed_stamp(body, Some(&env), Some(&stamp)).unwrap();
        let parsed = parse_document(&out).unwrap();
        assert_eq!(parsed.body, body);
        assert_eq!(parsed.envelope, Some(env));
        assert_eq!(parsed.stamp, Some(stamp));
    }

    #[test]
    fn embed_with_neither_record_returns_body_unchanged() {
        let body = "raw text\n";
        let out = embed_stamp(body, None, None).unwrap();
        assert_eq!(out, body);
    }
}
