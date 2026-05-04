//! Use case: process correspondence claim events and auto-add bilateral contacts.
//!
//! When a peer claims an invite this principal created, the relay records
//! it. The inviter's daemon pulls those claim events and turns each one
//! into a contact-book entry — making bidirectional contact-add the
//! defining behavior of a correspondence invite (see
//! `docs/milestones/2026-05-04-tauri-front-door.md` slice 2,
//! `memory/project_invite_is_correspondence.md`).
//!
//! Naming policy lives here, not in the daemon's IO loop:
//!
//! - First choice: the invite's `purpose` (e.g. "co-author book" → contact
//!   "co-author book"). Lets the inviter set the relationship name when
//!   creating the invite.
//! - Fallback: a short DID-suffix-disambiguated name ("peer-z6Mk…dx"),
//!   so a contact still lands even when no purpose was supplied.
//! - Slug collision: append the DID's tail-4 to disambiguate
//!   ("co-author book-eddx").
//! - Already known by DID: skip silently, idempotent — the daemon polls
//!   relays repeatedly and we never want duplicate adds.

use std::path::Path;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::domain::{Contact, Did, DisplayName, RelayEndpoint};
use crate::infrastructure::contact_store::{
    ContactBook, ContactStoreError,
};

/// One claim event in domain terms (not transport terms). Created by the
/// daemon at the wire boundary; passed into [`process_correspondence_claims`].
#[derive(Debug, Clone)]
pub struct CorrespondenceClaim {
    pub claimant: Did,
    pub claimed_at: DateTime<Utc>,
    pub purpose: Option<String>,
}

