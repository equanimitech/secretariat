//! Use cases for managing the contact book.
//!
//! These wrap [`ContactBook`] operations with the load → mutate → save
//! lifecycle the CLI and MCP server want. Pure error mapping; no policy
//! beyond what the underlying store enforces.

use std::path::Path;

use thiserror::Error;

use crate::domain::{Contact, Did};
use crate::infrastructure::{ContactBook, ContactStoreError};

#[derive(Debug, Error)]
pub enum ContactOpError {
    #[error("contact store: {0}")]
    Store(#[from] ContactStoreError),
}

/// Add a contact and persist. Errors on duplicate DID or slug.
pub fn add_contact(path: &Path, contact: Contact) -> Result<(), ContactOpError> {
    let mut book = ContactBook::load(path)?;
    book.add(contact)?;
    book.save(path)?;
    Ok(())
}

/// Remove by display-name slug. Returns the removed contact.
pub fn remove_contact(path: &Path, slug: &str) -> Result<Contact, ContactOpError> {
    let mut book = ContactBook::load(path)?;
    let removed = book.remove_by_slug(slug)?;
    book.save(path)?;
    Ok(removed)
}

/// List all contacts (load-only).
pub fn list_contacts(path: &Path) -> Result<Vec<Contact>, ContactOpError> {
    let book = ContactBook::load(path)?;
    Ok(book.iter().cloned().collect())
}

/// Resolve a contact by DID. Returns `Ok(None)` if not found (vs. error).
pub fn find_by_did(path: &Path, did: &Did) -> Result<Option<Contact>, ContactOpError> {
    let book = ContactBook::load(path)?;
    Ok(book.find_by_did(did).cloned())
}

/// Resolve a contact by display-name slug (case-insensitive).
/// Returns `Ok(None)` if not found.
pub fn find_by_slug(path: &Path, slug: &str) -> Result<Option<Contact>, ContactOpError> {
    let book = ContactBook::load(path)?;
    Ok(book.find_by_slug(slug).cloned())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DisplayName, RelayEndpoint};
    use tempfile::TempDir;

    fn marcelo() -> Contact {
        Contact::new(
            Did::parse("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK").unwrap(),
            DisplayName::parse("Marcelo").unwrap(),
            Some(RelayEndpoint::parse("wss://relay.rafa.equanimi.tech").unwrap()),
        )
    }

    #[test]
    fn add_and_list() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.json");
        add_contact(&path, marcelo()).unwrap();
        let listed = list_contacts(&path).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].display_name.as_str(), "Marcelo");
    }

    #[test]
    fn add_remove_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.json");
        add_contact(&path, marcelo()).unwrap();
        let removed = remove_contact(&path, "marcelo").unwrap();
        assert_eq!(removed.display_name.as_str(), "Marcelo");
        assert!(list_contacts(&path).unwrap().is_empty());
    }

    #[test]
    fn find_by_slug_returns_none_when_missing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.json");
        let result = find_by_slug(&path, "ghost").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn find_by_did_returns_some() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.json");
        add_contact(&path, marcelo()).unwrap();
        let did = Did::parse("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK").unwrap();
        let found = find_by_did(&path, &did).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().display_name.as_str(), "Marcelo");
    }
}
