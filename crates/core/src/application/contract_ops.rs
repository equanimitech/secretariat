//! Use cases for reading/writing per-principal consumption contracts.
//!
//! Per [[project-consumption-vs-governance]]: `contract.local.md` files at
//! `<channel-dir>/contract.local.md` (and `<org-dir>/contract.local.md`) carry
//! the subscriber's own consumption preferences — cadence floors,
//! min-trust filters. Private per principal, never shared.
//!
//! Two pairs of verbs, mirroring CLI/MCP surface:
//! - `get_channel_contract` / `set_channel_contract`
//! - `get_org_contract` / `set_org_contract`
//!
//! Set verbs apply a `ContractPatch` over the existing contract:
//! `Leave` keeps the current value, `Set(v)` writes `v`, `Clear`
//! reverts to `None` (inherit from ancestors). Body prose is
//! preserved across set calls — the patch only touches frontmatter.
//!
//! Accumulate resolver lives in a sibling slice; this module is
//! own-only reads/writes.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::domain::{ChannelContract, OrgAlias, QueueHandle, TrustGate};
use crate::infrastructure::channel_def_store::{
    channel_def_path, ChannelDefStoreError,
};
use crate::infrastructure::contract_store::{
    channel_contract_path, load_contract, org_contract_path, save_contract,
    save_stub_if_absent, ContractStoreError,
};
use crate::infrastructure::org_store::{org_dir, org_metadata_path};

#[derive(Debug, Error)]
pub enum ContractOpsError {
    #[error("channel `{0}` does not exist — create it first")]
    ChannelNotFound(String),
    #[error("org `{0}` does not exist — create it first")]
    OrgNotFound(String),
    #[error("contract store: {0}")]
    ContractStore(#[from] ContractStoreError),
    #[error("channel def store: {0}")]
    ChannelDefStore(#[from] ChannelDefStoreError),
}

/// Tristate update for a contract field.
///
/// Distinguishes "leave alone" (no-op) from "clear to None" (revert
/// to inheriting from ancestors). Without this, a partial patch
/// can't differentiate "I'm not touching this field" from "I'm
/// explicitly removing this field" — both look like `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PatchField<T> {
    /// Keep the current on-disk value untouched.
    #[default]
    Leave,
    /// Replace with the given value.
    Set(T),
    /// Clear back to `None` — inherit from ancestors instead.
    Clear,
}

impl<T: Clone> PatchField<T> {
    /// Apply this patch over a current `Option<T>`.
    pub fn apply(&self, current: Option<T>) -> Option<T> {
        match self {
            PatchField::Leave => current,
            PatchField::Set(v) => Some(v.clone()),
            PatchField::Clear => None,
        }
    }
}

/// Partial update to a contract's frontmatter fields. Body prose is
/// preserved separately.
#[derive(Debug, Clone, Default)]
pub struct ContractPatch {
    pub cadence_floor_minutes: PatchField<u32>,
    pub min_trust: PatchField<TrustGate>,
}

impl ContractPatch {
    pub fn is_noop(&self) -> bool {
        matches!(self.cadence_floor_minutes, PatchField::Leave)
            && matches!(self.min_trust, PatchField::Leave)
    }

    fn apply(&self, current: ChannelContract) -> ChannelContract {
        ChannelContract {
            cadence_floor_minutes: self.cadence_floor_minutes.apply(current.cadence_floor_minutes),
            min_trust: self.min_trust.apply(current.min_trust),
        }
    }
}

/// Loaded contract + the body prose that surrounds the frontmatter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractView {
    pub contract: ChannelContract,
    pub body: String,
    pub path: PathBuf,
}

/// One level visited by the accumulate resolver. Levels are emitted in
/// walk order (org-root first, leaf last).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractLevel {
    /// Human-readable label, e.g. `org:themia.pro`, `dev`,
    /// `dev:leggia`.
    pub scope: String,
    pub path: PathBuf,
    pub contract: ChannelContract,
}

