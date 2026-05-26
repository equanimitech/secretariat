//! Use case: drain undelivered envelopes to their respective relays.
//!
//! Substrate-for-themia Move 5 — the federation half that was deferred
//! when Move 4 collapsed `_drafts/` + `sent/` into a single envelope
//! tree. The drain walks every envelope-shaped `.md` under the principal's
//! channel roots, skips those already carrying a `delivered:` marker,
//! and POSTs the rest to the relay declared on the org's membership.
//!
//! ## Today (Slice A' MVP)
//!
//! - Org-scoped channels only (`<root>/orgs/<alias>/channels/.../envelopes/`).
//!   Self-owned channels (`<root>/channels/.../envelopes/`) are
//!   marked `delivered: local` without a network call — they never
//!   federate per the substrate invariant.
//! - Polling-tick driven: `sync_now` calls into `drain_undelivered`
//!   once per cycle. The fs-notify watcher in `crates/daemon/` exists
//!   but is NOT wired here yet — sub-second send latency is a follow-up
//!   slice. The 15-min poll floor is the user-facing latency for the
//!   v0.11.x release.
//! - Best-effort: each envelope is independent. A transient relay
//!   failure on one envelope does not poison the cursor for the others;
//!   the next tick retries. Hard malformations are logged and skipped
//!   (the envelope file stays on disk for human inspection).

use std::fs;
use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use thiserror::Error;

use crate::domain::Did;
use crate::infrastructure::keys::KeyPaths;
use crate::infrastructure::markdown::{embed_frontmatter_with_extra, parse_document, MarkdownError};
use crate::infrastructure::membership_store::{load_membership, MEMBERSHIP_FILENAME};
use crate::infrastructure::org_store::{list_org_dirs, org_channels_root, OrgStoreError};
use crate::infrastructure::transport::{RelayClient, RelayState};

