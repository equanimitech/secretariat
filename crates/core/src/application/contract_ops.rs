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
    #[error("handle `{0}` is not a channel handle (must start with `channel:`)")]
    NotAChannelHandle(String),
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

// -- channel ------------------------------------------------------------------

pub fn get_channel_contract(
    channels_root: &Path,
    handle: &QueueHandle,
) -> Result<ContractView, ContractOpsError> {
    if handle.top_namespace() != "channel" {
        return Err(ContractOpsError::NotAChannelHandle(
            handle.as_str().to_string(),
        ));
    }
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
            save_stub_if_absent(&path)?;
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
            save_stub_if_absent(&path)?;
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
        let h = QueueHandle::parse("channel:dev:secretariat").unwrap();
        create_channel(&channels, h.clone(), "", "", when()).unwrap();
        let view = get_channel_contract(&channels, &h).unwrap();
        assert!(view.contract.is_empty());
        assert!(view.body.contains("consumption contract"));
    }

    #[test]
    fn get_channel_contract_errors_when_channel_missing() {
        let dir = TempDir::new().unwrap();
        let channels = dir.path().join("channels");
        let h = QueueHandle::parse("channel:nothing").unwrap();
        let r = get_channel_contract(&channels, &h);
        assert!(matches!(r, Err(ContractOpsError::ChannelNotFound(_))));
    }

    #[test]
    fn set_channel_contract_writes_fields_and_preserves_body() {
        let dir = TempDir::new().unwrap();
        let channels = dir.path().join("channels");
        let h = QueueHandle::parse("channel:dev:secretariat").unwrap();
        create_channel(&channels, h.clone(), "", "", when()).unwrap();
        let patch = ContractPatch {
            cadence_floor_minutes: PatchField::Set(45),
            min_trust: PatchField::Set(TrustGate::StampRequired),
        };
        let view = set_channel_contract(&channels, &h, patch).unwrap();
        assert_eq!(view.contract.cadence_floor_minutes, Some(45));
        assert_eq!(view.contract.min_trust, Some(TrustGate::StampRequired));
        // Body of the stub stays intact.
        assert!(view.body.contains("consumption contract"));
        // Re-reading yields the same state.
        let reread = get_channel_contract(&channels, &h).unwrap();
        assert_eq!(reread.contract, view.contract);
    }

    #[test]
    fn set_channel_contract_leaves_unmentioned_fields_alone() {
        let dir = TempDir::new().unwrap();
        let channels = dir.path().join("channels");
        let h = QueueHandle::parse("channel:dev:secretariat").unwrap();
        create_channel(&channels, h.clone(), "", "", when()).unwrap();
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
        let h = QueueHandle::parse("channel:dev:secretariat").unwrap();
        create_channel(&channels, h.clone(), "", "", when()).unwrap();
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
        create_org(&orgs, a.clone(), None, "", "", when()).unwrap();
        let view = get_org_contract(&orgs, &a).unwrap();
        assert!(view.contract.is_empty());
        assert!(view.body.contains("consumption contract"));
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
        create_org(&orgs, a.clone(), None, "", "", when()).unwrap();
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
}
