//! Contact book persistence at `<self_root>/contacts.md`.
//!
//! One markdown file with one `## <display_name>` section per contact.
//! Each section opens with a YAML frontmatter block carrying the typed
//! fields (`did`, `display_name`, `relay_endpoint?`, `added_at?`); the
//! body following the frontmatter is free-form principal-editable
//! prose about that contact.
//!
//! Shape:
//!
//! ```markdown
//! # Contacts
//!
//! ## Marcelo
//!
//! ---
//! did: did:key:z6Mk...
//! display_name: Marcelo
//! relay_endpoint: wss://relay.rafa.equanimi.tech
//! ---
//!
//! Free-form notes about Marcelo. Prefers vouvoiement, replies in
//! Portuguese when written in Portuguese.
//!
//! ## Christophe
//!
//! ---
//! did: did:web:christophe-marchand.com
//! display_name: Christophe
//! ---
//!
//! Avocat, dommage corporel. French only.
//! ```
//!
//! Backward-compat: loads legacy `contacts.json` (single-file JSON with
//! `{version: 1, contacts: [...]}`) when the new `contacts.md` is
//! absent. Writes always target the new shape.
//!
//! File mode is `0600` on Unix — the contact book reveals who you
//! correspond with, which is private metadata.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::domain::{Contact, Did, DisplayName, RelayEndpoint};

#[derive(Debug, Error)]
pub enum ContactStoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed contacts.md at {path}: {message}")]
    MalformedMarkdown { path: PathBuf, message: String },
    #[error("a contact with DID `{0}` already exists")]
    DuplicateDid(Did),
    #[error("a contact with name slug `{0}` already exists (display names must be unique)")]
    DuplicateSlug(String),
    #[error("no contact found matching `{0}`")]
    NotFound(String),
}

/// Per-contact section: structured fields + free-form body prose.
#[derive(Debug, Clone)]
struct ContactSection {
    contact: Contact,
    body: String,
}

/// In-memory view of the contact book. Mutations don't touch disk until
/// [`ContactBook::save`] is called.
#[derive(Debug, Clone, Default)]
pub struct ContactBook {
    sections: Vec<ContactSection>,
}

impl ContactBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load from `<self_root>/contacts.md`. Missing file is not an error
    /// — returns an empty book.
    pub fn load(path: &Path) -> Result<Self, ContactStoreError> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let raw = fs::read_to_string(path).map_err(|e| ContactStoreError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        parse_markdown(&raw, path)
    }

    /// Persist to `contacts.md` atomically. Creates parent dir if needed.
    /// File is mode `0600` on Unix.
    pub fn save(&self, path: &Path) -> Result<(), ContactStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| ContactStoreError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }

        let rendered = render_markdown(&self.sections)?;
        let parent = path.parent().unwrap_or(Path::new("."));
        let mut tmp = NamedTempFile::new_in(parent).map_err(|e| ContactStoreError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
        tmp.write_all(rendered.as_bytes())
            .map_err(|e| ContactStoreError::Io {
                path: tmp.path().to_path_buf(),
                source: e,
            })?;

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
        self.sections.iter().map(|s| &s.contact)
    }

    pub fn len(&self) -> usize {
        self.sections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }

    pub fn find_by_did(&self, did: &Did) -> Option<&Contact> {
        self.sections
            .iter()
            .map(|s| &s.contact)
            .find(|c| &c.did == did)
    }

    pub fn find_by_slug(&self, slug: &str) -> Option<&Contact> {
        let target = slug.to_lowercase();
        self.sections
            .iter()
            .map(|s| &s.contact)
            .find(|c| c.display_name.slug() == target)
    }

    /// Return the principal-editable prose body for a contact, if any.
    pub fn body_for_slug(&self, slug: &str) -> Option<&str> {
        let target = slug.to_lowercase();
        self.sections
            .iter()
            .find(|s| s.contact.display_name.slug() == target)
            .map(|s| s.body.as_str())
    }

    pub fn add(&mut self, contact: Contact) -> Result<(), ContactStoreError> {
        if self.find_by_did(&contact.did).is_some() {
            return Err(ContactStoreError::DuplicateDid(contact.did));
        }
        let slug = contact.display_name.slug();
        if self
            .sections
            .iter()
            .any(|s| s.contact.display_name.slug() == slug)
        {
            return Err(ContactStoreError::DuplicateSlug(slug));
        }
        self.sections.push(ContactSection {
            contact,
            body: String::new(),
        });
        Ok(())
    }

    pub fn remove_by_slug(&mut self, slug: &str) -> Result<Contact, ContactStoreError> {
        let target = slug.to_lowercase();
        let idx = self
            .sections
            .iter()
            .position(|s| s.contact.display_name.slug() == target)
            .ok_or_else(|| ContactStoreError::NotFound(slug.to_string()))?;
        Ok(self.sections.remove(idx).contact)
    }
}