/// Merged contract plus the chain of levels that contributed to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContract {
    pub merged: ChannelContract,
    pub chain: Vec<ContractLevel>,
}

// -- channel ------------------------------------------------------------------

pub fn get_channel_contract(
    channels_root: &Path,
    handle: &QueueHandle,
) -> Result<ContractView, ContractOpsError> {
    if !channel_def_path(channels_root, handle).is_file() {
        return Err(ContractOpsError::ChannelNotFound(handle.as_str().to_string()));
    }
    let path = channel_contract_path(channels_root, handle);
    let (contract, body) = match load_contract(&path)? {
        Some((c, b)) => (c, b),
        None => {
            // Channel exists but lacks a contract.local.md (created before
            // slice 1a shipped, or hand-deleted). Auto-scaffold the
            // stub so the principal isn't surprised by a missing file
            // when they go to read it.
            save_stub_if_absent(&path, None)?;
            load_contract(&path)?.expect("stub just written")
        }
    };
    Ok(ContractView {
        contract,
        body,
        path,
    })
}

pub fn set_channel_contract(
    channels_root: &Path,
    handle: &QueueHandle,
    patch: ContractPatch,
) -> Result<ContractView, ContractOpsError> {
    let current = get_channel_contract(channels_root, handle)?;
    let new_contract = patch.apply(current.contract);
    save_contract(&current.path, &new_contract, &current.body, true)?;
    Ok(ContractView {
        contract: new_contract,
        body: current.body,
        path: current.path,
    })
}

// -- org ----------------------------------------------------------------------

pub fn get_org_contract(
    orgs_root: &Path,
    alias: &OrgAlias,
) -> Result<ContractView, ContractOpsError> {
    let org_dir = org_dir(orgs_root, alias);
    if !org_metadata_path(orgs_root, alias).is_file() {
        return Err(ContractOpsError::OrgNotFound(alias.as_str().to_string()));
    }
    let path = org_contract_path(&org_dir);
    let (contract, body) = match load_contract(&path)? {
        Some((c, b)) => (c, b),
        None => {
            save_stub_if_absent(&path, None)?;
            load_contract(&path)?.expect("stub just written")
        }
    };
    Ok(ContractView {
        contract,
        body,
        path,
    })
}

pub fn set_org_contract(
    orgs_root: &Path,
    alias: &OrgAlias,
    patch: ContractPatch,
) -> Result<ContractView, ContractOpsError> {
    let current = get_org_contract(orgs_root, alias)?;
    let new_contract = patch.apply(current.contract);
    save_contract(&current.path, &new_contract, &current.body, true)?;
    Ok(ContractView {
        contract: new_contract,
        body: current.body,
        path: current.path,
    })
}

// -- accumulate resolver ------------------------------------------------------

