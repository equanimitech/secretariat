//! Use cases for triaging an inbox envelope during a review session.
//!
//! Three actions move the file out of the active inbox; one (reply) is
//! handled outside this module (composer creates a new outbox draft).
//!
//! - **Defer** (remind me later) — move to `inbox/deferred/`. Future bubble-up
//!   logic surfaces these back; v1 just stages them out of the way.
//! - **Archive** (ignore / handled) — move to `inbox/archived/`. Out of the
//!   active queue, kept on disk for history.
//!
//! See `docs/ideas/two-buttons-cadenced-reviews.md` and
//! `docs/ideas/bubble-up-like-hey.md` for design context.

use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum InboxActionError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("not an inbox file: {path}")]
    NotInInbox { path: PathBuf },
    #[error("envelope file not found: {path}")]
    NotFound { path: PathBuf },
}

/// Move an envelope out of the active inbox into the deferred subfolder.
/// Idempotent against the destination — if a file with the same name
/// already exists in `deferred/`, the existing one is overwritten (the
/// principal is re-deferring, that's fine).
pub fn defer_envelope(file_path: &Path, inbox_root: &Path) -> Result<PathBuf, InboxActionError> {
    move_into_subdir(file_path, inbox_root, "deferred")
}

/// Move an envelope out of the active inbox into the archived subfolder.
/// Same idempotency semantics as `defer_envelope`.
pub fn archive_envelope(file_path: &Path, inbox_root: &Path) -> Result<PathBuf, InboxActionError> {
    move_into_subdir(file_path, inbox_root, "archived")
}

fn move_into_subdir(
    file_path: &Path,
    inbox_root: &Path,
    subdir_name: &str,
) -> Result<PathBuf, InboxActionError> {
    if !file_path.exists() {
        return Err(InboxActionError::NotFound {
            path: file_path.to_path_buf(),
        });
    }

    // The file must live directly under `inbox_root` (not already in a subdir).
    let parent = file_path
        .parent()
        .ok_or_else(|| InboxActionError::NotInInbox {
            path: file_path.to_path_buf(),
        })?;
    let parent_canon = parent.canonicalize().map_err(|e| InboxActionError::Io {
        path: parent.to_path_buf(),
        source: e,
    })?;
    let inbox_canon = inbox_root.canonicalize().map_err(|e| InboxActionError::Io {
        path: inbox_root.to_path_buf(),
        source: e,
    })?;
    if parent_canon != inbox_canon {
        return Err(InboxActionError::NotInInbox {
            path: file_path.to_path_buf(),
        });
    }

    let dest_dir = inbox_root.join(subdir_name);
    std::fs::create_dir_all(&dest_dir).map_err(|e| InboxActionError::Io {
        path: dest_dir.clone(),
        source: e,
    })?;
    let file_name = file_path
        .file_name()
        .ok_or_else(|| InboxActionError::NotInInbox {
            path: file_path.to_path_buf(),
        })?;
    let dest = dest_dir.join(file_name);
    std::fs::rename(file_path, &dest).map_err(|e| InboxActionError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_envelope(dir: &Path, name: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let p = dir.join(name);
        fs::write(&p, "---\nbody\n---\n").unwrap();
        p
    }

    #[test]
    fn defer_moves_file_to_deferred_subdir() {
        let dir = TempDir::new().unwrap();
        let inbox = dir.path().join("inbox");
        fs::create_dir_all(&inbox).unwrap();
        let envelope = write_envelope(&inbox, "test.md");

        let dest = defer_envelope(&envelope, &inbox).unwrap();
        assert!(!envelope.exists());
        assert!(dest.exists());
        assert!(dest.starts_with(inbox.join("deferred")));
    }

    #[test]
    fn archive_moves_file_to_archived_subdir() {
        let dir = TempDir::new().unwrap();
        let inbox = dir.path().join("inbox");
        fs::create_dir_all(&inbox).unwrap();
        let envelope = write_envelope(&inbox, "test.md");

        let dest = archive_envelope(&envelope, &inbox).unwrap();
        assert!(!envelope.exists());
        assert!(dest.exists());
        assert!(dest.starts_with(inbox.join("archived")));
    }

    #[test]
    fn rejects_files_already_in_subdir() {
        let dir = TempDir::new().unwrap();
        let inbox = dir.path().join("inbox");
        let deferred = inbox.join("deferred");
        fs::create_dir_all(&deferred).unwrap();
        let envelope = write_envelope(&deferred, "already.md");

        let err = defer_envelope(&envelope, &inbox).unwrap_err();
        assert!(matches!(err, InboxActionError::NotInInbox { .. }));
    }

    #[test]
    fn rejects_missing_files() {
        let dir = TempDir::new().unwrap();
        let inbox = dir.path().join("inbox");
        fs::create_dir_all(&inbox).unwrap();
        let phantom = inbox.join("phantom.md");

        let err = archive_envelope(&phantom, &inbox).unwrap_err();
        assert!(matches!(err, InboxActionError::NotFound { .. }));
    }
}
