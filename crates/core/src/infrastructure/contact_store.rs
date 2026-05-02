//! Contact book persistence at `~/.secretariat/contacts.json`.
//!
//! On-disk shape (versioned for forward compatibility):
//!
//! ```json
//! {
//!   "version": 1,
//!   "contacts": [
//!     {
//!       "did": "did:web:rafa.equanimi.tech",
//!       "display_name": "Rafa"
//!     },
//!     {
//!       "did": "did:key:z6Mk...",
//!       "display_name": "Marcelo",
//!       "relay_endpoint": "wss://relay.rafa.equanimi.tech"
//!     }
//!   ]
//! }
//! ```
//!
//! `relay_endpoint` is omitted for `did:web` peers (their relay is
//! discovered live via the DID document's `serviceEndpoint`) and required
//! for `did:key` peers (whose relay must be exchanged out-of-band).
//!
//! Writes are atomic via `tempfile::NamedTempFile::persist`. The file is
//! created with `0600` on Unix — the contact book reveals who you
//! correspond with, which is private metadata.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::domain::{Contact, Did};

const STORE_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ContactStoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("contacts file is malformed JSON: {0}")]
    MalformedJson(#[from] serde_json::Error),
    #[error("contacts file has unsupported version {0} (this build understands {STORE_VERSION})")]
    UnsupportedVersion(u32),
    #[error("a contact with DID `{0}` already exists")]
    DuplicateDid(Did),
    #[error("a contact with name slug `{0}` already exists (display names must be unique)")]
    DuplicateSlug(String),
    #[error("no contact found matching `{0}`")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContactsFile {
    version: u32,
    contacts: Vec<Contact>,
}

impl Default for ContactsFile {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            contacts: Vec::new(),
        }
    }
}

/// In-memory view of the contact book. Mutations don't touch disk until
/// [`ContactBook::save`] is called.
#[derive(Debug, Clone, Default)]
pub struct ContactBook {
    contacts: Vec<Contact>,
}