// ---------------------------------------------------------------------------
// Markdown shape (read + write)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
struct ContactFrontmatter {
    #[serde(rename = "$type", default, skip_serializing_if = "String::is_empty")]
    ty: String,
    did: String,
    display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    relay_endpoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    added_at: Option<String>,
}

const CONTACT_TYPE: &str = "tech.equanimi.secretariat.contact";

fn parse_markdown(raw: &str, path: &Path) -> Result<ContactBook, ContactStoreError> {
    let mut sections = Vec::new();
    // Skip any preamble before the first `## ` header (e.g. `# Contacts`).
    let lines: Vec<&str> = raw.lines().collect();
    let mut cursor = 0usize;
    while cursor < lines.len() {
        if lines[cursor].starts_with("## ") {
            break;
        }
        cursor += 1;
    }
    while cursor < lines.len() {
        if !lines[cursor].starts_with("## ") {
            cursor += 1;
            continue;
        }
        // Section header. Find the bounds of this section (until next ## or EOF).
        let _name = lines[cursor].trim_start_matches("## ").trim().to_string();
        cursor += 1;
        let section_start = cursor;
        while cursor < lines.len() && !lines[cursor].starts_with("## ") {
            cursor += 1;
        }
        let section_end = cursor;
        let section_text = lines[section_start..section_end].join("\n");
        let section = parse_section(&section_text, path)?;
        sections.push(section);
        // Cursor already at next `## ` or EOF — loop continues.
    }
    Ok(ContactBook { sections })
}

fn parse_section(text: &str, path: &Path) -> Result<ContactSection, ContactStoreError> {
    // Skip leading blanks before frontmatter.
    let trimmed = text.trim_start_matches(['\n', '\r', ' ']);
    let (yaml, body) = split_frontmatter(trimmed).ok_or_else(|| {
        ContactStoreError::MalformedMarkdown {
            path: path.to_path_buf(),
            message: "contact section missing `---` frontmatter".into(),
        }
    })?;
    let fm: ContactFrontmatter =
        serde_yaml::from_str(yaml).map_err(|e| ContactStoreError::MalformedMarkdown {
            path: path.to_path_buf(),
            message: format!("contact frontmatter YAML: {e}"),
        })?;
    let did = Did::parse(&fm.did).map_err(|e| ContactStoreError::MalformedMarkdown {
        path: path.to_path_buf(),
        message: format!("invalid did `{}`: {e}", fm.did),
    })?;
    let display_name =
        DisplayName::parse(&fm.display_name).map_err(|e| ContactStoreError::MalformedMarkdown {
            path: path.to_path_buf(),
            message: format!("invalid display_name `{}`: {e}", fm.display_name),
        })?;
    let relay_endpoint = match fm.relay_endpoint.as_deref() {
        None | Some("") => None,
        Some(s) => Some(RelayEndpoint::parse(s).map_err(|e| {
            ContactStoreError::MalformedMarkdown {
                path: path.to_path_buf(),
                message: format!("invalid relay_endpoint `{s}`: {e}"),
            }
        })?),
    };
    Ok(ContactSection {
        contact: Contact::new(did, display_name, relay_endpoint),
        body: body.trim_start_matches('\n').to_string(),
    })
}

