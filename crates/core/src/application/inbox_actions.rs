//! Use cases for triaging an inbox envelope during a review session.
//!
//! Two actions move the file out of the active envelopes/ tree but
//! keep it on disk for history. Both infer the *queue directory*
//! (the parent of `envelopes/`) by walking up the file path, then
//! move the file to `<queue-dir>/deferred/` or `<queue-dir>/archived/`.
//!
//! - **Defer** (remind me later) — move to `<queue-dir>/deferred/`. Future
//!   bubble-up logic surfaces these back; v1 just stages them out of the way.
//! - **Archive** (ignore / handled) — move to `<queue-dir>/archived/`.
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
    #[error("not an envelope under any queue: {path}")]
    NotInQueue { path: PathBuf },
    #[error("envelope file not found: {path}")]
    NotFound { path: PathBuf },
}

/// Move an envelope from `<queue-dir>/envelopes/.../<file>.md` into
/// `<queue-dir>/deferred/<file>.md`. Idempotent against the
/// destination — re-deferring overwrites a previous deferred file
/// with the same basename.
pub fn defer_envelope(file_path: &Path) -> Result<PathBuf, InboxActionError> {
    move_into_queue_subdir(file_path, "deferred")
}

/// Move an envelope from `<queue-dir>/envelopes/.../<file>.md` into
/// `<queue-dir>/archived/<file>.md`. Same idempotency as `defer_envelope`.
pub fn archive_envelope(file_path: &Path) -> Result<PathBuf, InboxActionError> {
    move_into_queue_subdir(file_path, "archived")
}

/// Reverse of `archive_envelope` — move an envelope from
/// `<queue-dir>/archived/<file>.md` back into
/// `<queue-dir>/envelopes/<file>.md` (flat; the original date shard is
/// not reconstructed — the daemon's inbox writer is the only path that
/// owns date-sharding semantics, and round-tripping here would require
/// re-reading frontmatter the use case otherwise doesn't touch).
///
/// `NotInQueue` if the file isn't under an `archived/` ancestor.
pub fn unarchive_envelope(file_path: &Path) -> Result<PathBuf, InboxActionError> {
    if !file_path.exists() {
        return Err(InboxActionError::NotFound {
            path: file_path.to_path_buf(),
        });
    }
    let queue_dir = find_queue_dir_from(file_path, "archived")?;
    let dest_dir = queue_dir.join("envelopes");
    std::fs::create_dir_all(&dest_dir).map_err(|e| InboxActionError::Io {
        path: dest_dir.clone(),
        source: e,
    })?;
    let file_name = file_path
        .file_name()
        .ok_or_else(|| InboxActionError::NotInQueue {
            path: file_path.to_path_buf(),
        })?;
    let dest = dest_dir.join(file_name);
    std::fs::rename(file_path, &dest).map_err(|e| InboxActionError::Io {
        path: file_path.to_path_buf(),
        source: e,
    })?;
    Ok(dest)
}

/// Walk up the path looking for an ancestor named `envelopes`; the
/// parent of that ancestor is the queue directory. Returns
/// `NotInQueue` if the file isn't under an `envelopes/` subtree.
fn find_queue_dir(file_path: &Path) -> Result<PathBuf, InboxActionError> {
    find_queue_dir_from(file_path, "envelopes")
}

/// Generalized queue-dir resolver — walks ancestors until it finds a
/// directory with the given `marker` name, and returns its parent.
fn find_queue_dir_from(file_path: &Path, marker: &str) -> Result<PathBuf, InboxActionError> {
    let mut current = file_path.parent();
    while let Some(p) = current {
        if p.file_name().and_then(|n| n.to_str()) == Some(marker) {
            if let Some(q) = p.parent() {
                return Ok(q.to_path_buf());
            }
        }
        current = p.parent();
    }
    Err(InboxActionError::NotInQueue {
        path: file_path.to_path_buf(),
    })
}

fn move_into_queue_subdir(
    file_path: &Path,
    subdir_name: &str,
) -> Result<PathBuf, InboxActionError> {
    if !file_path.exists() {
        return Err(InboxActionError::NotFound {
            path: file_path.to_path_buf(),
        });
    }
    let queue_dir = find_queue_dir(file_path)?;
    let dest_dir = queue_dir.join(subdir_name);
    std::fs::create_dir_all(&dest_dir).map_err(|e| InboxActionError::Io {
        path: dest_dir.clone(),
        source: e,
    })?;
    let file_name = file_path
        .file_name()
        .ok_or_else(|| InboxActionError::NotInQueue {
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
    fn defer_moves_file_to_queue_deferred_subdir() {
        let dir = TempDir::new().unwrap();
        let queue = dir.path().join("channels/inbox/default");
        let active = queue.join("envelopes/2026/05/12");
        fs::create_dir_all(&active).unwrap();
        let envelope = write_envelope(&active, "test.md");

        let dest = defer_envelope(&envelope).unwrap();
        assert!(!envelope.exists());
        assert!(dest.exists());
        assert!(dest.starts_with(queue.join("deferred")));
    }

    #[test]
    fn archive_moves_file_to_queue_archived_subdir() {
        let dir = TempDir::new().unwrap();
        let queue = dir.path().join("channels/inbox/default");
        let active = queue.join("envelopes/2026/05/12");
        fs::create_dir_all(&active).unwrap();
        let envelope = write_envelope(&active, "test.md");

        let dest = archive_envelope(&envelope).unwrap();
        assert!(!envelope.exists());
        assert!(dest.exists());
        assert!(dest.starts_with(queue.join("archived")));
    }

    #[test]
    fn unarchive_moves_file_back_to_envelopes_dir() {
        let dir = TempDir::new().unwrap();
        let queue = dir.path().join("channels/inbox/default");
        let archived = queue.join("archived");
        fs::create_dir_all(&archived).unwrap();
        let envelope = write_envelope(&archived, "test.md");

        let dest = unarchive_envelope(&envelope).unwrap();
        assert!(!envelope.exists());
        assert!(dest.exists());
        assert_eq!(dest, queue.join("envelopes/test.md"));
    }

    #[test]
    fn unarchive_rejects_files_not_under_archived_dir() {
        let dir = TempDir::new().unwrap();
        let queue = dir.path().join("channels/inbox/default");
        let active = queue.join("envelopes/2026/05/12");
        fs::create_dir_all(&active).unwrap();
        let envelope = write_envelope(&active, "test.md");

        let err = unarchive_envelope(&envelope).unwrap_err();
        assert!(matches!(err, InboxActionError::NotInQueue { .. }));
    }

    #[test]
    fn rejects_files_not_under_any_envelopes_dir() {
        let dir = TempDir::new().unwrap();
        let stray = dir.path().join("stray-dir");
        fs::create_dir_all(&stray).unwrap();
        let envelope = write_envelope(&stray, "loose.md");

        let err = defer_envelope(&envelope).unwrap_err();
        assert!(matches!(err, InboxActionError::NotInQueue { .. }));
    }

    #[test]
    fn rejects_missing_files() {
        let dir = TempDir::new().unwrap();
        let phantom = dir
            .path()
            .join("channels/inbox/default/envelopes/2026/05/12/phantom.md");

        let err = archive_envelope(&phantom).unwrap_err();
        assert!(matches!(err, InboxActionError::NotFound { .. }));
    }
}
