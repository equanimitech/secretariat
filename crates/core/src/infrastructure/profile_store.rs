//! Local principal profile — the human-readable name + future presence
//! metadata that pairs with the cryptographic identity (DID).
//!
//! The DID is identity; the profile is *presence*. A bare DID is fine
//! for protocol-level addressing but unfriendly in any UI surface
//! ("you: did:key:z6Mk…"). This store gives the principal a place to
//! say "call me Rafa" — local-only, never sent over the wire.
//!
//! Stored at `~/.secretariat/profile.json`:
//!
//! ```json
//! {
//!   "version": 1,
//!   "display_name": "Rafa"
//! }
//! ```
//!
//! Versioned for forward-compat. Missing file → no profile yet (the UI
//! prompts during onboarding).

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::domain::DisplayName;

const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ProfileStoreError {
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed json: {0}")]
    MalformedJson(#[from] serde_json::Error),
    #[error("unsupported profile.json version {0} — upgrade Secretariat")]
    UnsupportedVersion(u32),
    #[error("invalid display_name: {0}")]
    InvalidName(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrincipalProfile {
    pub display_name: DisplayName,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProfileFile {
    version: u32,
    display_name: String,
}

/// Load the principal's profile. Returns `Ok(None)` when no profile has
/// been set yet (fresh install or pre-onboarding).
pub fn load_profile(path: &Path) -> Result<Option<PrincipalProfile>, ProfileStoreError> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path).map_err(|e| ProfileStoreError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let file: ProfileFile = serde_json::from_str(&raw)?;
    if file.version != CURRENT_VERSION {
        return Err(ProfileStoreError::UnsupportedVersion(file.version));
    }
    let display_name = DisplayName::parse(&file.display_name)
        .map_err(|e| ProfileStoreError::InvalidName(e.to_string()))?;
    Ok(Some(PrincipalProfile { display_name }))
}

/// Atomic save (write temp + rename) so a crash mid-write doesn't leave
/// a corrupted profile.
pub fn save_profile(path: &Path, profile: &PrincipalProfile) -> Result<(), ProfileStoreError> {
    let file = ProfileFile {
        version: CURRENT_VERSION,
        display_name: profile.display_name.to_string(),
    };
    let json = serde_json::to_string_pretty(&file)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, json).map_err(|e| ProfileStoreError::Io {
        path: tmp.display().to_string(),
        source: e,
    })?;
    std::fs::rename(&tmp, path).map_err(|e| ProfileStoreError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn missing_file_returns_none() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("profile.json");
        assert!(load_profile(&path).unwrap().is_none());
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("profile.json");
        let profile = PrincipalProfile {
            display_name: DisplayName::parse("Rafa").unwrap(),
        };
        save_profile(&path, &profile).unwrap();
        let loaded = load_profile(&path).unwrap().unwrap();
        assert_eq!(loaded, profile);
    }

    #[test]
    fn rejects_unknown_version() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("profile.json");
        std::fs::write(&path, r#"{"version": 99, "display_name": "X"}"#).unwrap();
        let err = load_profile(&path).unwrap_err();
        assert!(matches!(err, ProfileStoreError::UnsupportedVersion(99)));
    }

    #[test]
    fn rejects_invalid_name() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("profile.json");
        std::fs::write(&path, r#"{"version": 1, "display_name": ""}"#).unwrap();
        let err = load_profile(&path).unwrap_err();
        assert!(matches!(err, ProfileStoreError::InvalidName(_)));
    }
}
