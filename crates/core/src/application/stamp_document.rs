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
use crate::infrastructure::markdown::{embed_stamp, parse_document, MarkdownError};
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

    let reason = match &basename {
        Some(b) => format!("Stamp Secretariat envelope: {b}"),
        None => "Stamp Secretariat envelope".to_string(),
    };
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
    let _ = AttestedDocument::new(
        parsed.envelope.clone(),
        stamp.clone(),
        parsed.body.clone(),
    )?;

    let new_content = embed_stamp(&parsed.body, parsed.envelope.as_ref(), Some(&stamp))?;
    fs::write(file_path, new_content).map_err(|e| StampError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;

    Ok(StampOutcome {
        stamped_path: file_path.to_path_buf(),
        stamp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Did, EnvelopeBuilder, EnvelopeDepth, EnvelopeUrgency};
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
    fn preserves_existing_envelope_block() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("doc.md");

        let envelope = EnvelopeBuilder::new(Did::parse("did:web:rafa.equanimi.tech").unwrap())
            .to(Did::parse("did:web:marcelo.ballestiero.com").unwrap())
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
