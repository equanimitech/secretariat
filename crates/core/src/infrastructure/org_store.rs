//! Per-org metadata storage at `<orgs_root>/<alias>/org.md`.
//!
//! Markdown + YAML frontmatter (same shape as `channel.md`):
//!
//! ```markdown
//! ---
//! $type: tech.equanimi.secretariat.org
//! alias: themia.pro
//! did: did:web:themia.pro
//! name: Themia
//! description: Legal-tech jurimetry platform
//! created_at: 2026-05-12T03:00:00Z
//! ---
//!
//! # Themia
//!
//! Org-level prose: why this org exists in my vault, who the
//! operational contact is, signature line for org correspondence.
//! ```
//!
//! No backward-compat reads — pre-v0.7 vaults migrate via
//! `scripts/migrate-vault-v0.7.0.sh` BEFORE upgrading.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::{Did, Org, OrgAlias, OrgAliasError};

const ORG_TYPE: &str = "tech.equanimi.secretariat.org";
const DEFAULT_BODY: &str = "\n# {NAME}\n\n";
/// On-disk filename for org metadata at the root of every org dir.
pub const ORG_METADATA_FILENAME: &str = "org.md";

#[derive(Debug, Error)]
pub enum OrgStoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed frontmatter at {path}: {message}")]
    MalformedFrontmatter { path: PathBuf, message: String },
    #[error("invalid alias `{alias}`: {source}")]
    InvalidAlias {
        alias: String,
        #[source]
        source: OrgAliasError,
    },
    #[error("invalid did `{did}`: {reason}")]
    InvalidDid { did: String, reason: String },
    #[error("invalid created_at `{value}` at {path}")]
    InvalidTimestamp { value: String, path: PathBuf },
    #[error("org `{0}` does not exist at this orgs_root")]
    NotFound(String),
    #[error("org `{0}` already exists at this orgs_root — refuse to overwrite")]
    AlreadyExists(String),
}

/// Path to a specific org's directory under `orgs_root`.
pub fn org_dir(orgs_root: &Path, alias: &OrgAlias) -> PathBuf {
    orgs_root.join(alias.as_str())
}

/// Path to the `org.md` metadata file inside an org's directory.
pub fn org_metadata_path(orgs_root: &Path, alias: &OrgAlias) -> PathBuf {
    org_dir(orgs_root, alias).join(ORG_METADATA_FILENAME)
}

/// Path to the channels root inside an org's directory.
pub fn org_channels_root(orgs_root: &Path, alias: &OrgAlias) -> PathBuf {
    org_dir(orgs_root, alias).join("channels")
}

/// Load an org's metadata. Returns `Ok(None)` if the dir exists but the
/// `.org` file is missing; returns `Err(NotFound)` if the dir itself
/// doesn't exist.
pub fn load_org(orgs_root: &Path, alias: &OrgAlias) -> Result<Option<Org>, OrgStoreError> {
    let dir = org_dir(orgs_root, alias);
    if !dir.exists() {
        return Err(OrgStoreError::NotFound(alias.as_str().to_string()));
    }
    let path = org_metadata_path(orgs_root, alias);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| OrgStoreError::Io {
        path: path.clone(),
        source: e,
    })?;
    let (yaml, _body) =
        split_frontmatter(&raw).ok_or_else(|| OrgStoreError::MalformedFrontmatter {
            path: path.clone(),
            message: "missing `---` frontmatter delimiters".into(),
        })?;
    let fm: OrgFrontmatter =
        serde_yaml::from_str(yaml).map_err(|e| OrgStoreError::MalformedFrontmatter {
            path: path.clone(),
            message: e.to_string(),
        })?;
    finalize(
        fm.alias,
        fm.did,
        fm.name,
        fm.description,
        fm.created_at,
        &path,
    )
    .map(Some)
}