#[derive(Debug, Error)]
pub enum FederationError {
    #[error("org store: {0}")]
    OrgStore(#[from] OrgStoreError),
    #[error("markdown: {0}")]
    Markdown(#[from] MarkdownError),
}

#[derive(Debug, Default, Clone)]
pub struct FederationOutcome {
    pub sent: usize,
    pub local_marked: usize,
    pub warnings: Vec<String>,
}

/// Walk every org's channel tree, federate undelivered envelopes. Uses
/// the same `RelayState` the inbound poller uses so auth tokens are
/// shared. The caller's outer save of `RelayState` persists any token
/// refreshes — this function does not save state itself.
pub async fn drain_undelivered(
    paths: &KeyPaths,
    self_did: &Did,
    self_key: &SigningKey,
    _state: &mut RelayState,
) -> Result<FederationOutcome, FederationError> {
    let mut outcome = FederationOutcome::default();
    let orgs = list_org_dirs(&paths.orgs_root)?;
    for org in orgs {
        let membership_path = paths
            .orgs_root
            .join(org.alias.as_str())
            .join(MEMBERSHIP_FILENAME);
        let Ok(Some(membership)) = load_membership(&membership_path) else {
            continue;
        };
        let endpoint = membership.relay_endpoint.as_str().to_string();
        let channels_root = org_channels_root(&paths.orgs_root, &org.alias);
        let candidates = collect_undelivered_under(&channels_root);
        if candidates.is_empty() {
            continue;
        }

        // POST to `/v0/queue/<owner>/<handle>` is gated on the OWNER
        // being a registered tenant — not on the sender's bearer token.
        // The sender announces itself via `x-sender-did`; the relay
        // accepts the envelope into the owner's queue without bearer
        // auth (queue/post does not call `validate_token`). So no
        // token refresh is needed on the outbound drain path; the
        // inbound poll uses a bearer (in `sync.rs::poll_one_queue`)
        // and refreshes there.
        let client = RelayClient::new(&endpoint, self_did.clone(), self_key);

        for path in candidates {
            match federate_one(&client, &path).await {
                Ok(seq) => {
                    if let Err(e) = mark_delivered(&path, &seq) {
                        outcome.warnings.push(format!(
                            "POST succeeded but could not write `delivered:` at {}: {e}",
                            path.display()
                        ));
                    } else {
                        outcome.sent += 1;
                    }
                }
                Err(FederationLocal::SelfOwned) => {
                    if let Err(e) = mark_delivered(&path, "local") {
                        outcome.warnings.push(format!(
                            "self-owned envelope, could not write `delivered: local` at {}: {e}",
                            path.display()
                        ));
                    } else {
                        outcome.local_marked += 1;
                    }
                }
                Err(FederationLocal::Soft(msg)) => outcome.warnings.push(msg),
            }
        }
    }
    Ok(outcome)
}

#[derive(Debug)]
enum FederationLocal {
    SelfOwned,
    Soft(String),
}

async fn federate_one(client: &RelayClient<'_>, path: &Path) -> Result<String, FederationLocal> {
    let raw = fs::read_to_string(path)
        .map_err(|e| FederationLocal::Soft(format!("read {}: {e}", path.display())))?;
    let parsed = parse_document(&raw)
        .map_err(|e| FederationLocal::Soft(format!("parse {}: {e}", path.display())))?;
    let Some(envelope) = parsed.envelope else {
        return Err(FederationLocal::Soft(format!(
            "no $envelope frontmatter at {}",
            path.display()
        )));
    };
    if envelope.delivered.is_some() {
        return Err(FederationLocal::Soft(format!(
            "race: {} already delivered before we got to it",
            path.display()
        )));
    }
    // Defensive: don't push other peoples' drafts. The drain is for
    // our own outbound. Mismatched `from` indicates either an inbound
    // envelope written without a `delivered:` marker (bug elsewhere)
    // or filesystem corruption.
    if envelope.from.as_str() != client.did.as_str() {
        return Err(FederationLocal::Soft(format!(
            "skipping {} — envelope.from={} ≠ principal_did",
            path.display(),
            envelope.from.as_str()
        )));
    }

    // Self-owned channels never federate.
    if envelope.recipient.owner.as_str() == client.did.as_str() {
        return Err(FederationLocal::SelfOwned);
    }

    let seq = client
        .send(
            &envelope.recipient.owner,
            &envelope.recipient.handle,
            raw.as_bytes(),
            "text/markdown",
        )
        .await
        .map_err(|e| FederationLocal::Soft(format!("relay POST {}: {e}", path.display())))?;
    Ok(seq.to_string())
}

/// Re-write the envelope file with `delivered: <marker>` set in its
/// `$envelope` frontmatter block. Idempotent — if the file already
/// carries a `delivered:` value, the existing one wins.
fn mark_delivered(path: &Path, marker: &str) -> Result<(), std::io::Error> {
    let raw = fs::read_to_string(path)?;
    let parsed = match parse_document(&raw) {
        Ok(p) => p,
        Err(e) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("parse {}: {e}", path.display()),
            ));
        }
    };
    let mut envelope = parsed
        .envelope
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no $envelope"))?;
    if envelope.delivered.is_some() {
        return Ok(()); // idempotent
    }
    envelope.delivered = Some(marker.to_string());
    let rebuilt = embed_frontmatter_with_extra(
        &parsed.body,
        Some(&envelope),
        parsed.signature.as_ref(),
        parsed.stamp.as_ref(),
        parsed.extra,
    )
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    fs::write(path, rebuilt)?;
    Ok(())
}

/// Walk under `channels_root` collecting every `.md` envelope-shaped file
/// (must be under an `envelopes/` subdir to qualify). Caller decides
/// whether each candidate is actually undelivered by parsing the
/// `$envelope.delivered` field.
fn collect_undelivered_under(channels_root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !channels_root.exists() {
        return out;
    }
    walk_envelopes(channels_root, false, &mut out);
    // Filter to those without `delivered:` set. We parse just enough
    // frontmatter to check the field. The full federate_one parse will
    // run again on accepted candidates; the cost is intentional —
    // keeping the predicate side-effect-free.
    out.retain(|p| match fs::read_to_string(p).ok() {
        Some(raw) => match parse_document(&raw) {
            Ok(d) => d
                .envelope
                .map(|e| e.delivered.is_none())
                .unwrap_or(false),
            Err(_) => false,
        },
        None => false,
    });
    out
}

