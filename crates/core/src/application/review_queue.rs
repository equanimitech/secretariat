//! Use case: list the principal's *review queue* — unstamped drafts
//! awaiting a stamp.
//!
//! Post-v0.9 (see `docs/pitches/2026-05-18-drop-outbox.md`) the substrate
//! holds:
//! 1. unstamped drafts under per-queue `_drafts/` (→ review queue),
//! 2. stamped envelopes — received OR self-authored awaiting send — under
//!    per-queue `envelopes/YYYY/MM/DD/`,
//! 3. delivered self-authored archive under per-queue `sent/YYYY/MM/DD/`
//!    (skipped by both inbox + drafts walkers).
//!
//! The Tauri review surface needs only category 1 — the principal's
//! chosen-time review session is about acting on UNSTAMPED drafts.

use std::path::Path;

use crate::application::inbox_ops::{list_draft_files, list_inbox_files, InboxOpError, ListedEnvelope};

/// Return the unstamped drafts on disk — the review queue.
///
/// Built on top of [`list_draft_files`]; kept as a distinct verb so the
/// UI's "what's awaiting stamp?" question reads at the presentation
/// boundary as a domain concept.
pub fn list_drafts_queue(root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    let all = list_draft_files(root)?;
    // Drafts are stamped exactly when the stamp ceremony has moved them
    // out of `_drafts/`. Defensive filter — a stamped file lingering in
    // `_drafts/` is a partial-rename artifact; surface only unstamped.
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

/// The principal's full review queue — unstamped drafts AND every
/// envelope addressed to any of their queues, in a single list. The
/// UI presents this as one stream; each entry is disambiguated by
/// inspecting its `file_path` and `to` / `queue` fields.
///
/// Takes the substrate root rather than separate roots, since both
/// shapes converge on `<root>/<alias>/channels/<segs>/{envelopes,_drafts}/`
/// in v0.9.
pub fn list_review_queue(root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    let mut out = list_drafts_queue(root)?;
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
        // v0.9 layout: drafts to a peer live at
        // `<peer-alias>/channels/inbox/default/_drafts/*.md` — recipient is
        // encoded in the path, no per-recipient subdir under `_drafts/`.
        let dir = TempDir::new().unwrap();
        let drafts = dir.path().join("did_key_z6Mkb/channels/inbox/default/_drafts");
        write_envelope(&drafts, "draft.md", false);
        // A stamped file lingering in `_drafts/` (partial-rename artifact)
        // must NOT surface in the review queue.
        write_envelope(&drafts, "stamped.md", true);

        let queue = list_drafts_queue(dir.path()).unwrap();
        assert_eq!(queue.len(), 1);
        assert!(queue[0].file_path.ends_with("draft.md"));
        assert!(!queue[0].stamped);
    }

    #[test]
    fn empty_drafts_returns_empty_queue() {
        let dir = TempDir::new().unwrap();
        let queue = list_drafts_queue(dir.path()).unwrap();
        assert!(queue.is_empty());
    }

    #[test]
    fn review_queue_unions_drafts_and_local_queues() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // One unstamped peer draft at <peer>/channels/inbox/default/_drafts/.
        write_envelope(
            &root.join("did_key_z6Mkb/channels/inbox/default/_drafts"),
            "draft.md",
            false,
        );

        // One local capture under _self/channels/inbox/triage/envelopes/.
        write_envelope(
            &root.join("_self/channels/inbox/triage/envelopes/2026/05/12"),
            "capture.md",
            false,
        );

        let unioned = list_review_queue(root).unwrap();
        // Both the draft and the captured envelope show up; the caller
        // discriminates peer-letter vs local-capture by inspecting the
        // `to` field against the principal's own DID.
        assert_eq!(unioned.len(), 2);

        assert!(unioned
            .iter()
            .any(|e| e.file_path.ends_with("draft.md")));
        assert!(unioned
            .iter()
            .any(|e| e.file_path.ends_with("capture.md")));
    }

    #[test]
    fn queue_skips_sent_subdirectory() {
        // v0.9 layout: `<peer>/channels/inbox/default/sent/` is the
        // daemon's post-delivery archive; it must never appear in the
        // review queue. The underlying walkers filter `sent` via
        // `should_skip`; this test guards the contract.
        let dir = TempDir::new().unwrap();
        let drafts = dir.path().join("did_key_z6Mkb/channels/inbox/default/_drafts");
        let sent = dir
            .path()
            .join("did_key_z6Mkb/channels/inbox/default/sent/2026/05/12");
        write_envelope(&drafts, "active.md", false);
        write_envelope(&sent, "historical.md", true);

        let queue = list_drafts_queue(dir.path()).unwrap();
        assert_eq!(queue.len(), 1);
        assert!(queue[0].file_path.ends_with("active.md"));
    }
}