/// Walk a principal's contract chain and return the merged view per
/// [[project-contracts-accumulate]]:
///
/// 1. Org-root `<orgs_root>/<alias>/contract.local.md` (if `org` is
///    `Some` and the file exists).
/// 2. Each intermediate channel-segment directory in turn (e.g. for
///    `dev:leggia`, load `<channels_root>/dev/contract.local.md`
///    if present).
/// 3. The leaf channel's `contract.local.md`.
///
/// Merge rules per consumption-shape field:
/// - `cadence_floor_minutes` → **MAX** (the tightest floor along the
///   chain wins — children can narrow further, never widen).
/// - `min_trust` → **MAX-RESTRICTIVE**
///   (`stamp-required` dominates `signed-only`).
///
/// Missing files are silently skipped (they contribute nothing).
/// Personal channels (where `org` is `None`) skip step 1 and walk
/// `<channels_root>/...` directly.
///
/// Errors when a loaded file fails to parse — those are bugs in the
/// substrate, not inheritance edge-cases.
pub fn resolve_channel_contract(
    orgs_root: &Path,
    personal_channels_root: &Path,
    org: Option<&OrgAlias>,
    handle: &QueueHandle,
) -> Result<ResolvedContract, ContractOpsError> {
    let mut chain: Vec<ContractLevel> = Vec::new();
    let mut merged = ChannelContract::empty();

    // 1. Org-root level (only if scoping inside an org).
    let (channels_root, org_label) = match org {
        Some(alias) => {
            let org_root_dir = org_dir(orgs_root, alias);
            if !org_metadata_path(orgs_root, alias).is_file() {
                return Err(ContractOpsError::OrgNotFound(alias.as_str().to_string()));
            }
            let path = org_contract_path(&org_root_dir);
            if let Some((c, _)) = load_contract(&path)? {
                merge_into(&mut merged, &c);
                chain.push(ContractLevel {
                    scope: format!("org:{}", alias.as_str()),
                    path,
                    contract: c,
                });
            }
            (org_root_dir.join("channels"), Some(alias.as_str().to_string()))
        }
        None => (personal_channels_root.to_path_buf(), None),
    };

    // 2 + 3. Each prefix of the handle's segments contributes if a
    //        `contract.local.md` exists at that directory. v0.5 handles
    //        no longer carry a `channel:` token, so every segment is
    //        directory depth.
    let segs: Vec<&str> = handle.segments();
    let mut cursor = channels_root.clone();
    let mut handle_so_far = String::new();
    for seg in &segs {
        cursor.push(seg);
        if !handle_so_far.is_empty() {
            handle_so_far.push(':');
        }
        handle_so_far.push_str(seg);
        let path = cursor.join(crate::infrastructure::contract_store::CONTRACT_FILENAME);
        if let Some((c, _)) = load_contract(&path)? {
            merge_into(&mut merged, &c);
            chain.push(ContractLevel {
                scope: handle_so_far.clone(),
                path,
                contract: c,
            });
        }
    }

    // For personal contexts, validate the leaf channel actually exists
    // (parity with `get_channel_contract`'s ChannelNotFound guard).
    if org_label.is_none() {
        let leaf_def = crate::infrastructure::channel_def_store::channel_def_path(
            personal_channels_root,
            handle,
        );
        if !leaf_def.is_file() {
            return Err(ContractOpsError::ChannelNotFound(
                handle.as_str().to_string(),
            ));
        }
    } else {
        // Inside an org: same guard but scoped under the org's channels root.
        let leaf_def = crate::infrastructure::channel_def_store::channel_def_path(
            &channels_root,
            handle,
        );
        if !leaf_def.is_file() {
            return Err(ContractOpsError::ChannelNotFound(
                handle.as_str().to_string(),
            ));
        }
    }

    Ok(ResolvedContract { merged, chain })
}

