//! Use cases for listing and reading envelopes (inbox + outbox).
//!
//! Used by the MCP server (and `sec read` CLI) to surface envelope state
//! without each caller re-implementing directory walks + frontmatter parsing.

use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::domain::{Did, QueueHandle};
use crate::infrastructure::crypto::sealed::{open, signing_to_x25519, OpenError, SealedBox};
use crate::infrastructure::keys::{load_signing_key, KeyError};
use crate::infrastructure::markdown::{parse_document, MarkdownError};

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

#[derive(Debug, Clone, Serialize)]
pub struct ListedEnvelope {
    pub file_path: String,
    pub from: Option<String>,
    /// DID of the queue owner (recipient). Same field as `to` in older
    /// API revisions — under the queues-as-primitive collapse this is
    /// always the queue owner; consumers compute `is_local` by comparing
    /// to the principal's own DID.
    pub to: Option<String>,
    /// Queue handle (`<namespace>:<slug>`). Always present alongside
    /// `to`, since every envelope is addressed to a queue.
    pub queue: Option<String>,
    pub stamped: bool,
    pub encrypted: bool,
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

/// Walk a directory one level deep, listing `.md` files. For the outbox,
/// recurses one extra level into `<recipient-did>/` subdirs (but skips the
/// `sent/` subdirectory).
pub fn list_outbox_files(outbox_root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    let mut out = Vec::new();
    if !outbox_root.exists() {
        return Ok(out);
    }
    for entry in read_dir(outbox_root)? {
        let entry = io_entry(entry, outbox_root)?;
        let path = entry.path();
        if path.is_dir() {
            // recipient dir; collect .md files (skip nested `sent/`).
            for inner in read_dir(&path)? {
                let inner = io_entry(inner, &path)?;
                let inner_path = inner.path();
                if inner_path.is_file() && has_md_ext(&inner_path) {
                    push_envelope(&mut out, &inner_path)?;
                }
            }
        } else if has_md_ext(&path) {
            push_envelope(&mut out, &path)?;
        }
    }
    Ok(out)
}

pub fn list_inbox_files(inbox_root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    let mut out = Vec::new();
    if !inbox_root.exists() {
        return Ok(out);
    }
    for entry in read_dir(inbox_root)? {
        let entry = io_entry(entry, inbox_root)?;
        let path = entry.path();
        if path.is_file() && has_md_ext(&path) {
            push_envelope(&mut out, &path)?;
        }
    }
    Ok(out)
}

/// Decrypt + return the body of an envelope file. Plaintext envelopes pass
/// through unchanged; encrypted envelopes load the local signing key,
/// derive the X25519 secret, and decrypt in-process.
pub fn read_envelope(file_path: &Path, signing_key_path: &Path) -> Result<ReadResult, InboxOpError> {
    let raw = std::fs::read_to_string(file_path).map_err(|e| InboxOpError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;
    let parsed = parse_document(&raw)?;
    let envelope = parsed.envelope.ok_or(InboxOpError::NoEnvelope)?;

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_dir(p: &Path) -> Result<std::fs::ReadDir, InboxOpError> {
    std::fs::read_dir(p).map_err(|e| InboxOpError::Io {
        path: p.to_path_buf(),
        source: e,
    })
}

fn io_entry(
    entry: std::io::Result<std::fs::DirEntry>,
    parent: &Path,
) -> Result<std::fs::DirEntry, InboxOpError> {
    entry.map_err(|e| InboxOpError::Io {
        path: parent.to_path_buf(),
        source: e,
    })
}

fn has_md_ext(p: &Path) -> bool {
    p.extension().and_then(|x| x.to_str()) == Some("md")
}

fn push_envelope(out: &mut Vec<ListedEnvelope>, path: &Path) -> Result<(), InboxOpError> {
    let raw = std::fs::read_to_string(path).map_err(|e| InboxOpError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let parsed = parse_document(&raw)?;
    let (from, to, queue, encrypted) = match &parsed.envelope {
        Some(e) => (
            Some(e.from.as_str().to_string()),
            Some(e.recipient.owner.as_str().to_string()),
            Some(e.recipient.handle.as_str().to_string()),
            e.is_encrypted(),
        ),
        None => (None, None, None, false),
    };
    out.push(ListedEnvelope {
        file_path: path.display().to_string(),
        from,
        to,
        queue,
        stamped: parsed.stamp.is_some(),
        encrypted,
    });
    Ok(())
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

    #[test]
    fn list_inbox_returns_files() {
        let dir = TempDir::new().unwrap();
        let env =
            EnvelopeBuilder::new(rafa_did(), self_recipient()).build();
        let body = "hello\n";
        let content = embed_stamp(body, Some(&env), None).unwrap();
        std::fs::write(dir.path().join("a.md"), content).unwrap();
        std::fs::write(dir.path().join("not-md.txt"), "skip me").unwrap();

        let listed = list_inbox_files(dir.path()).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].from.as_deref(), Some("did:web:rafa.equanimi.tech"));
        assert!(!listed[0].stamped);
        assert!(!listed[0].encrypted);
    }

    #[test]
    fn list_inbox_handles_missing_dir() {
        let dir = TempDir::new().unwrap();
        let listed = list_inbox_files(&dir.path().join("does-not-exist")).unwrap();
        assert!(listed.is_empty());
    }

    #[test]
    fn read_plaintext_envelope_returns_body() {
        let dir = TempDir::new().unwrap();
        let env =
            EnvelopeBuilder::new(rafa_did(), self_recipient()).build();
        let body = "the body content\n";
        let content = embed_stamp(body, Some(&env), None).unwrap();
        let path = dir.path().join("envelope.md");
        std::fs::write(&path, content).unwrap();

        // We don't need a real signing key for plaintext.
        let result = read_envelope(&path, &dir.path().join("nonexistent-key")).unwrap();
        assert!(!result.was_encrypted);
        assert_eq!(result.body, body);
    }
}
