//! Markdown frontmatter parsing and embedding.
//!
//! Wire format (per the plan's "Stamped envelope file" spec):
//!
//! ```text
//! ---
//! $envelope:
//!   $type: tech.equanimi.secretariat.envelope
//!   ...
//! $attestation:
//!   $type: tech.equanimi.secretariat.stamp
//!   ...
//! ---
//!
//! # Body
//! ```
//!
//! The frontmatter block is delimited by `^---\n` ... `\n---\n`. Any of
//! `$envelope` / `$signature` / `$attestation` may be absent. The body
//! starts immediately after the closing `---\n`.
//!
//! Three-layer trust per AGENTS.md hard rule #4 (substrate-for-themia
//! Move 2, 2026-05-21):
//!   - `$signature` — author signature (typically scribe agent;
//!     optionally principal for manually-composed envelopes). Mandatory
//!     on post-Move-2 envelopes; optional in the parser for legacy
//!     back-compat.
//!   - `$attestation` — principal's Touch-ID-gated stamp. Selective.
//!
//! ## Encrypted-body convention
//!
//! When the envelope's `encryption` field is `Some(scheme)`, the body is no
//! longer plaintext markdown — it is the wire-string form of an encrypted
//! blob (see [`crate::infrastructure::crypto::sealed::SealedBox::to_wire_string`]).
//! This module is encryption-agnostic: it preserves whatever bytes the body
//! contains. The hash invariant (`docHash` over body) holds in both modes,
//! so the ed25519 signature authenticates the bytes that travel over the
//! transport (whether plaintext or ciphertext). Decryption is a separate
//! step performed after verification, on the recipient side, by the
//! application layer.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{Envelope, EnvelopeSignature, Stamp};

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
/// for round-trip diagnostics — round-tripping through `embed_frontmatter`
/// re-emits canonical YAML, not the original.
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub envelope: Option<Envelope>,
    /// Author signature (substrate-for-themia Move 2). Distinct from the
    /// principal's `stamp`; see module docs.
    pub signature: Option<EnvelopeSignature>,
    pub stamp: Option<Stamp>,
    pub body: String,
    pub raw_frontmatter: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct FrontmatterShape {
    #[serde(rename = "$envelope", default, skip_serializing_if = "Option::is_none")]
    envelope: Option<Envelope>,
    #[serde(rename = "$signature", default, skip_serializing_if = "Option::is_none")]
    signature: Option<EnvelopeSignature>,
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
            signature: None,
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
        signature: parsed.signature,
        stamp: parsed.attestation,
        body: after_close.to_string(),
        raw_frontmatter: Some(yaml_block.to_string()),
    })
}

/// Rebuild a markdown document with the given envelope, author signature,
/// and stamp embedded in frontmatter. The body is preserved byte-for-byte.
/// If all three records are `None`, returns the body unchanged (no
/// frontmatter block written).
///
/// Field emission order in the YAML: `$envelope`, `$signature`,
/// `$attestation`. The two cryptographic blocks are independent — a
/// signed-only envelope omits `$attestation`; a stamped envelope carries
/// both (the principal stamps an already-signed envelope, never replaces
/// the author's signature).
pub fn embed_frontmatter(
    body: &str,
    envelope: Option<&Envelope>,
    signature: Option<&EnvelopeSignature>,
    stamp: Option<&Stamp>,
) -> Result<String, MarkdownError> {
    if envelope.is_none() && signature.is_none() && stamp.is_none() {
        return Ok(body.to_string());
    }

    let shape = FrontmatterShape {
        envelope: envelope.cloned(),
        signature: signature.cloned(),
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

/// Back-compat shim for callers that only carry envelope + stamp. Equivalent
/// to `embed_frontmatter(body, envelope, None, stamp)`. New call sites
/// composing on the substrate-for-themia path SHOULD use
/// [`embed_frontmatter`] directly so the `$signature` layer is explicit.
pub fn embed_stamp(
    body: &str,
    envelope: Option<&Envelope>,
    stamp: Option<&Stamp>,
) -> Result<String, MarkdownError> {
    embed_frontmatter(body, envelope, None, stamp)
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
        EnvelopeUrgency, QueueHandle, Recipient, Signature, StampAct,
    };
    use chrono::TimeZone;
    use chrono::Utc;

    fn rafa_did() -> Did {
        Did::parse("did:web:rafa.equanimi.tech").unwrap()
    }

    fn fixture_envelope() -> crate::domain::Envelope {
        EnvelopeBuilder::new(
            rafa_did(),
            Recipient::new(
                Did::parse("did:web:marcelo.ballestiero.com").unwrap(),
                QueueHandle::parse("inbox:default").unwrap(),
            ),
        )
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
            signature: None,
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

    #[test]
    fn encrypted_body_round_trip_through_wire_format() {
        // Cross-layer test: sealed-box body + encrypted envelope marker +
        // markdown frontmatter wrapping. End-to-end the way the daemon will
        // build inbound/outbound envelopes once the application layer is wired.
        use crate::domain::{canonical_body_hash, EncryptionScheme};
        use crate::infrastructure::crypto::sealed::{
            open, pubkey_to_x25519, seal, signing_to_x25519, SealedBox,
        };
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        // 1. Recipient has a signing key; sender encrypts to its X25519 form.
        let recipient = SigningKey::generate(&mut OsRng);
        let recipient_pubkey = pubkey_to_x25519(&recipient.verifying_key());

        // 2. Plaintext message from the principal; daemon seals it.
        let plaintext = b"# ch7\n\nstaff vs. tools push-back\n";
        let sealed = seal(plaintext, &recipient_pubkey).unwrap();
        let body_wire = sealed.to_wire_string();

        // 3. Compose envelope with encryption marker.
        let envelope = EnvelopeBuilder::new(
            rafa_did(),
            Recipient::new(
                Did::parse("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK").unwrap(),
                QueueHandle::parse("inbox:default").unwrap(),
            ),
        )
        .encryption(EncryptionScheme::X25519XChaCha20Poly1305)
        .build();

        // 4. Stamp covers the wire-string body bytes.
        let body_hash = canonical_body_hash(&body_wire);
        let stamp = fixture_stamp_for(body_hash.clone());

        // 5. Embed → markdown document with encryption-marked envelope and
        //    SealedBox-wire-string body.
        let doc = embed_stamp(&body_wire, Some(&envelope), Some(&stamp)).unwrap();

        // 6. Recipient parses.
        let parsed = parse_document(&doc).unwrap();
        let parsed_envelope = parsed.envelope.expect("envelope must round-trip");
        assert!(parsed_envelope.is_encrypted());
        assert_eq!(
            parsed_envelope.encryption,
            Some(EncryptionScheme::X25519XChaCha20Poly1305)
        );

        // 7. Hash invariant holds: docHash matches the body bytes the
        //    recipient sees (the wire-string ciphertext).
        assert_eq!(canonical_body_hash(&parsed.body), body_hash);

        // 8. Recipient parses body as SealedBox and decrypts with its secret.
        let parsed_sealed = SealedBox::parse_wire_string(&parsed.body).unwrap();
        let recipient_secret = signing_to_x25519(&recipient);
        let opened = open(&parsed_sealed, &recipient_secret).unwrap();
        assert_eq!(opened, plaintext);
    }
}
