//! Per-channel binding — a leaf-only mapping from a channel to a host
//! filesystem directory, plus optional overrides for which cognition
//! substrate `sec launch` invokes inside that directory.
//!
//! Lives in `contract.local.md` frontmatter alongside [`ChannelContract`]
//! but does NOT participate in the accumulate merge: bindings are
//! per-device, per-principal, and never inherited from ancestors. The
//! contract's [[project_filesystem_authoritative]] stance keeps the
//! channel-dir authoritative — `root_path` only changes *where on disk*
//! that channel-dir lives, not what it contains.
//!
//! The `launch_*` fields layer on top of the global `[cognition]`
//! preferences when set. Lets a "journals" channel route to a local
//! LM Studio endpoint while everything else stays on Claude Code's
//! default backend, without forking the launcher.

use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChannelBinding {
    /// Absolute host path the channel-dir should resolve to. `None` =
    /// fall through to the default `<root>/<alias>/<handle-segments>/`.
    pub root_path: Option<PathBuf>,

    /// Overrides `preferences.cognition.launch_command` when set.
    pub launch_command: Option<String>,

    /// Replaces `preferences.cognition.launch_args` when non-empty.
    /// (`Vec` not `Option<Vec>` because empty == "use the default";
    /// the round-trip uses `skip_serializing_if = is_empty`.)
    pub launch_args: Vec<String>,

    /// Merged on top of `preferences.cognition.launch_env`. Keys
    /// present here override the global; keys absent are inherited.
    pub launch_env: BTreeMap<String, String>,
}

impl ChannelBinding {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.root_path.is_none()
            && self.launch_command.is_none()
            && self.launch_args.is_empty()
            && self.launch_env.is_empty()
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
            ..Default::default()
        };
        assert!(!b.is_empty());
    }

    #[test]
    fn any_launch_override_makes_not_empty() {
        let mut b = ChannelBinding::empty();
        b.launch_env
            .insert("ANTHROPIC_BASE_URL".into(), "http://localhost:1234".into());
        assert!(!b.is_empty());
    }
}
