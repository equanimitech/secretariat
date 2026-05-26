//! Use case: persist an org membership locally on invite claim.
//!
//! When `claim_invite` returns org-flavored context (`org_did`,
//! `org_alias`, `role`, `channel_relay_endpoint`, optional `scope_intent`),
//! this orchestration:
//!
//! 1. Creates the on-disk org skeleton at `<orgs_root>/<alias>/` —
//!    metadata file + empty `channels/` dir + stub contract — so the
//!    daemon's sync enumerator can discover it.
//! 2. Writes `<orgs_root>/<alias>/membership.local.md` declaring this
//!    principal's role + relay endpoint + inviter.
//!
//! The use case is idempotent: re-running on an already-accepted invite
//! either no-ops (org + membership already match) or fails with a
//! conflict if the metadata disagrees. Channel materialisation is NOT
//! this use case's responsibility — channelDef envelopes coming off the
//! `<alias>:_meta` queue do that (Slice A' element §5).
//!
//! Pure orchestration: time and randomness flow in via parameters. IO
//! goes through the org_store and membership_store ports.

use std::path::Path;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::application::OrgOpsError;
use crate::domain::{Did, OrgAlias, OrgAliasError, RelayEndpoint, RelayEndpointParseError};
use crate::infrastructure::membership_store::{
    load_membership, save_membership, MembershipStoreError, OrgMembership, MEMBERSHIP_FILENAME,
};
use crate::infrastructure::org_store::org_dir;

#[derive(Debug, Error)]
pub enum AcceptMembershipError {
    #[error("invalid org alias `{value}`: {source}")]
    InvalidOrgAlias {
        value: String,
        #[source]
        source: OrgAliasError,
    },
    #[error("invalid relay endpoint `{value}`: {source}")]
    InvalidRelayEndpoint {
        value: String,
        #[source]
        source: RelayEndpointParseError,
    },
    #[error("org operation: {0}")]
    Org(#[from] OrgOpsError),
    #[error("membership store: {0}")]
    Membership(#[from] MembershipStoreError),
    #[error(
        "an org with alias `{alias}` already exists locally and its DID \
         disagrees with the invite's `org_did` — refusing to overwrite"
    )]
    AliasConflict { alias: String },
}

