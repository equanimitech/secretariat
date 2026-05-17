//! Resolver for `(Recipient) → on-disk queue directory`.
//!
//! Every envelope is addressed to a queue identified by `(to, handle)`
//! per the v0.3 substrate. This module decides where that queue lives
//! on disk. The layout is uniform across namespaces (`inbox`, `area`,
//! `channel`) and across queue-owner kinds (self, peer, org):
//!
//! ```text
//! ~/.secretariat/
//!   <alias-of-to>/<namespace>/<segments>/
//!     envelopes/YYYY/MM/DD/*.md      (decrypted, AI/grep-able)
//!     outbox/*.md                     (drafts; daemon ferries)
//!     _ciphertext/                    (encrypted-at-rest blobs)
//! ```
//!
//! Examples:
//!
//! - DM to Marcelo (`to == marcelo_did`, `handle == inbox:default`)
//!   → `marcelo/inbox/default/`
//! - Local triage capture (`to == self_did`, `handle == inbox:triage`)
//!   → `_self/inbox/triage/`
//! - Writing area (`to == self_did`, `handle == area:writing`)
//!   → `_self/area/writing/`
//! - Themia channel (`to == themia_did`, `handle == channel:dommage-corporel:paris-cohort`)
//!   → `themia.pro/channel/dommage-corporel/paris-cohort/`
//!
//! See [[project_namespace_symmetry]] for the design rationale — the
//! tldr is that `inbox:`, `area:`, `channel:` are sibling *namespaces*
//! in the handle grammar (same primitive, different publishability
//! semantics), so they share storage layout.
//!
//! # Aliases
//!
//! Aliases are filesystem ergonomics — short, human-readable directory
//! names that map to canonical DIDs. The canonical address is always
//! the DID; the alias is just a friendlier directory name on disk.
//!
//! - `_self` for the principal's own DID (always reserved).
//! - Org aliases: `OrgAlias` (DNS-label-shaped, e.g. `themia.pro`).
//! - Peer aliases: contact display-name slug (`marcelo`,
//!   `christophe`) for known peers; sanitized DID fallback for
//!   unknown peers (`did_key_z6mk…`).
//!
//! The mapping is principal-local; never published. Aliases can change
//! (rename a contact) without invalidating envelopes — the wire
//! address stays `(to-DID, handle)`.