impl ContactBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load from disk. Missing file is not an error — returns an empty book.
    pub fn load(path: &Path) -> Result<Self, ContactStoreError> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let raw = fs::read_to_string(path).map_err(|e| ContactStoreError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let parsed: ContactsFile = serde_json::from_str(&raw)?;
        if parsed.version != STORE_VERSION {
            return Err(ContactStoreError::UnsupportedVersion(parsed.version));
        }
        Ok(Self {
            contacts: parsed.contacts,
        })
    }

    /// Persist to disk atomically. Creates parent directory if needed.
    /// File is mode `0600` on Unix.
    pub fn save(&self, path: &Path) -> Result<(), ContactStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| ContactStoreError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let snapshot = ContactsFile {
            version: STORE_VERSION,
            contacts: self.contacts.clone(),
        };
        let pretty = serde_json::to_string_pretty(&snapshot)?;

        let parent = path.parent().unwrap_or(Path::new("."));
        let mut tmp = NamedTempFile::new_in(parent).map_err(|e| ContactStoreError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
        tmp.write_all(pretty.as_bytes())
            .and_then(|_| tmp.write_all(b"\n"))
            .map_err(|e| ContactStoreError::Io {
                path: tmp.path().to_path_buf(),
                source: e,
            })?;

        // Apply 0600 before atomic rename so the final file is never world-readable.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(tmp.path(), perms).map_err(|e| ContactStoreError::Io {
                path: tmp.path().to_path_buf(),
                source: e,
            })?;
        }

        tmp.persist(path).map_err(|e| ContactStoreError::Io {
            path: path.to_path_buf(),
            source: e.error,
        })?;

        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = &Contact> {
        self.contacts.iter()
    }

    pub fn len(&self) -> usize {
        self.contacts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.contacts.is_empty()
    }

    /// Look up by exact DID equality.
    pub fn find_by_did(&self, did: &Did) -> Option<&Contact> {
        self.contacts.iter().find(|c| &c.did == did)
    }

    /// Look up by case-insensitive display-name slug.
    /// See [`crate::domain::DisplayName::slug`] for the slug rule.
    pub fn find_by_slug(&self, slug: &str) -> Option<&Contact> {
        let target = slug.to_lowercase();
        self.contacts
            .iter()
            .find(|c| c.display_name.slug() == target)
    }

    /// Add a contact. Errors on duplicate DID or duplicate slug.
    pub fn add(&mut self, contact: Contact) -> Result<(), ContactStoreError> {
        if self.find_by_did(&contact.did).is_some() {
            return Err(ContactStoreError::DuplicateDid(contact.did));
        }
        let slug = contact.display_name.slug();
        if self
            .contacts
            .iter()
            .any(|c| c.display_name.slug() == slug)
        {
            return Err(ContactStoreError::DuplicateSlug(slug));
        }
        self.contacts.push(contact);
        Ok(())
    }

    /// Remove by slug (CLI-friendly). Returns the removed contact.
    /// Errors if no match.
    pub fn remove_by_slug(&mut self, slug: &str) -> Result<Contact, ContactStoreError> {
        let target = slug.to_lowercase();
        let idx = self
            .contacts
            .iter()
            .position(|c| c.display_name.slug() == target)
            .ok_or_else(|| ContactStoreError::NotFound(slug.to_string()))?;
        Ok(self.contacts.remove(idx))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DisplayName, RelayEndpoint};
    use tempfile::TempDir;

    fn rafa() -> Contact {
        // did:web peer — no relay_endpoint stored locally (looked up live).
        Contact::new(
            Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            DisplayName::parse("Rafa").unwrap(),
            None,
        )
    }

    fn marcelo() -> Contact {
        // did:key peer — relay_endpoint exchanged out-of-band.
        Contact::new(
            Did::parse("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK").unwrap(),
            DisplayName::parse("Marcelo").unwrap(),
            Some(RelayEndpoint::parse("wss://relay.rafa.equanimi.tech").unwrap()),
        )
    }

    #[test]
    fn load_missing_file_returns_empty_book() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.json");
        let book = ContactBook::load(&path).unwrap();
        assert!(book.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.json");
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        book.add(marcelo()).unwrap();
        book.save(&path).unwrap();

        let reloaded = ContactBook::load(&path).unwrap();
        assert_eq!(reloaded.len(), 2);
        assert!(reloaded
            .find_by_did(&Did::parse("did:web:rafa.equanimi.tech").unwrap())
            .is_some());
        assert!(reloaded.find_by_slug("marcelo").is_some());
    }

    #[test]
    fn save_writes_0600_on_unix() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.json");
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        book.save(&path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "contacts.json mode must be 0600 (got {:o})", mode);
        }
    }

    #[test]
    fn add_rejects_duplicate_did() {
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        let dup = Contact::new(
            Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            DisplayName::parse("Rafael").unwrap(),
            None,
        );
        assert!(matches!(
            book.add(dup),
            Err(ContactStoreError::DuplicateDid(_))
        ));
        assert_eq!(book.len(), 1);
    }

    #[test]
    fn add_rejects_duplicate_slug() {
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        let dup = Contact::new(
            Did::parse("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK").unwrap(),
            DisplayName::parse("RAFA").unwrap(), // same slug
            None,
        );
        assert!(matches!(
            book.add(dup),
            Err(ContactStoreError::DuplicateSlug(_))
        ));
    }

    #[test]
    fn find_by_did_works() {
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        book.add(marcelo()).unwrap();
        let found = book
            .find_by_did(&Did::parse("did:web:rafa.equanimi.tech").unwrap())
            .unwrap();
        assert_eq!(found.display_name.as_str(), "Rafa");
    }

    #[test]
    fn find_by_slug_is_case_insensitive() {
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        assert!(book.find_by_slug("rafa").is_some());
        assert!(book.find_by_slug("RAFA").is_some());
        assert!(book.find_by_slug("Rafa").is_some());
        assert!(book.find_by_slug("nope").is_none());
    }

    #[test]
    fn remove_by_slug_works() {
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        book.add(marcelo()).unwrap();
        let removed = book.remove_by_slug("marcelo").unwrap();
        assert_eq!(removed.display_name.as_str(), "Marcelo");
        assert_eq!(book.len(), 1);
        assert!(book.find_by_slug("marcelo").is_none());
    }

    #[test]
    fn remove_unknown_slug_errors() {
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        assert!(matches!(
            book.remove_by_slug("ghost"),
            Err(ContactStoreError::NotFound(_))
        ));
    }

    #[test]
    fn unsupported_version_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.json");
        fs::write(
            &path,
            r#"{"version": 999, "contacts": []}"#,
        )
        .unwrap();
        assert!(matches!(
            ContactBook::load(&path),
            Err(ContactStoreError::UnsupportedVersion(999))
        ));
    }

    #[test]
    fn malformed_json_errors() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.json");
        fs::write(&path, "{ this is not json").unwrap();
        assert!(matches!(
            ContactBook::load(&path),
            Err(ContactStoreError::MalformedJson(_))
        ));
    }

    #[test]
    fn save_overwrites_atomically() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.json");

        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        book.save(&path).unwrap();

        book.add(marcelo()).unwrap();
        book.save(&path).unwrap();

        let reloaded = ContactBook::load(&path).unwrap();
        assert_eq!(reloaded.len(), 2);
    }
}
