//! The substrate manifest: the list of git repos Secretariat treats as its
//! world. Serialized as top-level `[[repos]]` inside `preferences.toml`
//! (see `preferences.rs`). `RepoRole` gates behavior; `tags` group (the
//! org-replacement). `RepoRegistry` is a borrowed query view over the slice.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What kind of repo this is. Gates behavior, per "roles are capabilities,
/// not badges": `Home` repos are private (cross-cutting PKM, may never push)
/// and map to penceive `private-roots` in the later penceive slice.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RepoRole {
    /// Designs live with code; pushable.
    #[default]
    Project,
    /// Cross-cutting personal-knowledge / journals; private.
    Home,
}

impl RepoRole {
    /// Parse a CLI/MCP string into a role.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "project" => Ok(Self::Project),
            "home" => Ok(Self::Home),
            other => Err(format!("unknown role `{other}` (expected project|home)")),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Home => "home",
        }
    }
}

/// One enrolled repo. `path` is the identity (canonicalized absolute).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoEntry {
    pub path: PathBuf,
    #[serde(default)]
    pub role: RepoRole,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Borrowed query view over the registry slice. The walker / launcher /
/// (later) penceive take this, not the whole `Preferences`.
pub struct RepoRegistry<'a>(&'a [RepoEntry]);

impl<'a> RepoRegistry<'a> {
    pub fn new(entries: &'a [RepoEntry]) -> Self {
        Self(entries)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RepoEntry> {
        self.0.iter()
    }

    /// Repos carrying `tag`.
    pub fn with_tag(&self, tag: &str) -> impl Iterator<Item = &RepoEntry> {
        // `to_string()` is intentional: the `move` closure must own `tag`,
        // since the `&str` input cannot cross into the returned iterator.
        let tag = tag.to_string();
        self.0
            .iter()
            .filter(move |e| e.tags.iter().any(|t| t == &tag))
    }

    /// Repos with `role = Home` (the private PKM repos).
    pub fn home_repos(&self) -> impl Iterator<Item = &RepoEntry> {
        self.0.iter().filter(|e| e.role == RepoRole::Home)
    }

    /// Find an entry by exact (already-canonicalized) path.
    pub fn find(&self, path: &Path) -> Option<&RepoEntry> {
        self.0.iter().find(|e| e.path == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<RepoEntry> {
        vec![
            RepoEntry {
                path: PathBuf::from("/a/themia"),
                role: RepoRole::Project,
                tags: vec!["themia".into()],
            },
            RepoEntry {
                path: PathBuf::from("/a/knowledge"),
                role: RepoRole::Home,
                tags: vec!["equanimitech".into(), "personal".into()],
            },
        ]
    }

    #[test]
    fn role_parse_roundtrips() {
        assert_eq!(RepoRole::parse("project").unwrap(), RepoRole::Project);
        assert_eq!(RepoRole::parse("home").unwrap(), RepoRole::Home);
        assert!(RepoRole::parse("nope").is_err());
        assert_eq!(RepoRole::Home.as_str(), "home");
    }

    #[test]
    fn role_defaults_to_project() {
        assert_eq!(RepoRole::default(), RepoRole::Project);
    }

    #[test]
    fn with_tag_filters() {
        let e = entries();
        let reg = RepoRegistry::new(&e);
        let hits: Vec<_> = reg.with_tag("themia").collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, PathBuf::from("/a/themia"));
        assert_eq!(reg.with_tag("nope").count(), 0);
    }

    #[test]
    fn home_repos_filters_by_role() {
        let e = entries();
        let reg = RepoRegistry::new(&e);
        let homes: Vec<_> = reg.home_repos().collect();
        assert_eq!(homes.len(), 1);
        assert_eq!(homes[0].path, PathBuf::from("/a/knowledge"));
    }

    #[test]
    fn find_by_path() {
        let e = entries();
        let reg = RepoRegistry::new(&e);
        assert!(reg.find(Path::new("/a/themia")).is_some());
        assert!(reg.find(Path::new("/a/missing")).is_none());
    }
}
