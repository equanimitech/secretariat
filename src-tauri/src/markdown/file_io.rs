//! Atomic read/write of markdown files with sha256-based optimistic locking.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub struct ReadResult {
    pub content: String,
    pub sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("not utf-8")]
    NotUtf8,
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("file changed on disk")]
    Conflict { current_sha256: String },
}

pub fn read_file(path: &Path) -> Result<ReadResult, ReadError> {
    let bytes = fs::read(path)?;
    let content = String::from_utf8(bytes).map_err(|_| ReadError::NotUtf8)?;
    let sha256 = hash(content.as_bytes());
    Ok(ReadResult { content, sha256 })
}

pub fn write_file(
    path: &Path,
    new_content: &str,
    expected_sha256: &str,
) -> Result<String, WriteError> {
    if path.exists() {
        let current = fs::read(path)?;
        let current_sha = hash(&current);
        if current_sha != expected_sha256 {
            return Err(WriteError::Conflict {
                current_sha256: current_sha,
            });
        }
    }
    let tmp = path.with_extension("md.tmp");
    fs::write(&tmp, new_content.as_bytes())?;
    fs::rename(&tmp, path)?;
    Ok(hash(new_content.as_bytes()))
}

fn hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn read_returns_content_and_sha256() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a.md");
        std::fs::write(&path, b"hello world").unwrap();
        let result = read_file(&path).unwrap();
        assert_eq!(result.content, "hello world");
        assert_eq!(result.sha256.len(), 64);
    }

    #[test]
    fn write_succeeds_when_expected_sha_matches() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("b.md");
        std::fs::write(&path, b"v1").unwrap();
        let first = read_file(&path).unwrap();
        let new_sha = write_file(&path, "v2", &first.sha256).unwrap();
        assert_ne!(new_sha, first.sha256);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "v2");
    }

    #[test]
    fn write_rejects_when_disk_sha_diverged() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("c.md");
        std::fs::write(&path, b"orig").unwrap();
        let stale = "0".repeat(64);
        let err = write_file(&path, "new", &stale).unwrap_err();
        assert!(matches!(err, WriteError::Conflict { .. }));
    }
}
