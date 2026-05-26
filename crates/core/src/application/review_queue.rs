//! Use case: list the principal's *review queue* — undelivered drafts
//! awaiting attention.
//!
//! Post-Move 4 (substrate-for-themia, see
//! `docs/pitches/2026-05-21-substrate-for-themia.md`) every envelope
//! lives at per-queue `envelopes/YYYY/MM/DD/<rkey>.md`. Delivery
//! state is the envelope frontmatter's `delivered:` field; drafts
//! are envelopes with the field absent. The review surface filters
//! the inbox walk for that condition.

use std::path::Path;

use crate::application::inbox_ops::{
    list_draft_files, list_inbox_files, InboxOpError, ListedEnvelope,
};

/// Return the undelivered drafts on disk — the review queue.
///
/// Built on top of [`list_draft_files`]; kept as a distinct verb so the
/// UI's "what's awaiting attention?" question reads at the presentation
/// boundary as a domain concept. Post-Move 4 (substrate-for-themia)
/// "draft" = envelope frontmatter lacks `delivered:`; whether the
/// envelope carries a `$attestation` block (stamp) is orthogonal —
/// a draft may be unstamped, stamped, or even counter-stamped.
pub fn list_drafts_queue(root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    list_draft_files(root)
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

/// The principal's full review queue — every envelope addressed to
/// any of their queues, in a single list. Post-Move 4 (substrate-
/// for-themia) drafts and federated envelopes share the
/// `envelopes/YYYY/MM/DD/` tree, so this is one walk; callers
/// disambiguate by the entry's `delivered` field (`None` =
/// draft / undelivered) and `stamped` flag.
pub fn list_review_queue(root: &Path) -> Result<Vec<ListedEnvelope>, InboxOpError> {
    list_local_queues(root)
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

    /// Helper. `delivered: Some(_)` writes the `delivered:` frontmatter
    /// field; `None` omits it (the substrate's "draft" signal).
    fn write_envelope(dir: &Path, name: &str, stamped: bool, delivered: Option<&str>) {
        fs::create_dir_all(dir).unwrap();
        let from = alice_did();
        let to = bob_did();
        let delivered_line = match delivered {
            Some(v) => format!("  delivered: {v}\n"),
            None => String::new(),
        };
        let stamp_block = if stamped {
            format!(
                "$attestation:\n  $type: tech.equanimi.secretariat.stamp\n  signer: {from}\n  act: attest\n  docHash: sha256:7d289c3de73f3dc1b0bd26f1e908bcdcc6b8e3242a33d478d356ce1cfb878547\n  docFilename: x.md\n  stampedAt: 2026-05-04T00:00:00Z\n  signature: ed25519:5t0ypQ0NmRzJrK0F9wKCkTwFCeSuhbxaZ7kpDfXOX3IZWDeCugRr8qpLQZ5B9MgK87uuz1PP6T8WOrNEEdLnCQ==\n"
            )
        } else {
            String::new()
        };
        let body = format!(
            "---\n$envelope:\n  $type: tech.equanimi.secretariat.envelope\n  from: {from}\n  to: {to}\n  handle: triage\n  source: test\n{delivered_line}{stamp_block}---\n# Hello\n"
        );
        fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn queue_excludes_delivered_envelopes() {
        // Post-Move 4: drafts and federated envelopes share
        // `<queue>/envelopes/YYYY/MM/DD/`. The review queue surfaces
        // only envelopes whose frontmatter lacks `delivered:`.
        let dir = TempDir::new().unwrap();
        let shard = dir
            .path()
            .join("did_key_z6Mkb/channels/inbox/default/envelopes/2026/05/21");
        // Undelivered (draft): no `delivered:` field.
        write_envelope(&shard, "draft.md", false, None);
        // Federated: daemon wrote `delivered: <relay-seq-id>`.
        write_envelope(&shard, "sent.md", true, Some("relay-seq-42"));

        let queue = list_drafts_queue(dir.path()).unwrap();
        assert_eq!(queue.len(), 1);
        assert!(queue[0].file_path.ends_with("draft.md"));
        assert!(queue[0].delivered.is_none());
    }

    #[test]
    fn queue_includes_stamped_but_undelivered_envelopes() {
        // A stamped envelope still awaiting federation IS a draft in
        // the Move-4 model — stamped ≠ delivered. The principal has
        // attested but the daemon hasn't pushed yet (offline, etc.).
        let dir = TempDir::new().unwrap();
        let shard = dir.path().join("channels/journal/envelopes/2026/05/21");
        write_envelope(&shard, "stamped-pending.md", true, None);

        let queue = list_drafts_queue(dir.path()).unwrap();
        assert_eq!(queue.len(), 1);
        assert!(queue[0].stamped);
        assert!(queue[0].delivered.is_none());
    }

    #[test]
    fn local_marker_counts_as_delivered() {
        // Self-owned channels under `~/.secretariat/channels/` never
        // federate; the substrate writes `delivered: local` in place.
        // Such envelopes are not drafts.
        let dir = TempDir::new().unwrap();
        let shard = dir.path().join("channels/journal/envelopes/2026/05/21");
        write_envelope(&shard, "local.md", false, Some("local"));

        let queue = list_drafts_queue(dir.path()).unwrap();
        assert!(queue.is_empty());
    }

    #[test]
    fn empty_drafts_returns_empty_queue() {
        let dir = TempDir::new().unwrap();
        let queue = list_drafts_queue(dir.path()).unwrap();
        assert!(queue.is_empty());
    }

    #[test]
    fn review_queue_unions_all_envelope_entries() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();

        // Org draft (undelivered) — non-self lives under `orgs/<alias>/`.
        write_envelope(
            &root.join("orgs/did_key_z6Mkb/channels/inbox/default/envelopes/2026/05/21"),
            "draft.md",
            false,
            None,
        );

        // Local capture (undelivered) — self channels sit at root.
        write_envelope(
            &root.join("channels/inbox/triage/envelopes/2026/05/12"),
            "capture.md",
            false,
            None,
        );

        // Already-delivered envelope under the same org tree.
        write_envelope(
            &root.join("orgs/did_key_z6Mkb/channels/inbox/default/envelopes/2026/05/21"),
            "delivered.md",
            true,
            Some("relay-seq-9"),
        );

        let unioned = list_review_queue(root).unwrap();
        // Post-Move 4 the review queue is the full envelope set; the
        // caller disambiguates draft / federated via `delivered`.
        assert_eq!(unioned.len(), 3);
        assert_eq!(unioned.iter().filter(|e| e.delivered.is_none()).count(), 2);
    }
}
