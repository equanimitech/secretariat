//! Use cases for org-primitive CRUD (v0.3 slice 1.5).
//!
//! Orgs are principal-local for v0.3 — a directory under
//! `<orgs_root>/<alias>/` with a `.org` JSON metadata file at the root
//! and `channels/<segs>/...` underneath. No DID resolution, no federation
//! yet — those land with Story A.
//!
//! Verbs shipped:
//! - `create_org` — fail if exists.
//! - `list_orgs` — enumerate every org with metadata.
//! - `delete_org` — hard-remove the tree (caller handles confirmation UX).
//!
//! Rename / edit / archive deferred to the next slice.

use std::path::Path;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::domain::{Did, Org, OrgAlias};
use crate::infrastructure::contract_store::{
    org_contract_path, save_stub_if_absent, ContractStoreError,
};
use crate::infrastructure::org_store::{
    delete_org as delete_org_tree, list_org_dirs, load_org, org_dir, save_org, OrgStoreError,
};

#[derive(Debug, Error)]
pub enum OrgOpsError {
    #[error("org store: {0}")]
    Store(#[from] OrgStoreError),
    #[error("contract store: {0}")]
    ContractStore(#[from] ContractStoreError),
}

/// Create an org with the given metadata. Errors if an org with the
/// same alias already exists at `orgs_root`. Auto-scaffolds a stub
/// `<org-dir>/contract.local.md` at the org root (idempotent — hand-edits
/// survive a re-run).
pub fn create_org(
    orgs_root: &Path,
    alias: OrgAlias,
    did: Option<Did>,
    name: impl Into<String>,
    description: impl Into<String>,
    created_at: DateTime<Utc>,
) -> Result<Org, OrgOpsError> {
    let org = Org::new(alias, did, name, description, created_at);
    save_org(orgs_root, &org, false)?;
    let contract_path = org_contract_path(&org_dir(orgs_root, &org.alias));
    save_stub_if_absent(&contract_path)?;
    Ok(org)
}

/// List every org under `orgs_root`, sorted alphabetically by alias.
pub fn list_orgs(orgs_root: &Path) -> Result<Vec<Org>, OrgOpsError> {
    Ok(list_org_dirs(orgs_root)?)
}

/// Look up a single org by alias. Returns `Ok(None)` if the dir doesn't
/// exist (distinct from existing-but-no-metadata which yields `None`
/// from the lower-level store and is normalized here too).
pub fn show_org(orgs_root: &Path, alias: &OrgAlias) -> Result<Option<Org>, OrgOpsError> {
    match load_org(orgs_root, alias) {
        Ok(opt) => Ok(opt),
        Err(OrgStoreError::NotFound(_)) => Ok(None),
        Err(e) => Err(OrgOpsError::Store(e)),
    }
}

/// Hard-delete an org's tree (metadata + all channels + all envelopes).
/// Errors if the org doesn't exist. Caller handles confirmation UX —
/// this function is purely the action.
pub fn delete_org(orgs_root: &Path, alias: &OrgAlias) -> Result<(), OrgOpsError> {
    delete_org_tree(orgs_root, alias)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn when() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap()
    }

    fn alias(s: &str) -> OrgAlias {
        OrgAlias::parse(s).unwrap()
    }

    #[test]
    fn create_then_show_roundtrips() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("orgs");
        let org = create_org(
            &root,
            alias("themia.pro"),
            Some(Did::parse("did:web:themia.pro").unwrap()),
            "Themia",
            "Legal-tech",
            when(),
        )
        .unwrap();
        let shown = show_org(&root, &org.alias).unwrap().unwrap();
        assert_eq!(shown, org);
    }

    #[test]
    fn create_refuses_to_overwrite_existing() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("orgs");
        create_org(&root, alias("themia.pro"), None, "Themia", "", when()).unwrap();
        let r = create_org(&root, alias("themia.pro"), None, "Themia v2", "", when());
        assert!(matches!(
            r,
            Err(OrgOpsError::Store(OrgStoreError::AlreadyExists(_)))
        ));
    }

    #[test]
    fn list_orgs_sorted_alphabetically() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("orgs");
        create_org(&root, alias("themia.pro"), None, "Themia", "", when()).unwrap();
        create_org(&root, alias("equanimi.tech"), None, "Equanimi", "", when()).unwrap();
        create_org(&root, alias("nwyana"), None, "Nwyana", "", when()).unwrap();
        let orgs = list_orgs(&root).unwrap();
        assert_eq!(orgs.len(), 3);
        assert_eq!(orgs[0].alias.as_str(), "equanimi.tech");
        assert_eq!(orgs[1].alias.as_str(), "nwyana");
        assert_eq!(orgs[2].alias.as_str(), "themia.pro");
    }

    #[test]
    fn show_org_for_missing_returns_none() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("orgs");
        let r = show_org(&root, &alias("missing.tld")).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn create_org_writes_stub_contract_md() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("orgs");
        let a = alias("themia.pro");
        create_org(&root, a.clone(), None, "Themia", "", when()).unwrap();
        let contract_path = crate::infrastructure::contract_store::org_contract_path(
            &crate::infrastructure::org_store::org_dir(&root, &a),
        );
        assert!(contract_path.is_file(), "stub org contract.local.md should be written");
        let (loaded, body) = crate::infrastructure::contract_store::load_contract(&contract_path)
            .unwrap()
            .unwrap();
        assert!(loaded.is_empty(), "stub frontmatter contributes nothing");
        assert!(body.contains("consumption contract"));
    }

    #[test]
    fn create_org_does_not_clobber_hand_edited_contract() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("orgs");
        let a = alias("themia.pro");
        let contract_path = crate::infrastructure::contract_store::org_contract_path(
            &crate::infrastructure::org_store::org_dir(&root, &a),
        );
        std::fs::create_dir_all(contract_path.parent().unwrap()).unwrap();
        std::fs::write(
            &contract_path,
            "---\ncadence_floor_minutes: 60\n---\nmy-overrides\n",
        )
        .unwrap();
        create_org(&root, a, None, "Themia", "", when()).unwrap();
        let (loaded, body) = crate::infrastructure::contract_store::load_contract(&contract_path)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.cadence_floor_minutes, Some(60));
        assert!(body.contains("my-overrides"));
    }

    #[test]
    fn delete_removes_tree_and_makes_show_return_none() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("orgs");
        create_org(&root, alias("themia.pro"), None, "Themia", "", when()).unwrap();
        delete_org(&root, &alias("themia.pro")).unwrap();
        assert!(show_org(&root, &alias("themia.pro")).unwrap().is_none());
    }
}