use crate::domain::{Did, Recipient};
use crate::infrastructure::contact_store::{ContactBook, ContactStoreError};
use crate::infrastructure::keys::KeyPaths;
use crate::infrastructure::org_store::{list_org_dirs, OrgStoreError};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Reserved alias for the principal's own DID.
pub const SELF_ALIAS: &str = "_self";

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

    /// Build the alias map from the principal's contact book and org
    /// directory. Contacts contribute `display_name.slug() → did`;
    /// orgs contribute `OrgAlias.as_str() → did` (skipping orgs that
    /// don't yet have a canonical DID, since those can't be wire-
    /// addressed). Missing files are tolerated as empty — a fresh
    /// install with no contacts/orgs still produces a valid (empty)
    /// map.
    pub fn load(self_did: Did, paths: &KeyPaths) -> Result<Self, AliasMapError> {
        let mut map = Self::new(self_did);

        let book = ContactBook::load(&paths.contacts)?;
        for contact in book.iter() {
            map.insert(contact.did.clone(), contact.display_name.slug());
        }

        if paths.orgs_root.exists() {
            for org in list_org_dirs(&paths.orgs_root)? {
                if let Some(did) = org.did {
                    map.insert(did, org.alias.as_str().to_string());
                }
            }
        }

        Ok(map)
    }

    /// Resolve a DID to its on-disk alias. Self always returns
    /// `_self`; known peers/orgs return their registered alias;
    /// unknown DIDs get a sanitized fallback that's safe across
    /// filesystems but ugly (matches the `did:key:z6Mk…` form with
    /// `:` and `/` replaced).
    pub fn alias_for(&self, did: &Did) -> String {
        if did == &self.self_did {
            return SELF_ALIAS.to_string();
        }
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
    #[error("contact store: {0}")]
    Contacts(#[from] ContactStoreError),
    #[error("org store: {0}")]
    Orgs(#[from] OrgStoreError),
}

/// Compute the on-disk queue directory for a `Recipient` under the
/// principal's substrate root. The result is the directory that
/// *contains* `envelopes/`, `outbox/`, `_ciphertext/` for this queue.
///
/// Path shape (v0.7+ — queue_dir alignment slice): `<root>/<alias>/channels/<segs>/`.
/// Three kinds of `<alias>` — `_self`, `<org-alias>`, `<peer-alias>` —
/// all sit at the same depth with `channels/` between alias and
/// handle segments. Per [[project_contracts_attach_to_queues]] a DM
/// is a 2-roster channel; cardinality changes shape, not primitive.
///
/// Pure compute — no IO. Path is not guaranteed to exist on disk;
/// callers that need it materialized should `create_dir_all` against
/// the result (or against [`envelopes_dir`] / [`outbox_dir`] /
/// [`ciphertext_dir`] which compose on top).
pub fn queue_dir(aliases: &AliasMap, recipient: &Recipient, root: &Path) -> PathBuf {
    let alias = aliases.alias_for(&recipient.owner);
    let mut dir = root.join(alias);
    dir.push("channels");
    for seg in recipient.handle.segments() {
        dir.push(seg);
    }
    dir
}

/// `<queue-dir>/envelopes/` — decrypted markdown, time-sharded by the
/// caller via `<year>/<month>/<day>/` sub-paths.
pub fn envelopes_dir(aliases: &AliasMap, recipient: &Recipient, root: &Path) -> PathBuf {
    queue_dir(aliases, recipient, root).join("envelopes")
}

/// `<queue-dir>/outbox/` — drafts the daemon picks up to sign+send.
pub fn outbox_dir(aliases: &AliasMap, recipient: &Recipient, root: &Path) -> PathBuf {
    queue_dir(aliases, recipient, root).join("outbox")
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
    fn self_resolves_to_self_alias() {
        let aliases = AliasMap::new(alice());
        let recipient = Recipient::new(alice(), QueueHandle::parse("triage").unwrap());
        assert_eq!(
            queue_dir(&aliases, &recipient, root()),
            PathBuf::from("/var/secretariat/_self/channels/triage"),
        );
    }

    #[test]
    fn known_peer_resolves_to_registered_alias() {
        let mut aliases = AliasMap::new(alice());
        aliases.insert(bob(), "marcelo");
        let recipient = Recipient::new(bob(), QueueHandle::parse("inbox").unwrap());
        assert_eq!(
            queue_dir(&aliases, &recipient, root()),
            PathBuf::from("/var/secretariat/marcelo/channels/inbox"),
        );
    }

    #[test]
    fn unknown_peer_falls_back_to_sanitized_did() {
        let aliases = AliasMap::new(alice());
        let recipient = Recipient::new(bob(), QueueHandle::parse("inbox").unwrap());
        let path = queue_dir(&aliases, &recipient, root());
        // The fallback uses the DID with `:` and `/` replaced by `_`.
        // Don't assert the exact string — depends on bob's
        // `did:key:z6Mk…` form. Just confirm it's a single segment
        // under the root and contains no path separators.
        let first = path
            .strip_prefix(root())
            .unwrap()
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_string_lossy()
            .into_owned();
        assert!(first.starts_with("did_key_"), "got: {first}");
        assert!(!first.contains(':'));
        assert!(!first.contains('/'));
    }

    #[test]
    fn nested_channel_handle_becomes_nested_dirs() {
        let mut aliases = AliasMap::new(alice());
        aliases.insert(themia(), "themia.pro");
        let recipient = Recipient::new(
            themia(),
            QueueHandle::parse("dommage-corporel:paris-cohort").unwrap(),
        );
        assert_eq!(
            queue_dir(&aliases, &recipient, root()),
            PathBuf::from("/var/secretariat/themia.pro/channels/dommage-corporel/paris-cohort"),
        );
    }

    #[test]
    fn sub_paths_compose_correctly() {
        let aliases = AliasMap::new(alice());
        let recipient = Recipient::new(alice(), QueueHandle::parse("writing").unwrap());
        let base = root();
        assert_eq!(
            envelopes_dir(&aliases, &recipient, base),
            PathBuf::from("/var/secretariat/_self/channels/writing/envelopes"),
        );
        assert_eq!(
            outbox_dir(&aliases, &recipient, base),
            PathBuf::from("/var/secretariat/_self/channels/writing/outbox"),
        );
        assert_eq!(
            ciphertext_dir(&aliases, &recipient, base),
            PathBuf::from("/var/secretariat/_self/channels/writing/_ciphertext"),
        );
    }
}
