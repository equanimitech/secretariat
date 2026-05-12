//! `Org` — value object describing an organization the principal owns
//! or subscribes to.
//!
//! For v0.3, orgs are principal-local — created with `sec orgs create`
//! or the MCP `create_org` tool. The on-disk root is
//! `~/.secretariat/orgs/<alias>/`, with a `.org` JSON metadata file at
//! the root and channels nested under `channels/`.
//!
//! A DID is optional. Local-only orgs (test sandboxes, personal
//! taxonomies) have no DID; orgs intended for federation later get
//! one assigned at create time or via `edit_org` in a future slice.
//!
//! See `docs/decisions/2026-05-12-substrate-layout-v03.md`.

use chrono::{DateTime, Utc};

use super::{Did, OrgAlias};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Org {
    pub alias: OrgAlias,
    pub did: Option<Did>,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

impl Org {
    pub fn new(
        alias: OrgAlias,
        did: Option<Did>,
        name: impl Into<String>,
        description: impl Into<String>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            alias,
            did,
            name: name.into(),
            description: description.into(),
            created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn org_round_trips_through_constructor() {
        let alias = OrgAlias::parse("themia.pro").unwrap();
        let did = Some(Did::parse("did:web:themia.pro").unwrap());
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let o = Org::new(alias.clone(), did.clone(), "Themia", "Legal tech", when);
        assert_eq!(o.alias, alias);
        assert_eq!(o.did, did);
        assert_eq!(o.name, "Themia");
        assert_eq!(o.description, "Legal tech");
        assert_eq!(o.created_at, when);
    }
}
