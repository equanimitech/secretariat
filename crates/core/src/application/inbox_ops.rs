//! Use cases for listing and reading envelopes (inbox + drafts).
//!
//! Used by the MCP server (and `sec read` CLI) to surface envelope state
//! without each caller re-implementing directory walks + frontmatter parsing.
//!
//! Substrate-for-themia Move 4 (per
//! `docs/pitches/2026-05-21-substrate-for-themia.md`): one envelope
//! state, one filesystem location. Every envelope — draft, stamped,
//! received, federated — lives under per-queue
//! `envelopes/YYYY/MM/DD/*.md`. The `_drafts/` and `sent/` subdirs
//! are gone. Draft state is signalled by the envelope frontmatter's
//! `delivered:` field: absent = draft / undelivered, set = federated
//! (or marked `local` for self-owned channels). Both
//! `list_inbox_files` and `list_draft_files` walk the same
//! `envelopes/` tree; `list_draft_files` filters by absence of the
//! `delivered:` frontmatter field.

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
    /// Delivery state marker from the envelope frontmatter. Absent
    /// (`None`) = draft / undelivered (the substrate's "this is awaiting
    /// federation" signal, post-Move 4). `Some("<relay-seq-id>")` =
    /// federated. `Some("local")` = self-owned channel that never
    /// federates.
    pub delivered: Option<String>,
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

/// Walk the substrate root and collect every `.md` file under any
/// `envelopes/` directory. The new (v0.3+) layout puts received
/// envelopes under `<root>/<alias>/<namespace>/<segments>/envelopes/
/// YYYY/MM/DD/*.md` — so the walker descends through alias dirs,
/// namespace + segment dirs, and the time-shard subtree.
///
/// Skips: principal-level files (anything that isn't a directory),
/// dotfile dirs, and the legacy `_unsorted/` bucket (which holds
/// inbound envelopes whose frontmatter couldn't be parsed for
/// routing — those need manual triage, not the regular listing).
///
/// `deferred/` and `archived/` siblings of `envelopes/` are NOT
/// walked — the principal moved those out of the active surface on
/// purpose. They live on disk for history.
pub fn list_inbox_files(root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    let mut out = Vec::new();
    walk_envelopes_tree(root, &mut out)?;
    Ok(out)
}

/// Walk the substrate root and collect every envelope whose frontmatter
/// lacks a `delivered:` field — the per-queue undelivered drafts the
/// AI scribe has composed but the daemon has not yet federated. Post-
/// Move 4 (substrate-for-themia) drafts share the `envelopes/`
/// day-shard tree with federated envelopes; the `delivered:` frontmatter
/// field is the sole disambiguator. The daemon writes that field
/// in-place after federation succeeds (or sets it to `local` at compose
/// time for self-owned channels that never federate).
pub fn list_draft_files(root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    let all = list_inbox_files(root)?;
    Ok(all.into_iter().filter(|e| e.delivered.is_none()).collect())
}

fn walk_envelopes_tree(
    dir: &Path,
    out: &mut Vec<ListedEnvelope>,
) -> Result<(), InboxOpError> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in read_dir(dir)? {
        let entry = io_entry(entry, dir)?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "envelopes" {
            // We're inside a queue's envelopes/ — descend through the
            // time-shard subtree and collect every .md leaf.
            walk_md_leaves(&path, out)?;
        } else if !should_skip(name) {
            walk_envelopes_tree(&path, out)?;
        }
    }
    Ok(())
}

fn walk_md_leaves(dir: &Path, out: &mut Vec<ListedEnvelope>) -> Result<(), InboxOpError> {
    for entry in read_dir(dir)? {
        let entry = io_entry(entry, dir)?;
        let path = entry.path();
        if path.is_dir() {
            walk_md_leaves(&path, out)?;
        } else if has_md_ext(&path) {
            push_envelope(out, &path)?;
        }
    }
    Ok(())
}

/// Skip dotfiles, ciphertext blobs, the legacy unsorted bucket, and
/// the principal's own deferred/archived buckets (those are
/// intentionally out-of-view).
fn should_skip(name: &str) -> bool {
    name.starts_with('.')
        || name == "_unsorted"
        || name == "_ciphertext"
        || name == "deferred"
        || name == "archived"
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
    let (from, to, queue, encrypted, delivered) = match &parsed.envelope {
        Some(e) => (
            Some(e.from.as_str().to_string()),
            Some(e.recipient.owner.as_str().to_string()),
            Some(e.recipient.handle.as_str().to_string()),
            e.is_encrypted(),
            e.delivered.clone(),
        ),
        None => (None, None, None, false, None),
    };
    out.push(ListedEnvelope {
        file_path: path.display().to_string(),
        from,
        to,
        queue,
        stamped: parsed.stamp.is_some(),
        encrypted,
        delivered,
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
    fn list_inbox_walks_alias_namespace_segments_tree() {
        let root = TempDir::new().unwrap();
        // New layout: <root>/channels/inbox/default/envelopes/2026/05/12/a.md
        let nested = root
            .path()
            .join("channels/inbox/default/envelopes/2026/05/12");
        std::fs::create_dir_all(&nested).unwrap();
        let env =
            EnvelopeBuilder::new(rafa_did(), self_recipient()).build();
        let body = "hello\n";
        let content = embed_stamp(body, Some(&env), None).unwrap();
        std::fs::write(nested.join("a.md"), content).unwrap();
        std::fs::write(nested.join("not-md.txt"), "skip me").unwrap();

        let listed = list_inbox_files(root.path()).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].from.as_deref(), Some("did:web:rafa.equanimi.tech"));
        assert!(!listed[0].stamped);
        assert!(!listed[0].encrypted);
    }

    #[test]
    fn list_inbox_skips_deferred_and_archived_siblings() {
        let root = TempDir::new().unwrap();
        let env =
            EnvelopeBuilder::new(rafa_did(), self_recipient()).build();
        let body = "active\n";
        let active_dir = root
            .path()
            .join("channels/inbox/default/envelopes/2026/05/12");
        let deferred_dir = root.path().join("channels/inbox/default/deferred");
        let archived_dir = root.path().join("channels/inbox/default/archived");
        for dir in [&active_dir, &deferred_dir, &archived_dir] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let content = embed_stamp(body, Some(&env), None).unwrap();
        std::fs::write(active_dir.join("a.md"), &content).unwrap();
        std::fs::write(deferred_dir.join("b.md"), &content).unwrap();
        std::fs::write(archived_dir.join("c.md"), &content).unwrap();

        let listed = list_inbox_files(root.path()).unwrap();
        // Only the active envelope appears; deferred/archived are
        // intentionally out-of-view per their semantics.
        assert_eq!(listed.len(), 1);
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
