//! `OrgAlias` — human-readable name for an org-subscription root under
//! `~/.secretariat/`.
//!
//! v0.3 layout (see `docs/decisions/2026-05-12-substrate-layout-v03.md`):
//! orgs the principal subscribes to appear as top-level dirs with an
//! `.identity` file but no `key`. The directory name IS the alias — for
//! `did:web` orgs it conventionally mirrors the domain
//! (`themia.pro/`, `equanimi.tech/`); for `did:key` orgs the principal
//! picks a slug at subscribe time.
//!
//! ```text
//! ~/.secretariat/
//!   themia.pro/             (did:web:themia.pro)
//!     .identity             (role: org-subscription)
//!     channel/dommage-corporel/paris-cohort/...
//! ```
//!
//! Grammar: `^[a-z0-9][a-z0-9-.]*$`. Leading underscore is reserved for
//! substrate-private directories (`_meta`, `_org`, future `_index`).
//! Substrate top-level names (`peers`, `bin`) are reserved to prevent
//! collision with substrate-global directories.
//!
//! NB: the principal's *own* passport tree is NOT an `OrgAlias` — its
//! dir name is derived from `Did::filesystem_label` (domain for did:web,
//! `slug(profile.display_name)` for did:key). See the ADR for the
//! taxonomy.

use std::fmt;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OrgAliasError {
    #[error("org alias is empty")]
    Empty,
    #[error("org alias exceeds 63-byte length")]
    TooLong,
    #[error("org alias must start with a lowercase letter or digit (leading `_` is reserved for substrate-private directories)")]
    InvalidStart,
    #[error("org alias must contain only `[a-z0-9-.]`")]
    InvalidChars,
    #[error("`{0}` is a reserved substrate directory name; pick a different alias")]
    Reserved(String),
}

/// Top-level directory names already used by the substrate root. An org
/// alias must not shadow one or the on-disk layout collides.
const RESERVED_NAMES: &[&str] = &["inbox", "queues", "peers", "bin"];

/// Maximum alias length. Matches DNS-label conventions; the alias often
/// mirrors a domain (`themia.pro`, `equanimi.tech`).
const MAX_LEN: usize = 63;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrgAlias(String);

impl OrgAlias {
    pub fn parse(s: &str) -> Result<Self, OrgAliasError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(OrgAliasError::Empty);
        }
        if s.len() > MAX_LEN {
            return Err(OrgAliasError::TooLong);
        }
        let mut chars = s.chars();
        let first = chars.next().unwrap();
        if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
            return Err(OrgAliasError::InvalidStart);
        }
        for c in chars {
            if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.') {
                return Err(OrgAliasError::InvalidChars);
            }
        }
        if RESERVED_NAMES.contains(&s) {
            return Err(OrgAliasError::Reserved(s.to_string()));
        }
        Ok(Self(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OrgAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_domain_shaped_aliases() {
        assert!(OrgAlias::parse("themia.pro").is_ok());
        assert!(OrgAlias::parse("equanimi.tech").is_ok());
        assert!(OrgAlias::parse("rafa.equanimi.tech").is_ok());
    }

    #[test]
    fn parses_kebab_aliases() {
        assert!(OrgAlias::parse("nwyana").is_ok());
        assert!(OrgAlias::parse("autonomous-enterprise").is_ok());
    }

    #[test]
    fn rejects_empty() {
        assert!(matches!(OrgAlias::parse(""), Err(OrgAliasError::Empty)));
        assert!(matches!(OrgAlias::parse("   "), Err(OrgAliasError::Empty)));
    }

    #[test]
    fn rejects_leading_underscore() {
        // Reserved for substrate-private dirs (`_meta`, `_org`, `_index`).
        assert!(matches!(
            OrgAlias::parse("_meta"),
            Err(OrgAliasError::InvalidStart)
        ));
        assert!(matches!(
            OrgAlias::parse("_self"),
            Err(OrgAliasError::InvalidStart)
        ));
    }

    #[test]
    fn rejects_leading_hyphen_or_dot() {
        assert!(matches!(
            OrgAlias::parse("-foo"),
            Err(OrgAliasError::InvalidStart)
        ));
        assert!(matches!(
            OrgAlias::parse(".foo"),
            Err(OrgAliasError::InvalidStart)
        ));
    }

    #[test]
    fn rejects_uppercase() {
        assert!(matches!(
            OrgAlias::parse("Themia"),
            Err(OrgAliasError::InvalidStart)
        ));
        assert!(matches!(
            OrgAlias::parse("themiA"),
            Err(OrgAliasError::InvalidChars)
        ));
    }

    #[test]
    fn rejects_path_traversal_chars() {
        assert!(matches!(
            OrgAlias::parse("foo/bar"),
            Err(OrgAliasError::InvalidChars)
        ));
        assert!(matches!(
            OrgAlias::parse("foo bar"),
            Err(OrgAliasError::InvalidChars)
        ));
        assert!(matches!(
            OrgAlias::parse(".."),
            Err(OrgAliasError::InvalidStart)
        ));
    }

    #[test]
    fn rejects_reserved_substrate_names() {
        for name in ["inbox", "queues", "peers", "bin"] {
            assert!(
                matches!(OrgAlias::parse(name), Err(OrgAliasError::Reserved(_))),
                "expected `{name}` to be rejected as reserved"
            );
        }
    }

    #[test]
    fn rejects_overlong() {
        let s = "a".repeat(64);
        assert!(matches!(OrgAlias::parse(&s), Err(OrgAliasError::TooLong)));
    }
}