#[derive(Debug, Error)]
pub enum ClaimProcessError {
    #[error("contact store: {0}")]
    Store(#[from] ContactStoreError),
}

/// Why a single claim wasn't turned into a new contact entry. The caller
/// can log these without treating them as failures (most are normal:
/// re-polling the same relay returns the same claims).
#[derive(Debug, Clone)]
pub enum SkipReason {
    AlreadyKnown,
    NameInvalid(String),
}

#[derive(Debug, Clone)]
pub struct ClaimProcessOutcome {
    pub added: Vec<Contact>,
    pub skipped: Vec<(Did, SkipReason)>,
}

/// Idempotent. Safe to call every poll tick — claimers already in the
/// contact book are skipped.
pub fn process_correspondence_claims(
    claims: Vec<CorrespondenceClaim>,
    contacts_path: &Path,
    relay_endpoint: &RelayEndpoint,
) -> Result<ClaimProcessOutcome, ClaimProcessError> {
    let mut book = ContactBook::load(contacts_path)?;
    let mut added = Vec::new();
    let mut skipped = Vec::new();

    for claim in claims {
        if book.find_by_did(&claim.claimant).is_some() {
            skipped.push((claim.claimant, SkipReason::AlreadyKnown));
            continue;
        }

        let name = match propose_contact_name(&claim.claimant, claim.purpose.as_deref(), &book) {
            Ok(n) => n,
            Err(e) => {
                skipped.push((claim.claimant, SkipReason::NameInvalid(e)));
                continue;
            }
        };

        let contact = Contact::new(claim.claimant.clone(), name, Some(relay_endpoint.clone()));
        match book.add(contact.clone()) {
            Ok(()) => added.push(contact),
            Err(ContactStoreError::DuplicateDid(_)) => {
                // Race: another process added them between our check and add.
                // Treat as already-known.
                skipped.push((claim.claimant, SkipReason::AlreadyKnown));
            }
            Err(other) => return Err(ClaimProcessError::Store(other)),
        }
    }

    if !added.is_empty() {
        book.save(contacts_path)?;
    }
    Ok(ClaimProcessOutcome { added, skipped })
}

/// Policy: how to name a contact derived from a claim event.
///
/// Returns the first parseable name from this priority list, with each
/// candidate disambiguated against the existing book if its slug already
/// exists:
///
/// 1. `purpose` (if present and parseable as a DisplayName)
/// 2. `peer-<did-suffix>` (always a parseable DisplayName)
///
/// On slug collision, appends the DID's tail-4: `purpose-eddx`.
fn propose_contact_name(
    claimant: &Did,
    purpose: Option<&str>,
    book: &ContactBook,
) -> Result<DisplayName, String> {
    if let Some(p) = purpose {
        if let Ok(name) = DisplayName::parse(p) {
            if book.find_by_slug(&name.slug()).is_none() {
                return Ok(name);
            }
            // Slug collision — disambiguate with DID tail.
            let disambig = format!("{} {}", p, did_short_suffix(claimant, 4));
            if let Ok(name) = DisplayName::parse(&disambig) {
                if book.find_by_slug(&name.slug()).is_none() {
                    return Ok(name);
                }
            }
        }
    }

    let fallback_str = format!("peer {}", did_short_suffix(claimant, 6));
    DisplayName::parse(&fallback_str).map_err(|e| format!("could not parse fallback name: {e}"))
}

/// Last `n` characters of the DID string. Used to give auto-added contacts
/// a stable suffix that's distinct enough across peers without leaking
/// the full DID into the display name.
fn did_short_suffix(did: &Did, n: usize) -> String {
    let s = did.as_str();
    let len = s.chars().count();
    s.chars().skip(len.saturating_sub(n)).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    /// Synthetic DIDs for tests — derived from deterministic seed bytes
    /// so we never embed a real principal's DID in source. See
    /// `memory/feedback_no_real_dids_in_tests.md`.
    fn alice_did() -> Did {
        Did::from_ed25519_public_key(&[0xa1; 32])
    }

    fn bob_did() -> Did {
        Did::from_ed25519_public_key(&[0xb0; 32])
    }

    fn relay() -> RelayEndpoint {
        RelayEndpoint::parse("https://secretariat.equanimi.tech").unwrap()
    }

    #[test]
    fn adds_one_contact_using_purpose_as_name() {
        let dir = TempDir::new().unwrap();
        let contacts = dir.path().join("contacts.json");
        let claim = CorrespondenceClaim {
            claimant: bob_did(),
            claimed_at: Utc.with_ymd_and_hms(2026, 5, 4, 17, 0, 0).unwrap(),
            purpose: Some("co-author book".into()),
        };

        let out = process_correspondence_claims(vec![claim], &contacts, &relay()).unwrap();
        assert_eq!(out.added.len(), 1);
        assert_eq!(out.skipped.len(), 0);
        assert_eq!(out.added[0].display_name.as_str(), "co-author book");
    }

    #[test]
    fn falls_back_to_did_suffix_when_purpose_missing() {
        let dir = TempDir::new().unwrap();
        let contacts = dir.path().join("contacts.json");
        let claim = CorrespondenceClaim {
            claimant: bob_did(),
            claimed_at: Utc::now(),
            purpose: None,
        };

        let out = process_correspondence_claims(vec![claim], &contacts, &relay()).unwrap();
        assert_eq!(out.added.len(), 1);
        assert!(out.added[0].display_name.as_str().starts_with("peer "));
    }

    #[test]
    fn idempotent_on_repoll_of_same_claim() {
        let dir = TempDir::new().unwrap();
        let contacts = dir.path().join("contacts.json");
        let claim = CorrespondenceClaim {
            claimant: bob_did(),
            claimed_at: Utc::now(),
            purpose: Some("first-contact".into()),
        };

        // First pass: add.
        let out1 = process_correspondence_claims(vec![claim.clone()], &contacts, &relay()).unwrap();
        assert_eq!(out1.added.len(), 1);
        // Second pass: skip silently.
        let out2 = process_correspondence_claims(vec![claim], &contacts, &relay()).unwrap();
        assert_eq!(out2.added.len(), 0);
        assert_eq!(out2.skipped.len(), 1);
        assert!(matches!(out2.skipped[0].1, SkipReason::AlreadyKnown));
    }

    #[test]
    fn name_collision_disambiguated_with_did_tail() {
        let dir = TempDir::new().unwrap();
        let contacts = dir.path().join("contacts.json");

        // Pre-populate with a contact that occupies the slug "first-contact".
        let pre = Contact::new(
            alice_did(),
            DisplayName::parse("first-contact").unwrap(),
            Some(relay()),
        );
        let mut book = ContactBook::load(&contacts).unwrap();
        book.add(pre).unwrap();
        book.save(&contacts).unwrap();

        let claim = CorrespondenceClaim {
            claimant: bob_did(),
            claimed_at: Utc::now(),
            purpose: Some("first-contact".into()),
        };
        let out = process_correspondence_claims(vec![claim], &contacts, &relay()).unwrap();
        assert_eq!(out.added.len(), 1);
        // Name should still contain the original purpose plus a disambiguating
        // suffix from the DID tail.
        let name = out.added[0].display_name.as_str();
        assert!(name.starts_with("first-contact"));
        assert!(name.len() > "first-contact".len());
    }
}
