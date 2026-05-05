//! Use case: list the principal's *review queue* — outbox drafts awaiting
//! a stamp.
//!
//! The review queue is a domain concept distinct from "the outbox files."
//! The outbox holds three things at any moment:
//! 1. drafts the AI assistant has composed but the principal has not yet
//!    reviewed (→ review queue),
//! 2. drafts the principal has stamped but the daemon has not yet sent
//!    (→ in-flight),
//! 3. and a `sent/` subdirectory of historical successes
//!    (→ already handled by the existing list_outbox_files filter).
//!
//! The Tauri review surface (see
//! `docs/milestones/2026-05-04-tauri-front-door.md` slice 3) needs only
//! the first category — the principal's chosen-time review session is
//! about acting on UNSTAMPED drafts.

use std::path::Path;

use crate::application::inbox_ops::{list_outbox_files, InboxOpError, ListedEnvelope};

/// Return the subset of outbox files that are not yet stamped — the
/// drafts awaiting principal review.
///
/// Built on top of [`list_outbox_files`] rather than duplicating its
/// directory walk. This function exists as its own use case because
/// "review queue" is a first-class domain concept the UI surfaces
/// directly; the filter belongs in the application layer, not at the
/// presentation boundary.
pub fn list_outbox_queue(outbox_root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    let all = list_outbox_files(outbox_root)?;
    Ok(all.into_iter().filter(|e| !e.stamped).collect())
}

/// Walk the local-queues tree (`<queues_root>/<namespace>/<slug>/*.md`)
/// and return one [`ListedEnvelope`] per capture. Local-queue captures
/// are never stamped by invariant, so there is no `stamped` filter to
/// apply here — every entry is, by definition, a draft.
pub fn list_local_queues(queues_root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    let mut out = Vec::new();
    if !queues_root.exists() {
        return Ok(out);
    }
    walk_queues(queues_root, &mut out)?;
    Ok(out)
}

fn walk_queues(dir: &Path, out: &mut Vec<ListedEnvelope>) -> Result<(), InboxOpError> {
    for entry in std::fs::read_dir(dir).map_err(|e| InboxOpError::Io {
        path: dir.to_path_buf(),
        source: e,
    })? {
        let entry = entry.map_err(|e| InboxOpError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if path.is_dir() {
            walk_queues(&path, out)?;
        } else if path.extension().and_then(|x| x.to_str()) == Some("md") {
            push_listed(out, &path)?;
        }
    }
    Ok(())
}

fn push_listed(out: &mut Vec<ListedEnvelope>, path: &Path) -> Result<(), InboxOpError> {
    use crate::infrastructure::markdown::parse_document;

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

/// The principal's full review queue — outbox drafts AND local-queue
/// captures, in a single list. The UI presents this as one stream
/// (substrate v0.3); the kind discriminator is `to.is_some()` vs
/// `queue.is_some()` on each entry.
pub fn list_review_queue(
    outbox_root: &Path,
    queues_root: &Path,
) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    let mut out = list_outbox_queue(outbox_root)?;
    out.extend(list_local_queues(queues_root)?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Synthetic DIDs for tests — derived from deterministic seed bytes
    /// so we never embed a real principal's DID in source. See
    /// `memory/feedback_no_real_dids_in_tests.md`.
    fn alice_did() -> String {
        crate::Did::from_ed25519_public_key(&[0xa1; 32])
            .as_str()
            .to_string()
    }
    fn bob_did() -> String {
        crate::Did::from_ed25519_public_key(&[0xb0; 32])
            .as_str()
            .to_string()
    }

    fn write_envelope(dir: &Path, name: &str, stamped: bool) {
        fs::create_dir_all(dir).unwrap();
        let from = alice_did();
        let to = bob_did();
        let stamp_block = if stamped {
            format!(
                "$attestation:\n  $type: tech.equanimi.secretariat.stamp\n  signer: {from}\n  act: attest\n  docHash: sha256:7d289c3de73f3dc1b0bd26f1e908bcdcc6b8e3242a33d478d356ce1cfb878547\n  docFilename: x.md\n  stampedAt: 2026-05-04T00:00:00Z\n  signature: ed25519:5t0ypQ0NmRzJrK0F9wKCkTwFCeSuhbxaZ7kpDfXOX3IZWDeCugRr8qpLQZ5B9MgK87uuz1PP6T8WOrNEEdLnCQ==\n"
            )
        } else {
            String::new()
        };
        let body = format!(
            "---\n$envelope:\n  $type: tech.equanimi.secretariat.envelope\n  from: {from}\n  to: {to}\n  depth: subtle\n  urgency: soon\n  source: test\n{stamp_block}---\n# Hello\n"
        );
        fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn queue_excludes_stamped_drafts() {
        let dir = TempDir::new().unwrap();
        let outbox = dir.path().join("outbox");
        let recipient = outbox.join("did_key_z6Mkb");
        write_envelope(&recipient, "draft.md", false);
        write_envelope(&recipient, "stamped.md", true);

        let queue = list_outbox_queue(&outbox).unwrap();
        assert_eq!(queue.len(), 1);
        assert!(queue[0].file_path.ends_with("draft.md"));
        assert!(!queue[0].stamped);
    }

    #[test]
    fn empty_outbox_returns_empty_queue() {
        let dir = TempDir::new().unwrap();
        let queue = list_outbox_queue(&dir.path().join("outbox")).unwrap();
        assert!(queue.is_empty());
    }

    #[test]
    fn review_queue_unions_outbox_and_local_queues() {
        use crate::application::{capture_to_queue, CaptureRequest};
        use crate::domain::QueueHandle;

        let dir = TempDir::new().unwrap();
        let outbox = dir.path().join("outbox");
        let queues = dir.path().join("queues");

        // One unstamped peer draft in the outbox.
        write_envelope(&outbox.join("did_key_z6Mkb"), "draft.md", false);

        // One local-queue capture.
        let req = CaptureRequest {
            from: crate::Did::from_ed25519_public_key(&[0xa1; 32]),
            queue: QueueHandle::parse("inbox:triage").unwrap(),
            body: "fleeting thought".into(),
            source: "test".into(),
        };
        capture_to_queue(req, &queues, chrono::Utc::now()).unwrap();

        let unioned = list_review_queue(&outbox, &queues).unwrap();
        assert_eq!(unioned.len(), 2);

        // Both entries now have to + queue populated. Discriminate by
        // owner: peer letters have `to != self`, captures have
        // `to == self`.
        let me_str = crate::Did::from_ed25519_public_key(&[0xa1; 32])
            .as_str()
            .to_string();
        let peers: Vec<_> = unioned
            .iter()
            .filter(|e| e.to.as_deref() != Some(&me_str))
            .collect();
        let captures: Vec<_> = unioned
            .iter()
            .filter(|e| e.to.as_deref() == Some(&me_str))
            .collect();
        assert_eq!(peers.len(), 1);
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].queue.as_deref(), Some("inbox:triage"));
    }

    #[test]
    fn queue_skips_sent_subdirectory() {
        // Files in `outbox/<recipient>/sent/` should never appear in the
        // queue. The underlying list_outbox_files already does this; the
        // queue function inherits it.
        let dir = TempDir::new().unwrap();
        let outbox = dir.path().join("outbox");
        let recipient = outbox.join("did_key_z6Mkb");
        let sent = recipient.join("sent");
        write_envelope(&recipient, "active.md", false);
        write_envelope(&sent, "historical.md", true);

        let queue = list_outbox_queue(&outbox).unwrap();
        assert_eq!(queue.len(), 1);
        assert!(queue[0].file_path.ends_with("active.md"));
    }
}
