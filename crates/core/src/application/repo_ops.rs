//! Repo-registry use case: enroll / list / unenroll repos in the substrate
//! manifest (`preferences.toml` `[[repos]]`). Pure orchestration; IO is
//! `Preferences::load` / `save` + a `.git/` existence check.
//!
//! `path` is identity: `register_repo` upserts (updates role/tags on an
//! existing path, never duplicates). Paths are canonicalized to absolute so
//! `sec repo add .` and an absolute re-add resolve to one entry.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::infrastructure::preferences::{Preferences, PreferencesError};
use crate::infrastructure::repo_registry::{RepoEntry, RepoRole};

#[derive(Debug, Error)]
pub enum RepoOpsError {
    #[error("not a git repo: {path} — run `git init` there first")]
    NotAGitRepo { path: PathBuf },
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Preferences(#[from] PreferencesError),
}

/// Enroll (or update) a repo. Canonicalizes `repo_path`, requires it contain
/// `.git/`, then upserts by path and saves.
pub fn register_repo(
    prefs_path: &Path,
    repo_path: &Path,
    role: RepoRole,
    tags: Vec<String>,
) -> Result<RepoEntry, RepoOpsError> {
    let abs = std::fs::canonicalize(repo_path).map_err(|source| RepoOpsError::Io {
        path: repo_path.to_path_buf(),
        source,
    })?;
    // `.exists()` is intentional: true for both a regular clone (`.git` dir)
    // and a worktree / submodule (`.git` file). Do not narrow to `is_dir()`.
    if !abs.join(".git").exists() {
        return Err(RepoOpsError::NotAGitRepo { path: abs });
    }

    let mut prefs = Preferences::load(prefs_path)?;
    let entry = RepoEntry {
        path: abs.clone(),
        role,
        tags,
    };
    if let Some(existing) = prefs.repos.iter_mut().find(|e| e.path == abs) {
        *existing = entry.clone();
    } else {
        prefs.repos.push(entry.clone());
    }
    prefs.save(prefs_path)?;
    Ok(entry)
}

/// List enrolled repos, optionally filtered to those carrying `tag`.
pub fn list_repos(
    prefs_path: &Path,
    tag_filter: Option<&str>,
) -> Result<Vec<RepoEntry>, RepoOpsError> {
    let prefs = Preferences::load(prefs_path)?;
    let out = match tag_filter {
        Some(tag) => prefs.registry().with_tag(tag).cloned().collect(),
        None => prefs.repos.clone(),
    };
    Ok(out)
}

/// Unenroll a repo by path. Canonicalizes first so `.` matches the stored
/// absolute path. Returns `false` if nothing matched.
pub fn unregister_repo(prefs_path: &Path, repo_path: &Path) -> Result<bool, RepoOpsError> {
    // Best-effort canonicalize: if the dir is gone we can still remove a
    // stale entry by its literal path.
    let target = std::fs::canonicalize(repo_path).unwrap_or_else(|_| repo_path.to_path_buf());
    let mut prefs = Preferences::load(prefs_path)?;
    let before = prefs.repos.len();
    prefs.repos.retain(|e| e.path != target);
    let removed = prefs.repos.len() != before;
    if removed {
        prefs.save(prefs_path)?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A tempdir that is a git repo (has `.git/`), plus the prefs path.
    fn repo_and_prefs() -> (TempDir, PathBuf, PathBuf) {
        let d = TempDir::new().unwrap();
        let repo = d.path().join("themia");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let prefs = d.path().join("preferences.toml");
        (d, repo, prefs)
    }

    #[test]
    fn register_canonicalizes_and_appends() {
        let (_d, repo, prefs) = repo_and_prefs();
        let entry = register_repo(&prefs, &repo, RepoRole::Project, vec!["themia".into()]).unwrap();
        assert!(entry.path.is_absolute());
        assert_eq!(entry.role, RepoRole::Project);
        let listed = list_repos(&prefs, None).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].tags, vec!["themia".to_string()]);
    }

    #[test]
    fn register_upserts_on_duplicate_path() {
        let (_d, repo, prefs) = repo_and_prefs();
        register_repo(&prefs, &repo, RepoRole::Project, vec!["a".into()]).unwrap();
        register_repo(&prefs, &repo, RepoRole::Home, vec!["b".into()]).unwrap();
        let listed = list_repos(&prefs, None).unwrap();
        assert_eq!(listed.len(), 1, "upsert, not duplicate");
        assert_eq!(listed[0].role, RepoRole::Home);
        assert_eq!(listed[0].tags, vec!["b".to_string()]);
    }

    #[test]
    fn register_rejects_non_git_dir() {
        let d = TempDir::new().unwrap();
        let plain = d.path().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let prefs = d.path().join("preferences.toml");
        let err = register_repo(&prefs, &plain, RepoRole::Project, vec![]).unwrap_err();
        assert!(matches!(err, RepoOpsError::NotAGitRepo { .. }));
    }

    #[test]
    fn list_filters_by_tag() {
        let d = TempDir::new().unwrap();
        let prefs = d.path().join("preferences.toml");
        for (name, tag) in [("themia", "themia"), ("zen", "equanimitech")] {
            let r = d.path().join(name);
            std::fs::create_dir_all(r.join(".git")).unwrap();
            register_repo(&prefs, &r, RepoRole::Project, vec![tag.into()]).unwrap();
        }
        assert_eq!(list_repos(&prefs, Some("themia")).unwrap().len(), 1);
        assert_eq!(list_repos(&prefs, Some("equanimitech")).unwrap().len(), 1);
        assert_eq!(list_repos(&prefs, None).unwrap().len(), 2);
    }

    #[test]
    fn unregister_removes_and_reports() {
        let (_d, repo, prefs) = repo_and_prefs();
        register_repo(&prefs, &repo, RepoRole::Project, vec![]).unwrap();
        assert!(unregister_repo(&prefs, &repo).unwrap());
        assert!(list_repos(&prefs, None).unwrap().is_empty());
        // Second remove is a no-op.
        assert!(!unregister_repo(&prefs, &repo).unwrap());
    }
}