fn merge_into(running: &mut ChannelContract, level: &ChannelContract) {
    running.cadence_floor_minutes = match (running.cadence_floor_minutes, level.cadence_floor_minutes) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };
    running.min_trust = match (running.min_trust, level.min_trust) {
        (Some(a), Some(b)) => Some(TrustGate::max_restrictive(a, b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{create_channel, create_org};
    use chrono::{TimeZone, Utc};
    use tempfile::TempDir;

    fn alias(s: &str) -> OrgAlias {
        OrgAlias::parse(s).unwrap()
    }

    fn when() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap()
    }

    #[test]
    fn get_channel_contract_returns_empty_stub_for_fresh_channel() {
        let dir = TempDir::new().unwrap();
        let channels = dir.path().join("channels");
        let h = QueueHandle::parse("dev:secretariat").unwrap();
        create_channel(&channels, h.clone(), "", "", when(), None).unwrap();
        let view = get_channel_contract(&channels, &h).unwrap();
        assert!(view.contract.is_empty());
        assert!(view.body.contains("# importance"));
    }

    #[test]
    fn get_channel_contract_errors_when_channel_missing() {
        let dir = TempDir::new().unwrap();
        let channels = dir.path().join("channels");
        let h = QueueHandle::parse("nothing").unwrap();
        let r = get_channel_contract(&channels, &h);
        assert!(matches!(r, Err(ContractOpsError::ChannelNotFound(_))));
    }

    #[test]
    fn set_channel_contract_writes_fields_and_preserves_body() {
        let dir = TempDir::new().unwrap();
        let channels = dir.path().join("channels");
        let h = QueueHandle::parse("dev:secretariat").unwrap();
        create_channel(&channels, h.clone(), "", "", when(), None).unwrap();
        let patch = ContractPatch {
            cadence_floor_minutes: PatchField::Set(45),
            min_trust: PatchField::Set(TrustGate::StampRequired),
        };
        let view = set_channel_contract(&channels, &h, patch).unwrap();
        assert_eq!(view.contract.cadence_floor_minutes, Some(45));
        assert_eq!(view.contract.min_trust, Some(TrustGate::StampRequired));
        // Body of the stub stays intact.
        assert!(view.body.contains("# importance"));
        // Re-reading yields the same state.
        let reread = get_channel_contract(&channels, &h).unwrap();
        assert_eq!(reread.contract, view.contract);
    }

    #[test]
    fn set_channel_contract_leaves_unmentioned_fields_alone() {
        let dir = TempDir::new().unwrap();
        let channels = dir.path().join("channels");
        let h = QueueHandle::parse("dev:secretariat").unwrap();
        create_channel(&channels, h.clone(), "", "", when(), None).unwrap();
        // First set: cadence + min_trust.
        set_channel_contract(
            &channels,
            &h,
            ContractPatch {
                cadence_floor_minutes: PatchField::Set(45),
                min_trust: PatchField::Set(TrustGate::SignedOnly),
            },
        )
        .unwrap();
        // Second set: only touch cadence; min_trust must stay.
        let view = set_channel_contract(
            &channels,
            &h,
            ContractPatch {
                cadence_floor_minutes: PatchField::Set(60),
                min_trust: PatchField::Leave,
            },
        )
        .unwrap();
        assert_eq!(view.contract.cadence_floor_minutes, Some(60));
        assert_eq!(view.contract.min_trust, Some(TrustGate::SignedOnly));
    }

    #[test]
    fn set_channel_contract_clear_reverts_to_inherit() {
        let dir = TempDir::new().unwrap();
        let channels = dir.path().join("channels");
        let h = QueueHandle::parse("dev:secretariat").unwrap();
        create_channel(&channels, h.clone(), "", "", when(), None).unwrap();
        set_channel_contract(
            &channels,
            &h,
            ContractPatch {
                cadence_floor_minutes: PatchField::Set(45),
                min_trust: PatchField::Set(TrustGate::StampRequired),
            },
        )
        .unwrap();
        let view = set_channel_contract(
            &channels,
            &h,
            ContractPatch {
                cadence_floor_minutes: PatchField::Clear,
                min_trust: PatchField::Leave,
            },
        )
        .unwrap();
        assert!(view.contract.cadence_floor_minutes.is_none());
        assert_eq!(view.contract.min_trust, Some(TrustGate::StampRequired));
    }

    #[test]
    fn get_org_contract_returns_stub_for_fresh_org() {
        let dir = TempDir::new().unwrap();
        let orgs = dir.path().join("orgs");
        let a = alias("themia.pro");
        create_org(&orgs, a.clone(), None, "", "", when(), None).unwrap();
        let view = get_org_contract(&orgs, &a).unwrap();
        assert!(view.contract.is_empty());
        assert!(view.body.contains("# importance"));
    }

    #[test]
    fn get_org_contract_errors_when_org_missing() {
        let dir = TempDir::new().unwrap();
        let orgs = dir.path().join("orgs");
        let r = get_org_contract(&orgs, &alias("missing.tld"));
        assert!(matches!(r, Err(ContractOpsError::OrgNotFound(_))));
    }

    #[test]
    fn set_org_contract_round_trips() {
        let dir = TempDir::new().unwrap();
        let orgs = dir.path().join("orgs");
        let a = alias("themia.pro");
        create_org(&orgs, a.clone(), None, "", "", when(), None).unwrap();
        let view = set_org_contract(
            &orgs,
            &a,
            ContractPatch {
                cadence_floor_minutes: PatchField::Set(15),
                min_trust: PatchField::Leave,
            },
        )
        .unwrap();
        assert_eq!(view.contract.cadence_floor_minutes, Some(15));
        let reread = get_org_contract(&orgs, &a).unwrap();
        assert_eq!(reread.contract, view.contract);
    }

    #[test]
    fn patch_is_noop_when_all_leave() {
        let p = ContractPatch::default();
        assert!(p.is_noop());
    }

    // -- resolver tests ------------------------------------------------------

    fn write_org_contract(orgs: &Path, a: &OrgAlias, c: ChannelContract) {
        let dir = crate::infrastructure::org_store::org_dir(orgs, a);
        std::fs::create_dir_all(&dir).unwrap();
        let path = org_contract_path(&dir);
        save_contract(&path, &c, "", true).unwrap();
    }

    fn write_dir_contract(channels_root: &Path, segs: &[&str], c: ChannelContract) {
        let mut p = channels_root.to_path_buf();
        for s in segs {
            p.push(s);
        }
        std::fs::create_dir_all(&p).unwrap();
        let path = p.join(crate::infrastructure::contract_store::CONTRACT_FILENAME);
        save_contract(&path, &c, "", true).unwrap();
    }

    #[test]
    fn resolve_returns_empty_when_no_levels_contribute() {
        let dir = TempDir::new().unwrap();
        let orgs = dir.path().join("orgs");
        let a = alias("themia.pro");
        create_org(&orgs, a.clone(), None, "", "", when(), None).unwrap();
        let h = QueueHandle::parse("dev:leggia").unwrap();
        let channels_in_org =
            crate::infrastructure::org_store::org_channels_root(&orgs, &a);
        create_channel(&channels_in_org, h.clone(), "", "", when(), None).unwrap();
        // create_org auto-writes a stub at org-root that is empty;
        // create_channel writes a stub at leaf that is empty.
        let r =
            resolve_channel_contract(&orgs, &dir.path().join("ignored"), Some(&a), &h).unwrap();
        assert!(r.merged.is_empty());
        // Chain: org-root + leaf stubs were both loaded as empty
        // contracts; they show up in the chain even if they contribute
        // nothing (they're real files).
        assert!(!r.chain.is_empty());
    }

    #[test]
    fn resolve_max_cadence_wins() {
        let dir = TempDir::new().unwrap();
        let orgs = dir.path().join("orgs");
        let a = alias("themia.pro");
        create_org(&orgs, a.clone(), None, "", "", when(), None).unwrap();
        let channels_in_org =
            crate::infrastructure::org_store::org_channels_root(&orgs, &a);
        let h = QueueHandle::parse("dev:leggia").unwrap();
        create_channel(&channels_in_org, h.clone(), "", "", when(), None).unwrap();

        // org=30, trunk=15, leaf=60 → MAX = 60
        write_org_contract(
            &orgs,
            &a,
            ChannelContract {
                cadence_floor_minutes: Some(30),
                min_trust: None,
            },
        );
        write_dir_contract(
            &channels_in_org,
            &["dev"],
            ChannelContract {
                cadence_floor_minutes: Some(15),
                min_trust: None,
            },
        );
        set_channel_contract(
            &channels_in_org,
            &h,
            ContractPatch {
                cadence_floor_minutes: PatchField::Set(60),
                min_trust: PatchField::Leave,
            },
        )
        .unwrap();

        let r =
            resolve_channel_contract(&orgs, &dir.path().join("ignored"), Some(&a), &h).unwrap();
        assert_eq!(r.merged.cadence_floor_minutes, Some(60));
        assert_eq!(r.chain.len(), 3);
        assert_eq!(r.chain[0].scope, "org:themia.pro");
        assert_eq!(r.chain[1].scope, "dev");
        assert_eq!(r.chain[2].scope, "dev:leggia");
    }

    #[test]
    fn resolve_max_restrictive_trust_wins() {
        let dir = TempDir::new().unwrap();
        let orgs = dir.path().join("orgs");
        let a = alias("themia.pro");
        create_org(&orgs, a.clone(), None, "", "", when(), None).unwrap();
        let channels_in_org =
            crate::infrastructure::org_store::org_channels_root(&orgs, &a);
        let h = QueueHandle::parse("assemblee_generale").unwrap();
        create_channel(&channels_in_org, h.clone(), "", "", when(), None).unwrap();

        // org = signed-only, leaf = stamp-required → MAX-RESTRICTIVE = stamp-required
        write_org_contract(
            &orgs,
            &a,
            ChannelContract {
                cadence_floor_minutes: None,
                min_trust: Some(TrustGate::SignedOnly),
            },
        );
        set_channel_contract(
            &channels_in_org,
            &h,
            ContractPatch {
                cadence_floor_minutes: PatchField::Leave,
                min_trust: PatchField::Set(TrustGate::StampRequired),
            },
        )
        .unwrap();

        let r =
            resolve_channel_contract(&orgs, &dir.path().join("ignored"), Some(&a), &h).unwrap();
        assert_eq!(r.merged.min_trust, Some(TrustGate::StampRequired));
    }

    #[test]
    fn resolve_inherits_from_ancestors_when_leaf_silent() {
        let dir = TempDir::new().unwrap();
        let orgs = dir.path().join("orgs");
        let a = alias("themia.pro");
        create_org(&orgs, a.clone(), None, "", "", when(), None).unwrap();
        let channels_in_org =
            crate::infrastructure::org_store::org_channels_root(&orgs, &a);
        let h = QueueHandle::parse("clients").unwrap();
        create_channel(&channels_in_org, h.clone(), "", "", when(), None).unwrap();

        write_org_contract(
            &orgs,
            &a,
            ChannelContract {
                cadence_floor_minutes: Some(15),
                min_trust: None,
            },
        );
        // Leaf left empty.
        let r =
            resolve_channel_contract(&orgs, &dir.path().join("ignored"), Some(&a), &h).unwrap();
        assert_eq!(r.merged.cadence_floor_minutes, Some(15));
        assert_eq!(r.merged.min_trust, None);
    }

    #[test]
    fn resolve_errors_when_channel_missing() {
        let dir = TempDir::new().unwrap();
        let orgs = dir.path().join("orgs");
        let a = alias("themia.pro");
        create_org(&orgs, a.clone(), None, "", "", when(), None).unwrap();
        let h = QueueHandle::parse("never:existed").unwrap();
        let r = resolve_channel_contract(&orgs, &dir.path().join("ignored"), Some(&a), &h);
        assert!(matches!(r, Err(ContractOpsError::ChannelNotFound(_))));
    }

    #[test]
    fn resolve_works_for_personal_channel() {
        let dir = TempDir::new().unwrap();
        let channels = dir.path().join("channels");
        let h = QueueHandle::parse("inbox-rules").unwrap();
        create_channel(&channels, h.clone(), "", "", when(), None).unwrap();
        set_channel_contract(
            &channels,
            &h,
            ContractPatch {
                cadence_floor_minutes: PatchField::Set(120),
                min_trust: PatchField::Leave,
            },
        )
        .unwrap();
        let r = resolve_channel_contract(&dir.path().join("orgs"), &channels, None, &h).unwrap();
        assert_eq!(r.merged.cadence_floor_minutes, Some(120));
    }
}
