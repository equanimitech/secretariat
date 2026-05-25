//! Use case: stamp a markdown document.
//!
//! Reads the file, computes the canonical body hash, asks the [`Signer`] to
//! sign it (which gates on biometric), embeds the resulting [`Stamp`] in the
//! file's frontmatter, and writes the file back.
//!
//! Decision log #2: refuses if the file already has a stamp unless `force` is
//! true. Decision log #4: an `$envelope` is not required.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::domain::{
    canonical_body_hash, AttestedDocument, DocumentInvariantError, Stamp, StampAct,
};
use crate::infrastructure::markdown::{embed_frontmatter, parse_document, MarkdownError};
use crate::ports::{Signer, SignerError};

#[derive(Debug, Error)]
pub enum StampError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("markdown error: {0}")]
    Markdown(#[from] MarkdownError),
    #[error("signer error: {0}")]
    Signer(#[from] SignerError),
    #[error("file already has a stamp; pass --force to re-stamp")]
    AlreadyStamped,
    #[error("aggregate invariant failed after stamping (this is a bug): {0}")]
    Invariant(#[from] DocumentInvariantError),
}

#[derive(Debug, Clone)]
pub struct StampOutcome {
    pub stamped_path: PathBuf,
    pub stamp: Stamp,
}

pub fn stamp_document<S: Signer>(
    file_path: &Path,
    signer: &S,
    act: StampAct,
    force: bool,
    now: DateTime<Utc>,
) -> Result<StampOutcome, StampError> {
    let raw = fs::read_to_string(file_path).map_err(|e| StampError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;
    let parsed = parse_document(&raw)?;

    if parsed.stamp.is_some() && !force {
        return Err(StampError::AlreadyStamped);
    }

    let hash = canonical_body_hash(&parsed.body);
    let basename = file_path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());

    // Reason string surfaced in the Touch ID dialog. Includes the first-line
    // headline + a short hash prefix so the principal can spot a mismatch
    // between what they think they're stamping and what the file actually
    // contains. Defense against a compromised composer slipping different
    // bytes between display and sign.
    let headline = extract_headline(&parsed.body);
    let short_hash = canonical_short_hash(&hash);
    let reason = build_stamp_reason(basename.as_deref(), headline.as_deref(), &short_hash);
    let signature = signer.sign(&hash, &reason)?;

    let stamp = Stamp::new(
        signer.signer_did().clone(),
        act,
        hash,
        basename,
        now,
        signature,
    );

    // Validate invariant — should always pass since we just hashed.
    let _ = AttestedDocument::new(parsed.envelope.clone(), stamp.clone(), parsed.body.clone())?;

    // Preserve any existing author `$signature` (Move 2): stamping
    // attests to an already-signed envelope; it does not replace the
    // author's signature.
    let new_content = embed_frontmatter(
        &parsed.body,
        parsed.envelope.as_ref(),
        parsed.signature.as_ref(),
        Some(&stamp),
    )?;
    fs::write(file_path, new_content).map_err(|e| StampError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;

    Ok(StampOutcome {
        stamped_path: file_path.to_path_buf(),
        stamp,
    })
}

/// Pull the first non-empty body line, strip leading markdown heading marks,
/// trim, and cap at 80 chars. Returns `None` if the body has no usable line.
fn extract_headline(body: &str) -> Option<String> {
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let stripped = line.trim_start_matches('#').trim();
        if stripped.is_empty() {
            continue;
        }
        let mut out: String = stripped.chars().take(80).collect();
        if stripped.chars().count() > 80 {
            out.push('…');
        }
        return Some(out);
    }
    None
}

/// First 8 hex chars of the doc hash. Short enough to read aloud, long
/// enough that flipping it requires a near-collision (~2^32 work).
fn canonical_short_hash(hash: &crate::domain::DocHash) -> String {
    hex::encode(&hash.as_bytes()[..4])
}

fn build_stamp_reason(basename: Option<&str>, headline: Option<&str>, short_hash: &str) -> String {
    // macOS Touch ID dialogs render a single-line reason; keep it tight.
    // Format: `<headline> [<short_hash>] — <basename>`
    let head = headline.unwrap_or("Secretariat envelope");
    match basename {
        Some(b) => format!("{head} [{short_hash}] — {b}"),
        None => format!("{head} [{short_hash}]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        Did, EnvelopeBuilder, EnvelopeDepth, EnvelopeUrgency, QueueHandle, Recipient,
    };
    use crate::infrastructure::ed25519_signer::{AlwaysAllowGate, Ed25519Signer};
    use crate::infrastructure::keys::generate_keypair;
    use crate::infrastructure::markdown::embed_stamp as do_embed;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn make_signer() -> Ed25519Signer<AlwaysAllowGate> {
        Ed25519Signer::new(
            Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            generate_keypair(),
            AlwaysAllowGate,
        )
    }

    #[test]
    fn stamps_a_raw_markdown_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("doc.md");
        fs::write(&path, "# Hello\n\nworld\n").unwrap();

        let signer = make_signer();
        let now = Utc.with_ymd_and_hms(2026, 4, 30, 14, 25, 0).unwrap();
        let out = stamp_document(&path, &signer, StampAct::Attest, false, now).unwrap();

        let new = fs::read_to_string(&path).unwrap();
        assert!(new.starts_with("---\n"));
        assert!(new.contains("$attestation:"));

        let parsed = parse_document(&new).unwrap();
        assert!(parsed.stamp.is_some());
        assert_eq!(parsed.stamp.unwrap().act, StampAct::Attest);
        assert!(parsed.body.contains("# Hello"));
        let _ = out;
    }

    #[test]
    fn refuses_to_re_stamp_without_force() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("doc.md");

        // Pre-write a doc that already has a stamp.
        let signer = make_signer();
        let body = "# Hello\n";
        let hash = canonical_body_hash(body);
        let stamp = Stamp::new(
            signer.signer_did().clone(),
            StampAct::Attest,
            hash,
            None,
            Utc.with_ymd_and_hms(2026, 4, 30, 14, 0, 0).unwrap(),
            crate::domain::Signature::from_bytes([0u8; 64]),
        );
        let pre = do_embed(body, None, Some(&stamp)).unwrap();
        fs::write(&path, pre).unwrap();

        let r = stamp_document(
            &path,
            &signer,
            StampAct::Attest,
            false,
            Utc.with_ymd_and_hms(2026, 4, 30, 14, 25, 0).unwrap(),
        );
        assert!(matches!(r, Err(StampError::AlreadyStamped)));
    }

    #[test]
    fn force_replaces_existing_stamp() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("doc.md");
        fs::write(&path, "# Hello\n").unwrap();

        let signer = make_signer();
        let _ = stamp_document(
            &path,
            &signer,
            StampAct::Attest,
            false,
            Utc.with_ymd_and_hms(2026, 4, 30, 14, 25, 0).unwrap(),
        )
        .unwrap();

        let r = stamp_document(
            &path,
            &signer,
            StampAct::Attest,
            true,
            Utc.with_ymd_and_hms(2026, 4, 30, 15, 0, 0).unwrap(),
        );
        assert!(r.is_ok());
    }

    #[test]
    fn stamps_local_capture_envelope() {
        // Queues-as-primitive: stamps allowed on any envelope, including
        // self-addressed local captures. Tamper-evident self-attestation
        // is a valid use case (stamp your own journal entry, prove later
        // it hasn't been edited). Pre-collapse this would have been
        // forbidden by the `Recipient::LocalQueue` invariant.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("capture.md");

        let me = Did::parse("did:web:rafa.equanimi.tech").unwrap();
        let envelope = EnvelopeBuilder::new(
            me.clone(),
            Recipient::new(me.clone(), QueueHandle::parse("inbox:triage").unwrap()),
        )
        .depth(EnvelopeDepth::Gross)
        .urgency(EnvelopeUrgency::Whenever)
        .source("capture-test")
        .build();
        // Local capture: owner DID matches the principal's DID.
        assert_eq!(envelope.recipient.owner, me);

        let pre = do_embed("# Thought\n\nworth keeping\n", Some(&envelope), None).unwrap();
        fs::write(&path, pre).unwrap();

        let signer = make_signer();
        let outcome = stamp_document(
            &path,
            &signer,
            StampAct::Attest,
            false,
            Utc.with_ymd_and_hms(2026, 5, 5, 12, 0, 0).unwrap(),
        )
        .expect("stamp on local capture must succeed");

        let parsed = parse_document(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(parsed.stamp.is_some());
        assert_eq!(
            parsed.envelope.unwrap().recipient.handle.as_str(),
            "inbox:triage"
        );
        assert_eq!(&outcome.stamp.signer, signer.signer_did());
    }

    #[test]
    fn preserves_existing_envelope_block() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("doc.md");

        let envelope = EnvelopeBuilder::new(
            Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            Recipient::new(
                Did::parse("did:web:marcelo.ballestiero.com").unwrap(),
                QueueHandle::parse("inbox:default").unwrap(),
            ),
        )
        .depth(EnvelopeDepth::Subtle)
        .urgency(EnvelopeUrgency::Soon)
        .source("test")
        .build();
        let pre = do_embed("# Body\n", Some(&envelope), None).unwrap();
        fs::write(&path, pre).unwrap();

        let signer = make_signer();
        let _ = stamp_document(
            &path,
            &signer,
            StampAct::Attest,
            false,
            Utc.with_ymd_and_hms(2026, 4, 30, 14, 25, 0).unwrap(),
        )
        .unwrap();

        let parsed = parse_document(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(parsed.envelope, Some(envelope));
        assert!(parsed.stamp.is_some());
    }
}