fn finalize(
    alias_str: String,
    did_str: Option<String>,
    name: String,
    description: String,
    created_at: String,
    path: &Path,
) -> Result<Org, OrgStoreError> {
    let alias = OrgAlias::parse(&alias_str).map_err(|e| OrgStoreError::InvalidAlias {
        alias: alias_str.clone(),
        source: e,
    })?;
    let did = match did_str {
        Some(s) if !s.is_empty() => {
            Some(Did::parse(&s).map_err(|e| OrgStoreError::InvalidDid {
                did: s,
                reason: e.to_string(),
            })?)
        }
        _ => None,
    };
    let created_at = DateTime::parse_from_rfc3339(&created_at)
        .map_err(|_| OrgStoreError::InvalidTimestamp {
            value: created_at.clone(),
            path: path.to_path_buf(),
        })?
        .with_timezone(&Utc);
    Ok(Org::new(alias, did, name, description, created_at))
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct OrgFrontmatter {
    #[serde(rename = "$type", default, skip_serializing_if = "String::is_empty")]
    ty: String,
    alias: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    did: Option<String>,
    name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
    created_at: String,
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

/// Atomic save (temp + rename). Creates parent dirs on demand. Errors
/// with `AlreadyExists` if `overwrite=false` and the file is already
/// present.
pub fn save_org(orgs_root: &Path, org: &Org, overwrite: bool) -> Result<(), OrgStoreError> {
    let dir = org_dir(orgs_root, &org.alias);
    std::fs::create_dir_all(&dir).map_err(|e| OrgStoreError::Io {
        path: dir.clone(),
        source: e,
    })?;
    // Pre-create the channels root so listing it doesn't error.
    let channels_root = org_channels_root(orgs_root, &org.alias);
    std::fs::create_dir_all(&channels_root).map_err(|e| OrgStoreError::Io {
        path: channels_root,
        source: e,
    })?;

    let md_path = org_metadata_path(orgs_root, &org.alias);
    if md_path.exists() && !overwrite {
        return Err(OrgStoreError::AlreadyExists(org.alias.as_str().to_string()));
    }

    let fm = OrgFrontmatter {
        ty: ORG_TYPE.to_string(),
        alias: org.alias.as_str().to_string(),
        did: org.did.as_ref().map(|d| d.as_str().to_string()),
        name: org.name.clone(),
        description: org.description.clone(),
        created_at: org.created_at.to_rfc3339(),
    };
    let yaml = serde_yaml::to_string(&fm).map_err(|e| OrgStoreError::MalformedFrontmatter {
        path: md_path.clone(),
        message: e.to_string(),
    })?;
    let title = if org.name.is_empty() {
        org.alias.as_str()
    } else {
        &org.name
    };
    let body = DEFAULT_BODY.replace("{NAME}", title);
    let rendered = format!("---\n{yaml}---\n{body}");

    let tmp = md_path.with_extension("md.tmp");
    std::fs::write(&tmp, rendered).map_err(|e| OrgStoreError::Io {
        path: tmp.clone(),
        source: e,
    })?;
    std::fs::rename(&tmp, &md_path).map_err(|e| OrgStoreError::Io {
        path: md_path.clone(),
        source: e,
    })?;
    Ok(())
}

/// List every org under `orgs_root` (skipping `_archive/` and any
/// dir with no `.org` metadata file).
pub fn list_org_dirs(orgs_root: &Path) -> Result<Vec<Org>, OrgStoreError> {
    let mut out = Vec::new();
    if !orgs_root.exists() {
        return Ok(out);
    }
    let entries = std::fs::read_dir(orgs_root).map_err(|e| OrgStoreError::Io {
        path: orgs_root.to_path_buf(),
        source: e,
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| OrgStoreError::Io {
            path: orgs_root.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('_') {
            // Reserved (`_archive/`, future `_index/`).
            continue;
        }
        let Ok(alias) = OrgAlias::parse(name) else {
            // Not a valid org alias — likely substrate noise. Skip.
            continue;
        };
        if let Some(org) = load_org(orgs_root, &alias)? {
            out.push(org);
        }
    }
    out.sort_by(|a, b| a.alias.as_str().cmp(b.alias.as_str()));
    Ok(out)
}

/// Hard-delete an org's tree. Returns `NotFound` if the dir didn't
/// exist. Caller is responsible for any confirmation UX.
pub fn delete_org(orgs_root: &Path, alias: &OrgAlias) -> Result<(), OrgStoreError> {
    let dir = org_dir(orgs_root, alias);
    if !dir.exists() {
        return Err(OrgStoreError::NotFound(alias.as_str().to_string()));
    }
    std::fs::remove_dir_all(&dir).map_err(|e| OrgStoreError::Io {
        path: dir,
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn themia(when: DateTime<Utc>) -> Org {
        Org::new(
            OrgAlias::parse("themia.pro").unwrap(),
            Some(Did::parse("did:web:themia.pro").unwrap()),
            "Themia",
            "Legal-tech",
            when,
        )
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let orgs_root = dir.path().join("orgs");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let org = themia(when);
        save_org(&orgs_root, &org, false).unwrap();
        let loaded = load_org(&orgs_root, &org.alias).unwrap().unwrap();
        assert_eq!(loaded, org);
        // Channels root created alongside .org metadata.
        assert!(org_channels_root(&orgs_root, &org.alias).is_dir());
    }

    #[test]
    fn save_refuses_to_overwrite_unless_explicit() {
        let dir = TempDir::new().unwrap();
        let orgs_root = dir.path().join("orgs");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let org = themia(when);
        save_org(&orgs_root, &org, false).unwrap();
        let r = save_org(&orgs_root, &org, false);
        assert!(matches!(r, Err(OrgStoreError::AlreadyExists(_))));
        // Overwrite explicit succeeds.
        save_org(&orgs_root, &org, true).unwrap();
    }

    #[test]
    fn load_org_for_missing_dir_errors() {
        let dir = TempDir::new().unwrap();
        let orgs_root = dir.path().join("orgs");
        let alias = OrgAlias::parse("themia.pro").unwrap();
        let r = load_org(&orgs_root, &alias);
        assert!(matches!(r, Err(OrgStoreError::NotFound(_))));
    }

    #[test]
    fn list_returns_sorted_orgs() {
        let dir = TempDir::new().unwrap();
        let orgs_root = dir.path().join("orgs");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        save_org(&orgs_root, &themia(when), false).unwrap();
        save_org(
            &orgs_root,
            &Org::new(
                OrgAlias::parse("equanimi.tech").unwrap(),
                None,
                "EquanimiTech",
                "",
                when,
            ),
            false,
        )
        .unwrap();
        // Drop an _archive dir; must be skipped.
        std::fs::create_dir_all(orgs_root.join("_archive")).unwrap();

        let orgs = list_org_dirs(&orgs_root).unwrap();
        assert_eq!(orgs.len(), 2);
        // Sorted alphabetical by alias.
        assert_eq!(orgs[0].alias.as_str(), "equanimi.tech");
        assert_eq!(orgs[1].alias.as_str(), "themia.pro");
    }

    #[test]
    fn delete_removes_tree() {
        let dir = TempDir::new().unwrap();
        let orgs_root = dir.path().join("orgs");
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let org = themia(when);
        save_org(&orgs_root, &org, false).unwrap();
        delete_org(&orgs_root, &org.alias).unwrap();
        assert!(!org_dir(&orgs_root, &org.alias).exists());
        let r = load_org(&orgs_root, &org.alias);
        assert!(matches!(r, Err(OrgStoreError::NotFound(_))));
    }

    #[test]
    fn missing_metadata_file_returns_none_not_error() {
        // Dir exists but .org file missing — surfacing as None lets the
        // app reconstruct from scratch or migrate.
        let dir = TempDir::new().unwrap();
        let orgs_root = dir.path().join("orgs");
        let alias = OrgAlias::parse("themia.pro").unwrap();
        std::fs::create_dir_all(org_dir(&orgs_root, &alias)).unwrap();
        let r = load_org(&orgs_root, &alias).unwrap();
        assert!(r.is_none());
    }
}
