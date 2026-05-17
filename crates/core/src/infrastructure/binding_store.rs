//! Single-channel binding resolution.
//!
//! `sec launch` and any future verb that consumes a `ChannelBinding`
//! does point-lookup: given the channel's default on-disk path, peek
//! at `contract.local.md` to see if `root_path` overrides it. No walker,
//! no cache — the binding-aware paths shipped today are all one-shot
//! resolutions where a single file read is cheaper than indexing the
//! substrate.
//!
//! A full [`BindingLookup`] that walks the substrate and indexes every
//! channel's binding lives in a follow-up slice (the `dispatch` path
//! per `docs/pitches/2026-05-13-launch-dispatch-root-path.md` needs it;
//! `launch` does not).

use std::path::{Path, PathBuf};

use crate::domain::ChannelBinding;
use crate::infrastructure::contract_store::{
    load_contract_with_binding, ContractStoreError, CONTRACT_FILENAME,
};

#[derive(Debug, thiserror::Error)]
pub enum BindingStoreError {
    #[error(transparent)]
    Contract(#[from] ContractStoreError),
}

/// Load the [`ChannelBinding`] persisted in `<default_path>/contract.local.md`.
/// Missing contract file or absent `root_path` → [`ChannelBinding::empty`].
pub fn load_channel_binding(default_path: &Path) -> Result<ChannelBinding, BindingStoreError> {
    let contract_path = default_path.join(CONTRACT_FILENAME);
    match load_contract_with_binding(&contract_path)? {
        Some((_contract, binding, _body)) => Ok(binding),
        None => Ok(ChannelBinding::empty()),
    }
}

/// Resolve the on-disk path for a channel-dir, honoring `root_path` when
/// set. `default_path` is the substrate's computed channel-dir (typically
/// from `queue_dir`); returns the bound path if the contract overrides
/// it, else the default unchanged.
pub fn resolve_channel_path(default_path: &Path) -> Result<PathBuf, BindingStoreError> {
    let binding = load_channel_binding(default_path)?;
    Ok(binding
        .root_path
        .unwrap_or_else(|| default_path.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn unbound_channel_resolves_to_default_path() {
        let tmp = TempDir::new().unwrap();
        let default = tmp.path().join("themia.pro/channel/dev/secretariat");
        std::fs::create_dir_all(&default).unwrap();
        assert_eq!(resolve_channel_path(&default).unwrap(), default);
    }

    #[test]
    fn missing_contract_file_resolves_to_default_path() {
        let tmp = TempDir::new().unwrap();
        let default = tmp.path().join("themia.pro/channel/dev/secretariat");
        std::fs::create_dir_all(&default).unwrap();
        // No contract.local.md present at all.
        assert_eq!(resolve_channel_path(&default).unwrap(), default);
    }

    #[test]
    fn root_path_overrides_default() {
        let tmp = TempDir::new().unwrap();
        let default = tmp.path().join("themia.pro/channel/dev/secretariat");
        std::fs::create_dir_all(&default).unwrap();
        let bound = tmp.path().join("repo");
        std::fs::create_dir_all(&bound).unwrap();
        std::fs::write(
            default.join(CONTRACT_FILENAME),
            format!("---\nroot_path: {}\n---\n", bound.display()),
        )
        .unwrap();
        assert_eq!(resolve_channel_path(&default).unwrap(), bound);
    }
}