fn walk_envelopes(dir: &Path, in_envelopes: bool, out: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if path.is_dir() {
            let next = in_envelopes || name == "envelopes";
            walk_envelopes(&path, next, out);
        } else if in_envelopes && path.extension().and_then(|x| x.to_str()) == Some("md") {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use ed25519_dalek::SigningKey;
    use tempfile::TempDir;

    fn deterministic_key(seed: u8) -> (SigningKey, Did) {
        let bytes = [seed; 32];
        let key = SigningKey::from_bytes(&bytes);
        let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        (key, did)
    }

    #[test]
    fn mark_delivered_writes_field_idempotently() {
        // Compose an envelope file on disk (via emit_channel_def — the
        // simplest path to a real signed envelope) and verify
        // mark_delivered embeds `delivered: <marker>` into the
        // $envelope frontmatter.
        let tmp = TempDir::new().unwrap();
        let orgs_root = tmp.path().join("orgs");
        let alias = crate::domain::OrgAlias::parse("equanimi.tech").unwrap();
        crate::application::create_org(
            &orgs_root,
            alias.clone(),
            None,
            "EquanimiTech",
            "",
            Utc::now(),
            None,
        )
        .unwrap();
        let (key, did) = deterministic_key(5);
        let (_, org_did) = deterministic_key(6);
        let handle = crate::domain::QueueHandle::parse("project:newthing").unwrap();
        let path = crate::application::emit_channel_def_envelope(
            &orgs_root,
            &alias,
            &org_did,
            &did,
            crate::domain::SignerRole::Principal,
            &key,
            &handle,
            "",
            "",
            false,
            Utc::now(),
        )
        .unwrap();

        // Pre-condition: no `delivered:` field.
        assert!(!fs::read_to_string(&path).unwrap().contains("delivered:"));
        mark_delivered(&path, "42").unwrap();
        let after = fs::read_to_string(&path).unwrap();
        assert!(after.contains("delivered: '42'") || after.contains("delivered: \"42\"") || after.contains("delivered: 42"));

        // Idempotent: a second call leaves the existing marker alone.
        mark_delivered(&path, "999").unwrap();
        let final_raw = fs::read_to_string(&path).unwrap();
        assert!(!final_raw.contains("999"), "second mark must not overwrite");
    }

    #[test]
    fn collect_undelivered_skips_already_delivered() {
        let tmp = TempDir::new().unwrap();
        let orgs_root = tmp.path().join("orgs");
        let alias = crate::domain::OrgAlias::parse("equanimi.tech").unwrap();
        crate::application::create_org(
            &orgs_root,
            alias.clone(),
            None,
            "EquanimiTech",
            "",
            Utc::now(),
            None,
        )
        .unwrap();
        let (key, did) = deterministic_key(15);
        let (_, org_did) = deterministic_key(16);
        let h1 = crate::domain::QueueHandle::parse("a").unwrap();
        let h2 = crate::domain::QueueHandle::parse("b").unwrap();
        let p1 = crate::application::emit_channel_def_envelope(
            &orgs_root,
            &alias,
            &org_did,
            &did,
            crate::domain::SignerRole::Principal,
            &key,
            &h1,
            "",
            "",
            false,
            Utc::now(),
        )
        .unwrap();
        let _p2 = crate::application::emit_channel_def_envelope(
            &orgs_root,
            &alias,
            &org_did,
            &did,
            crate::domain::SignerRole::Principal,
            &key,
            &h2,
            "",
            "",
            false,
            Utc::now(),
        )
        .unwrap();
        mark_delivered(&p1, "1").unwrap();

        let channels_root = crate::infrastructure::org_store::org_channels_root(&orgs_root, &alias);
        let cands = collect_undelivered_under(&channels_root);
        assert_eq!(cands.len(), 1, "delivered envelope must be skipped");
    }
}

