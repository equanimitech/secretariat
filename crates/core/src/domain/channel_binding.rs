//! Per-channel binding — a leaf-only mapping from a channel to a host
//! filesystem directory (typically a git repo).
//!
//! Lives in `contract.local.md` frontmatter alongside [`ChannelContract`]
//! but does NOT participate in the accumulate merge: bindings are
//! per-device, per-principal, and never inherited from ancestors. The
//! contract's [[project_filesystem_authoritative]] stance keeps the
//! channel-dir authoritative — `root_path` only changes *where on disk*
//! that channel-dir lives, not what it contains.

use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChannelBinding {
    /// Absolute host path the channel-dir should resolve to. `None` =
    /// fall through to the default `<root>/<alias>/<handle-segments>/`.
    pub root_path: Option<PathBuf>,
}

impl ChannelBinding {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.root_path.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_binding_is_empty() {
        assert!(ChannelBinding::empty().is_empty());
    }

    #[test]
    fn any_root_path_makes_not_empty() {
        let b = ChannelBinding {
            root_path: Some(PathBuf::from("/Users/rafa/Developer/secretariat")),
        };
        assert!(!b.is_empty());
    }
}
