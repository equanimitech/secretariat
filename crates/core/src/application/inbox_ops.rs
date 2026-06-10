//! Use case for reading (decrypting) a single envelope.
//!
//! Used by the MCP server (and `sec read` CLI) to surface an envelope's
//! body without each caller re-implementing frontmatter parsing +
//! sealed-box decryption.
//!
//! The cross-queue listing walkers (`list_inbox_files` /
//! `list_draft_files`) were removed in the git-native teardown (cut B):
//! the channel/draft surfaces that consumed them are gone. What remains
//! is the read/decrypt path — `read_envelope` reconstitutes the typed
//! [`Envelope`] on demand from the opaque `$envelope` frontmatter value.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::domain::{Did, Envelope, QueueHandle};
use crate::infrastructure::crypto::sealed::{open, signing_to_x25519, OpenError, SealedBox};
use crate::infrastructure::keys::{load_signing_key, KeyError};
use crate::infrastructure::markdown::{parse_document, MarkdownError};

/// Deserialize the opaque `$envelope` YAML value (carried by
/// [`crate::infrastructure::markdown::ParsedDocument`]) into the typed
/// [`Envelope`]. The markdown layer is envelope-schema-agnostic post
/// git-native teardown; the read/decrypt path reconstitutes the type here,
/// where it needs `from` / recipient / `encryption`.
fn typed_envelope(value: serde_yaml::Value) -> Result<Envelope, InboxOpError> {
    serde_yaml::from_value(value)
        .map_err(MarkdownError::from)
        .map_err(InboxOpError::from)
}

#[derive(Debug, Error)]
pub enum InboxOpError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("markdown parse: {0}")]
    Parse(#[from] MarkdownError),
    #[error("envelope frontmatter missing")]
    NoEnvelope,
    #[error("body is not a valid sealed-box wire string: {0}")]
    BadSealedWire(String),
    #[error("decryption failed: {0}")]
    Decryption(#[from] OpenError),
    #[error("key error: {0}")]
    Key(#[from] KeyError),
}

#[derive(Debug, Clone)]
pub struct ReadResult {
    pub body: String,
    pub envelope_from: Option<Did>,
    /// Owner DID of the recipient queue. None only if the envelope had
    /// no frontmatter (malformed file).
    pub envelope_to: Option<Did>,
    /// Handle of the recipient queue. None only if the envelope had no
    /// frontmatter.
    pub envelope_queue: Option<QueueHandle>,
    pub was_encrypted: bool,
}

/// Decrypt + return the body of an envelope file. Plaintext envelopes pass
/// through unchanged; encrypted envelopes load the local signing key,
/// derive the X25519 secret, and decrypt in-process.
pub fn read_envelope(
    file_path: &Path,
    signing_key_path: &Path,
) -> Result<ReadResult, InboxOpError> {
    let raw = std::fs::read_to_string(file_path).map_err(|e| InboxOpError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;
    let parsed = parse_document(&raw)?;

    // Unsigned / frontmatter-less docs: most working docs in the substrate
    // are plain markdown with no `$envelope` block. Treat them as a plaintext
    // passthrough — return the body with no envelope metadata rather than
    // erroring. The `ReadResult` Option fields already encode "no frontmatter".
    let Some(envelope_value) = parsed.envelope else {
        return Ok(ReadResult {
            body: parsed.body,
            envelope_from: None,
            envelope_to: None,
            envelope_queue: None,
            was_encrypted: false,
        });
    };
    let envelope = typed_envelope(envelope_value)?;

    let envelope_to = Some(envelope.recipient.owner.clone());
    let envelope_queue = Some(envelope.recipient.handle.clone());

    if envelope.is_encrypted() {
        let signing = load_signing_key(signing_key_path)?;
        let x25519_secret = signing_to_x25519(&signing);
        let sealed = SealedBox::parse_wire_string(parsed.body.trim())
            .map_err(|e| InboxOpError::BadSealedWire(e.to_string()))?;
        let plaintext = open(&sealed, &x25519_secret)?;
        Ok(ReadResult {
            body: String::from_utf8_lossy(&plaintext).into_owned(),
            envelope_from: Some(envelope.from),
            envelope_to,
            envelope_queue,
            was_encrypted: true,
        })
    } else {
        Ok(ReadResult {
            body: parsed.body,
            envelope_from: Some(envelope.from),
            envelope_to,
            envelope_queue,
            was_encrypted: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EnvelopeBuilder, Recipient};
    use crate::infrastructure::markdown::embed_stamp;
    use tempfile::TempDir;

    fn rafa_did() -> Did {
        Did::parse("did:web:rafa.equanimi.tech").unwrap()
    }

    fn self_recipient() -> Recipient {
        Recipient::new(rafa_did(), QueueHandle::parse("inbox:default").unwrap())
    }

    /// Self-addressed envelope as the opaque YAML value `embed_stamp` now
    /// takes (the markdown layer no longer knows the `Envelope` type).
    fn self_envelope_value() -> serde_yaml::Value {
        let env = EnvelopeBuilder::new(rafa_did(), self_recipient()).build();
        serde_yaml::to_value(&env).unwrap()
    }

    #[test]
    fn read_plaintext_envelope_returns_body() {
        let dir = TempDir::new().unwrap();
        let env = self_envelope_value();
        let body = "the body content\n";
        let content = embed_stamp(body, Some(&env), None).unwrap();
        let path = dir.path().join("envelope.md");
        std::fs::write(&path, content).unwrap();

        // We don't need a real signing key for plaintext.
        let result = read_envelope(&path, &dir.path().join("nonexistent-key")).unwrap();
        assert!(!result.was_encrypted);
        assert_eq!(result.body, body);
    }

    #[test]
    fn read_unsigned_doc_without_frontmatter_returns_body() {
        // Most working docs in the substrate are plain markdown with no
        // `$envelope` block. Reading one must pass the body through, not error.
        let dir = TempDir::new().unwrap();
        let body = "# A plain doc\n\nNo frontmatter, no signature.\n";
        let path = dir.path().join("plain.md");
        std::fs::write(&path, body).unwrap();

        let result = read_envelope(&path, &dir.path().join("nonexistent-key")).unwrap();
        assert!(!result.was_encrypted);
        assert_eq!(result.body, body);
        assert!(result.envelope_from.is_none());
        assert!(result.envelope_to.is_none());
        assert!(result.envelope_queue.is_none());
    }
}
