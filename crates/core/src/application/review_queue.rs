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

use crate::application::inbox_ops::{list_inbox_files, list_outbox_files, InboxOpError, ListedEnvelope};

/// Return the subset of outbox files that are not yet stamped — the
/// drafts awaiting principal review.
///
/// Built on top of [`list_outbox_files`] rather than duplicating its
/// directory walk. This function exists as its own use case because
/// "review queue" is a first-class domain concept the UI surfaces
/// directly; the filter belongs in the application layer, not at the
/// presentation boundary.
pub fn list_outbox_queue(root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    let all = list_outbox_files(root)?;
    Ok(all.into_iter().filter(|e| !e.stamped).collect())
}

/// Walk the substrate tree and return every envelope under any
/// `envelopes/` directory. In v0.3 this is the same walk as
/// [`list_inbox_files`] — both surface "all envelopes the principal
/// has on disk" — but the function is kept as a distinct verb so the
/// review-surface caller's intent stays readable. A future filter
/// (e.g. `from == self_did` to isolate principal-authored captures
/// from received peer letters) can land here without disturbing
/// `list_inbox_files`.
pub fn list_local_queues(root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    list_inbox_files(root)
}

#[allow(dead_code)]
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

/// The principal's full review queue — unstamped outbox drafts AND
/// envelopes addressed to any of their queues, in a single list. The
/// UI presents this as one stream (substrate v0.3); each entry is
/// disambiguated by inspecting its `file_path` and `to` / `queue`
/// fields.
///
/// Takes the substrate root rather than separate outbox / queues
/// roots, since both shapes converge on `<root>/<alias>/<namespace>/
/// <segments>/{envelopes,outbox}/` in v0.3.
pub fn list_review_queue(root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    let mut out = list_outbox_queue(root)?;
    out.extend(list_local_queues(root)?);
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
        // v0.3 layout: drafts to a peer live at
        // `<peer-alias>/inbox/default/outbox/*.md` — recipient is
        // encoded in the path, no per-recipient subdir under `outbox/`.
        let dir = TempDir::new().unwrap();
        let outbox = dir.path().join("did_key_z6Mkb/inbox/default/outbox");
        write_envelope(&outbox, "draft.md", false);
        write_envelope(&outbox, "stamped.md", true);

        let queue = list_outbox_queue(dir.path()).unwrap();
        assert_eq!(queue.len(), 1);
        assert!(queue[0].file_path.ends_with("draft.md"));
        assert!(!queue[0].stamped);
    }

    #[test]
    fn empty_outbox_returns_empty_queue() {
        let dir = TempDir::new().unwrap();
        let queue = list_outbox_queue(dir.path()).unwrap();
        assert!(queue.is_empty());
    }

    #[test]
    fn review_queue_unions_outbox_and_local_queues() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // One unstamped peer draft at <peer>/inbox/default/outbox/.
        write_envelope(&root.join("did_key_z6Mkb/inbox/default/outbox"), "draft.md", false);

        // One local capture under _self/inbox/triage/envelopes/.
        write_envelope(
            &root.join("_self/inbox/triage/envelopes/2026/05/12"),
            "capture.md",
            false,
        );

        let unioned = list_review_queue(root).unwrap();
        // In v0.3 both the outbox draft and the captured envelope show
        // up; distinguishing peer-letter vs local-capture is done at
        // the call site by inspecting the `to` field against the
        // principal's own DID rather than two separate roots.
        assert_eq!(unioned.len(), 2);

        // Both entries are findable; discrimination by `to` is left
        // to the caller now (see substrate report on namespace
        // symmetry — the kind is encoded in the path, not in a
        // separate field).
        assert!(unioned
            .iter()
            .any(|e| e.file_path.ends_with("draft.md")));
        assert!(unioned
            .iter()
            .any(|e| e.file_path.ends_with("capture.md")));
    }

    #[test]
    fn queue_skips_sent_subdirectory() {
        // v0.3 layout: `<peer>/inbox/default/outbox/sent/` is the
        // daemon's post-delivery move target; it must never appear in
        // the review queue. The underlying list_outbox_files already
        // filters; this test guards the contract.
        let dir = TempDir::new().unwrap();
        let outbox = dir.path().join("did_key_z6Mkb/inbox/default/outbox");
        let sent = outbox.join("sent");
        write_envelope(&outbox, "active.md", false);
        write_envelope(&sent, "historical.md", true);

        let queue = list_outbox_queue(dir.path()).unwrap();
        assert_eq!(queue.len(), 1);
        assert!(queue[0].file_path.ends_with("active.md"));
    }
}
