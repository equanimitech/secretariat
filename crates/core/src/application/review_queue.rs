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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const RAFA_DID: &str = "did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak";
    const MARCELO_DID: &str = "did:key:z6MkgyXNWdhXxW2xEEymYdRGCiohke8s8dskU1yW1TuGEddx";

    fn write_envelope(dir: &Path, name: &str, stamped: bool) {
        fs::create_dir_all(dir).unwrap();
        let stamp_block = if stamped {
            format!(
                "$attestation:\n  $type: tech.equanimi.secretariat.stamp\n  signer: {RAFA_DID}\n  act: attest\n  docHash: sha256:7d289c3de73f3dc1b0bd26f1e908bcdcc6b8e3242a33d478d356ce1cfb878547\n  docFilename: x.md\n  stampedAt: 2026-05-04T00:00:00Z\n  signature: ed25519:5t0ypQ0NmRzJrK0F9wKCkTwFCeSuhbxaZ7kpDfXOX3IZWDeCugRr8qpLQZ5B9MgK87uuz1PP6T8WOrNEEdLnCQ==\n"
            )
        } else {
            String::new()
        };
        let body = format!(
            "---\n$envelope:\n  $type: tech.equanimi.secretariat.envelope\n  from: {RAFA_DID}\n  to: {MARCELO_DID}\n  depth: subtle\n  urgency: soon\n  source: test\n{stamp_block}---\n# Hello\n"
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
