//! Resolver for `(Recipient) → on-disk queue directory`.
//!
//! Every envelope is addressed to a queue identified by `(to, handle)`.
//! This module decides where that queue lives on disk. Substrate-for-
//! themia Move 3c (`docs/pitches/2026-05-21-substrate-for-themia.md`,
//! element §2) collapses the previous three-segment layout
//! (`<alias>/channels/<segs>/`) into two channel-tree roots:
//!
//! ```text
//! ~/.secretariat/
//!   channels/<segments>/                     self-owned (recipient.owner == self_did)
//!     envelopes/YYYY/MM/DD/*.md
//!     _ciphertext/
//!   orgs/<alias>/channels/<segments>/        org-scoped (recipient.owner == org_did)
//!     envelopes/YYYY/MM/DD/*.md
//!     _ciphertext/
//! ```
//!
//! There is no `_self/` wrapper anymore; the principal's own channels
//! live straight under `<root>/channels/`. Org channels are wrapped by
//! `orgs/<alias>/`.
//!
//! Per Move 4: there is one envelope state and one filesystem location.
//! Every envelope — draft, stamped, received, federated — lives at
//! `<queue>/envelopes/YYYY/MM/DD/<rkey>.md`. Delivery state is the
//! envelope frontmatter's `delivered:` field.
//!
//! Examples:
//!
//! - Local journal capture (`to == self_did`, `handle == journal`)
//!   → `<root>/channels/journal/`
//! - Themia channel (`to == themia_did`, `handle == dommage-corporel:paris-cohort`)
//!   → `<root>/orgs/themia.pro/channels/dommage-corporel/paris-cohort/`
//!
//! # Aliases
//!
//! Aliases are filesystem ergonomics for non-self DIDs — short,
//! human-readable directory names that map to canonical org DIDs.
//! The canonical address is always the DID; the alias is just a
//! friendlier directory name on disk.
//!
//! - Org aliases: `OrgAlias` (DNS-label-shaped, e.g. `themia.pro`).
//! - Unknown DIDs: sanitized DID fallback under `orgs/` (still wrapped).
//!
//! The mapping is principal-local; never published. Aliases can change
//! without invalidating envelopes — the wire address stays
//! `(to-DID, handle)`.

use crate::domain::{Did, Recipient};
use crate::infrastructure::keys::KeyPaths;
use crate::infrastructure::org_store::{list_org_dirs, OrgStoreError};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Principal-local mapping from DID → alias-on-disk. Build with
/// [`AliasMap::new`] then `insert` for each known peer/org, or use
/// [`AliasMap::load`] (in `application::queue_paths`, once wired) to
/// populate from the contact book + org store.
#[derive(Debug, Clone)]
pub struct AliasMap {
    self_did: Did,
    by_did: BTreeMap<String, String>,
}

impl AliasMap {
    pub fn new(self_did: Did) -> Self {
        Self {
            self_did,
            by_did: BTreeMap::new(),
        }
    }

    /// Register an alias for a non-self DID. Last write wins —
    /// callers are responsible for not registering conflicts. (Org
    /// alias validation, contact-slug uniqueness, etc. happen at
    /// their respective layers; this map is the merged view.)
    pub fn insert(&mut self, did: Did, alias: impl Into<String>) {
        self.by_did.insert(did.as_str().to_string(), alias.into());
    }

    /// Build the alias map from the org directory.
    /// Orgs contribute `OrgAlias.as_str() → did` (skipping orgs that
    /// don't yet have a canonical DID, since those can't be wire-
    /// addressed). The contact-book contribution was removed in the
    /// substrate-for-themia slice (Move 3b) — peer aliases are gone.
    /// Missing files are tolerated as empty — a fresh install with no
    /// orgs still produces a valid (empty) map.
    pub fn load(self_did: Did, paths: &KeyPaths) -> Result<Self, AliasMapError> {
        let mut map = Self::new(self_did);

        if paths.orgs_root.exists() {
            for org in list_org_dirs(&paths.orgs_root)? {
                if let Some(did) = org.did {
                    map.insert(did, org.alias.as_str().to_string());
                }
            }
        }

        Ok(map)
    }

    /// True when the given DID is the principal's own. Used by
    /// [`queue_dir`] to decide which root to anchor against (self
    /// channels live at `<root>/channels/`; org channels at
    /// `<root>/orgs/<alias>/channels/`).
    pub fn is_self(&self, did: &Did) -> bool {
        did == &self.self_did
    }

    /// Resolve a non-self DID to its registered alias, or fall back
    /// to a sanitized DID. The result is always intended for use
    /// under `<root>/orgs/`; callers must NOT pass the principal's
    /// own DID — use [`AliasMap::is_self`] to branch upstream.
    pub fn alias_for(&self, did: &Did) -> String {
        if let Some(alias) = self.by_did.get(did.as_str()) {
            return alias.clone();
        }
        sanitize_did_fallback(did.as_str())
    }
}

fn sanitize_did_fallback(did: &str) -> String {
    did.replace([':', '/'], "_")
}