/// Input to [`persist_org_membership`]. Mirrors the shape of
/// `InviteClaimed` so the CLI / MCP can hand it through verbatim.
#[derive(Debug, Clone)]
pub struct AcceptMembershipRequest {
    /// Org's DID (`did:web:equanimi.tech` typically).
    pub org_did: Did,
    /// Org alias (`equanimi.tech`) — used as the on-disk dir name.
    pub org_alias: String,
    /// Role granted (`subscribe` / `publish` / `admin` per rosterUpdate).
    pub role: String,
    /// Relay endpoint where the org's channels live.
    pub relay_endpoint: String,
    /// Inviter's DID — recorded on the membership for provenance. Optional
    /// because some bootstrap flows (org owner self-grant) skip it.
    pub inviter_did: Option<Did>,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AcceptMembershipOutcome {
    pub alias: OrgAlias,
    pub membership: OrgMembership,
    /// `true` when the org dir was created by this call. `false` when it
    /// already existed locally (e.g. principal already accepted prior).
    pub org_created: bool,
    /// `true` when membership file was newly written. `false` when an
    /// equivalent membership was already on disk (idempotent re-accept).
    pub membership_created: bool,
}

/// Idempotent. Run when the principal accepts an org invite — writes the
/// org skeleton (if absent) and the membership.local.md record.
pub fn persist_org_membership(
    orgs_root: &Path,
    contract_stub: Option<&Path>,
    request: AcceptMembershipRequest,
) -> Result<AcceptMembershipOutcome, AcceptMembershipError> {
    let alias =
        OrgAlias::parse(&request.org_alias).map_err(|e| AcceptMembershipError::InvalidOrgAlias {
            value: request.org_alias.clone(),
            source: e,
        })?;
    let relay_endpoint = RelayEndpoint::parse(&request.relay_endpoint).map_err(|e| {
        AcceptMembershipError::InvalidRelayEndpoint {
            value: request.relay_endpoint.clone(),
            source: e,
        }
    })?;

    let existing = crate::application::org_ops::show_org(orgs_root, &alias)?;
    let org_created = match &existing {
        Some(org) => {
            if let Some(existing_did) = &org.did {
                if existing_did != &request.org_did {
                    return Err(AcceptMembershipError::AliasConflict {
                        alias: alias.as_str().to_string(),
                    });
                }
            }
            false
        }
        None => {
            // Create the org skeleton so the sync enumerator and the
            // channels root resolver work without further setup. Name +
            // description default to the alias — the principal can
            // customise via `sec orgs edit` once that ships.
            crate::application::org_ops::create_org(
                orgs_root,
                alias.clone(),
                Some(request.org_did.clone()),
                request.org_alias.clone(),
                String::new(),
                request.joined_at,
                contract_stub,
            )?;
            true
        }
    };

    let membership_path = org_dir(orgs_root, &alias).join(MEMBERSHIP_FILENAME);
    let prior = load_membership(&membership_path)?;
    let membership = OrgMembership {
        org_did: request.org_did,
        role: request.role,
        relay_endpoint,
        joined_at: request.joined_at,
        inviter_did: request.inviter_did,
        body: prior.as_ref().map(|m| m.body.clone()).unwrap_or_default(),
    };
    let membership_created = match &prior {
        Some(existing) => {
            existing.org_did != membership.org_did
                || existing.role != membership.role
                || existing.relay_endpoint != membership.relay_endpoint
                || existing.inviter_did != membership.inviter_did
        }
        None => true,
    };
    if membership_created {
        save_membership(&membership_path, &membership)?;
    }

    Ok(AcceptMembershipOutcome {
        alias,
        membership,
        org_created,
        membership_created,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn principal_did() -> Did {
        Did::from_ed25519_public_key(&[3u8; 32])
    }
    fn org_did() -> Did {
        Did::from_ed25519_public_key(&[9u8; 32])
    }
    fn inviter() -> Did {
        Did::from_ed25519_public_key(&[2u8; 32])
    }

    fn request() -> AcceptMembershipRequest {
        AcceptMembershipRequest {
            org_did: org_did(),
            org_alias: "equanimi.tech".to_string(),
            role: "collaborator".to_string(),
            relay_endpoint: "https://relay.equanimi.tech".to_string(),
            inviter_did: Some(inviter()),
            joined_at: DateTime::parse_from_rfc3339("2026-05-26T20:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        }
    }

    #[test]
    fn first_accept_creates_org_skeleton_and_membership() {
        let tmp = TempDir::new().unwrap();
        let outcome = persist_org_membership(tmp.path(), None, request()).unwrap();
        assert!(outcome.org_created);
        assert!(outcome.membership_created);
        assert!(tmp.path().join("equanimi.tech").is_dir());
        assert!(tmp
            .path()
            .join("equanimi.tech")
            .join("channels")
            .is_dir());
        assert!(tmp
            .path()
            .join("equanimi.tech")
            .join(MEMBERSHIP_FILENAME)
            .is_file());
    }

    #[test]
    fn second_accept_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let _ = persist_org_membership(tmp.path(), None, request()).unwrap();
        let again = persist_org_membership(tmp.path(), None, request()).unwrap();
        assert!(!again.org_created);
        assert!(!again.membership_created, "no change → no rewrite");
    }

    #[test]
    fn role_change_rewrites_membership() {
        let tmp = TempDir::new().unwrap();
        let _ = persist_org_membership(tmp.path(), None, request()).unwrap();
        let mut promoted = request();
        promoted.role = "admin".to_string();
        let outcome = persist_org_membership(tmp.path(), None, promoted).unwrap();
        assert!(!outcome.org_created);
        assert!(outcome.membership_created);
        let loaded =
            load_membership(&tmp.path().join("equanimi.tech").join(MEMBERSHIP_FILENAME))
                .unwrap()
                .unwrap();
        assert_eq!(loaded.role, "admin");
    }

    #[test]
    fn alias_taken_by_different_org_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let _ = persist_org_membership(tmp.path(), None, request()).unwrap();
        let mut foreign = request();
        foreign.org_did = principal_did(); // pretend a different org owns the alias
        let r = persist_org_membership(tmp.path(), None, foreign);
        assert!(matches!(r, Err(AcceptMembershipError::AliasConflict { .. })));
    }

    #[test]
    fn invalid_alias_rejected() {
        let tmp = TempDir::new().unwrap();
        let mut bad = request();
        bad.org_alias = "_meta".to_string();
        let r = persist_org_membership(tmp.path(), None, bad);
        assert!(matches!(
            r,
            Err(AcceptMembershipError::InvalidOrgAlias { .. })
        ));
    }
}