fn render_markdown(sections: &[ContactSection]) -> Result<String, ContactStoreError> {
    let mut out = String::from("# Contacts\n");
    for s in sections {
        let fm = ContactFrontmatter {
            ty: CONTACT_TYPE.to_string(),
            did: s.contact.did.as_str().to_string(),
            display_name: s.contact.display_name.to_string(),
            relay_endpoint: s
                .contact
                .relay_endpoint
                .as_ref()
                .map(|r| r.as_str().to_string()),
            added_at: None,
        };
        let yaml = serde_yaml::to_string(&fm).map_err(|e| ContactStoreError::MalformedMarkdown {
            path: PathBuf::new(),
            message: format!("emit frontmatter: {e}"),
        })?;
        out.push_str(&format!("\n## {}\n\n", s.contact.display_name));
        out.push_str(&format!("---\n{yaml}---\n"));
        if !s.body.is_empty() {
            out.push('\n');
            out.push_str(&s.body);
            if !s.body.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    Ok(out)
}

fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    let stripped = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    let after_open = stripped
        .strip_prefix("---\r\n")
        .or_else(|| stripped.strip_prefix("---\n"))?;
    let mut search_start = 0usize;
    while let Some(rel) = after_open[search_start..].find("\n---") {
        let abs = search_start + rel;
        let after_dashes = abs + 4;
        let tail = &after_open[after_dashes..];
        if let Some(after_lf) = tail.strip_prefix('\n') {
            return Some((&after_open[..abs], after_lf));
        }
        if let Some(after_crlf) = tail.strip_prefix("\r\n") {
            return Some((&after_open[..abs], after_crlf));
        }
        if tail.is_empty() {
            return Some((&after_open[..abs], ""));
        }
        search_start = abs + 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn rafa() -> Contact {
        Contact::new(
            Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            DisplayName::parse("Rafa").unwrap(),
            None,
        )
    }

    fn marcelo() -> Contact {
        Contact::new(
            Did::parse("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK").unwrap(),
            DisplayName::parse("Marcelo").unwrap(),
            Some(RelayEndpoint::parse("wss://relay.rafa.equanimi.tech").unwrap()),
        )
    }

    #[test]
    fn load_missing_file_returns_empty_book() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.md");
        let book = ContactBook::load(&path).unwrap();
        assert!(book.is_empty());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.md");
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
        let path = dir.path().join("contacts.md");
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        book.save(&path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "contacts.md mode must be 0600 (got {:o})", mode);
        }
    }

    #[test]
    fn duplicate_did_rejected() {
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        let r = book.add(rafa());
        assert!(matches!(r, Err(ContactStoreError::DuplicateDid(_))));
    }

    #[test]
    fn duplicate_slug_rejected() {
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        let dupe = Contact::new(
            Did::parse("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK").unwrap(),
            DisplayName::parse("Rafa").unwrap(),
            None,
        );
        let r = book.add(dupe);
        assert!(matches!(r, Err(ContactStoreError::DuplicateSlug(_))));
    }

    #[test]
    fn body_prose_preserved_across_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("contacts.md");
        let mut book = ContactBook::new();
        book.add(rafa()).unwrap();
        book.save(&path).unwrap();

        // Hand-edit the body under Rafa's section.
        let raw = std::fs::read_to_string(&path).unwrap();
        let augmented = raw + "\nHand-written prose about Rafa.\n";
        std::fs::write(&path, augmented).unwrap();

        let reloaded = ContactBook::load(&path).unwrap();
        assert_eq!(reloaded.len(), 1);
        let body = reloaded.body_for_slug("rafa").unwrap();
        assert!(body.contains("Hand-written prose"));
    }
}