#[derive(Debug, Error)]
pub enum AliasMapError {
    #[error("org store: {0}")]
    Orgs(#[from] OrgStoreError),
}

/// Compute the on-disk queue directory for a `Recipient` under the
/// principal's substrate root. The result is the directory that
/// *contains* `envelopes/` and `_ciphertext/` for this queue.
///
/// Two channel-tree roots (Move 3c — substrate-for-themia, element §2):
///
/// - `recipient.owner == self_did` → `<root>/channels/<segs>/`
/// - else (org) → `<root>/orgs/<alias>/channels/<segs>/`
///
/// Pure compute — no IO. Path is not guaranteed to exist on disk;
/// callers that need it materialized should `create_dir_all` against
/// the result (or against [`envelopes_dir`] / [`ciphertext_dir`]
/// which compose on top).
pub fn queue_dir(aliases: &AliasMap, recipient: &Recipient, root: &Path) -> PathBuf {
    let mut dir = if aliases.is_self(&recipient.owner) {
        root.join("channels")
    } else {
        let alias = aliases.alias_for(&recipient.owner);
        root.join("orgs").join(alias).join("channels")
    };
    for seg in recipient.handle.segments() {
        dir.push(seg);
    }
    dir
}

/// `<queue-dir>/envelopes/` — every envelope addressed to this queue,
/// time-sharded by the caller via `<year>/<month>/<day>/` sub-paths.
/// Drafts and federated envelopes share this tree; delivery state is
/// the envelope frontmatter's `delivered:` field (absent = draft).
pub fn envelopes_dir(aliases: &AliasMap, recipient: &Recipient, root: &Path) -> PathBuf {
    queue_dir(aliases, recipient, root).join("envelopes")
}

/// `<queue-dir>/_ciphertext/` — encrypted-at-rest blobs (what crosses
/// transports). Per-recipient encryption stays here; plaintext lives
/// only in `envelopes/`.
pub fn ciphertext_dir(aliases: &AliasMap, recipient: &Recipient, root: &Path) -> PathBuf {
    queue_dir(aliases, recipient, root).join("_ciphertext")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::QueueHandle;

    fn alice() -> Did {
        Did::from_ed25519_public_key(&[0xa1; 32])
    }

    fn bob() -> Did {
        Did::from_ed25519_public_key(&[0xb0; 32])
    }

    fn themia() -> Did {
        Did::from_ed25519_public_key(&[0x7e; 32])
    }

    fn root() -> &'static Path {
        Path::new("/var/secretariat")
    }

    #[test]
    fn self_channel_lives_directly_under_root_channels() {
        let aliases = AliasMap::new(alice());
        let recipient = Recipient::new(alice(), QueueHandle::parse("journal").unwrap());
        assert_eq!(
            queue_dir(&aliases, &recipient, root()),
            PathBuf::from("/var/secretariat/channels/journal"),
        );
    }

    #[test]
    fn known_org_resolves_under_orgs_root() {
        let mut aliases = AliasMap::new(alice());
        aliases.insert(themia(), "themia.pro");
        let recipient = Recipient::new(themia(), QueueHandle::parse("assemblee_generale").unwrap());
        assert_eq!(
            queue_dir(&aliases, &recipient, root()),
            PathBuf::from("/var/secretariat/orgs/themia.pro/channels/assemblee_generale"),
        );
    }

    #[test]
    fn unknown_org_falls_back_to_sanitized_did_under_orgs() {
        let aliases = AliasMap::new(alice());
        let recipient = Recipient::new(bob(), QueueHandle::parse("inbox").unwrap());
        let path = queue_dir(&aliases, &recipient, root());
        // Should sit under `<root>/orgs/<sanitized-did>/channels/inbox`.
        let stripped = path.strip_prefix(root()).unwrap();
        let mut comps = stripped.components();
        assert_eq!(comps.next().unwrap().as_os_str(), "orgs");
        let alias_seg = comps
            .next()
            .unwrap()
            .as_os_str()
            .to_string_lossy()
            .into_owned();
        assert!(alias_seg.starts_with("did_key_"), "got: {alias_seg}");
        assert!(!alias_seg.contains(':'));
        assert!(!alias_seg.contains('/'));
        assert_eq!(comps.next().unwrap().as_os_str(), "channels");
        assert_eq!(comps.next().unwrap().as_os_str(), "inbox");
    }

    #[test]
    fn nested_org_channel_handle_becomes_nested_dirs() {
        let mut aliases = AliasMap::new(alice());
        aliases.insert(themia(), "themia.pro");
        let recipient = Recipient::new(
            themia(),
            QueueHandle::parse("dommage-corporel:paris-cohort").unwrap(),
        );
        assert_eq!(
            queue_dir(&aliases, &recipient, root()),
            PathBuf::from("/var/secretariat/orgs/themia.pro/channels/dommage-corporel/paris-cohort"),
        );
    }

    #[test]
    fn self_sub_paths_compose_correctly() {
        let aliases = AliasMap::new(alice());
        let recipient = Recipient::new(alice(), QueueHandle::parse("writing").unwrap());
        let base = root();
        assert_eq!(
            envelopes_dir(&aliases, &recipient, base),
            PathBuf::from("/var/secretariat/channels/writing/envelopes"),
        );
        assert_eq!(
            ciphertext_dir(&aliases, &recipient, base),
            PathBuf::from("/var/secretariat/channels/writing/_ciphertext"),
        );
    }

    #[test]
    fn nested_self_channel_handle_becomes_nested_dirs() {
        let aliases = AliasMap::new(alice());
        let recipient = Recipient::new(
            alice(),
            QueueHandle::parse("articles:equanimitech").unwrap(),
        );
        assert_eq!(
            queue_dir(&aliases, &recipient, root()),
            PathBuf::from("/var/secretariat/channels/articles/equanimitech"),
        );
    }
}
