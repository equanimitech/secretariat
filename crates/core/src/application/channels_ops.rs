//! Use cases for browsing the channel tree.
//!
//! v0.5 channel-substrate primitives — surface what's in the tree
//! without callers re-implementing the directory walk.
//!
//! Two operations:
//! - `list_channels` — enumerate every channel that has at least one
//!   envelope, with count + latest timestamp.
//! - `read_channel` — return the N most-recent envelopes from a single
//!   channel, sorted newest-first.
//!
//! Handle convention: a channel at `<channels_root>/foo/bar/` is
//! addressable as the bare handle `foo:bar`. Nested handles compose
//! via colon segments — no namespace prefix token (v0.5 namespace
//! collapse).
//!
//! Substrate-private subdirs (leading underscore — `_meta`, `_ciphertext`)
//! and the `envelopes/` directory itself are skipped during the channel
//! tree walk.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use serde::Serialize;
use thiserror::Error;

use crate::domain::{ChannelDef, QueueHandle};
use crate::infrastructure::channel_def_store::{
    channel_def_exists_in_dir, delete_channel as delete_channel_tree, read_channel_meta_in_dir,
    save_channel_def, ChannelDefStoreError,
};
use crate::infrastructure::contract_store::{
    channel_contract_path, save_stub_if_absent, ContractStoreError,
};
use crate::infrastructure::markdown::{parse_document, MarkdownError};

#[derive(Debug, Error)]
pub enum ChannelOpError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("markdown parse at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: MarkdownError,
    },
    #[error("channel `{0}` has no envelopes directory on disk")]
    ChannelNotFound(String),
    #[error("channel def store: {0}")]
    ChannelDefStore(#[from] ChannelDefStoreError),
    #[error("contract store: {0}")]
    ContractStore(#[from] ContractStoreError),
}

/// One row in `list_channels` output.
#[derive(Debug, Clone, Serialize)]
pub struct ChannelSummary {
    /// Canonical handle, e.g. `secretariat:dev`.
    pub handle: String,
    /// Human-readable display name from the channel manifest (empty if
    /// no manifest exists or name field is empty).
    pub name: String,
    /// Free-form description from the channel manifest (empty if unset).
    pub description: String,
    /// Number of `.md` envelope files under `envelopes/` (any depth).
    pub envelope_count: usize,
    /// Timestamp of the most recent envelope, parsed from its filename
    /// (`%Y%m%dT%H%M%SZ-<rand>.md`). None when filenames are malformed.
    pub latest_at: Option<DateTime<Utc>>,
}

/// One entry in `read_channel` output.
#[derive(Debug, Clone, Serialize)]
pub struct ChannelEnvelope {
    pub file_path: String,
    /// DID string of the sender (envelope.from). None if frontmatter
    /// missing.
    pub from: Option<String>,
    /// Captured-at timestamp parsed from the filename. None if
    /// malformed.
    pub captured_at: Option<DateTime<Utc>>,
    /// Free-form source marker (envelope.source), e.g. `idea-skill`,
    /// `mcp-capture`. Empty string if no envelope frontmatter.
    pub source: String,
    /// Stamped flag (true if a `$attestation` block is present).
    pub stamped: bool,
    /// Encrypted flag (true if the body is a sealed-box wire form).
    pub encrypted: bool,
    /// Decrypted body, or the sealed wire form if decryption isn't
    /// performed (we leave decryption to a future slice that wires the
    /// key — for self-owned channel captures the body is plaintext).
    pub body: String,
}

/// Walk `<channels_root>/` and emit one `ChannelSummary` per dir that
/// contains an `envelopes/` subtree. Sorted by `latest_at` desc
/// (channels with no parseable timestamps sort last).
pub fn list_channels(channels_root: &Path) -> Result<Vec<ChannelSummary>, ChannelOpError> {
    let mut out = Vec::new();
    if !channels_root.exists() {
        return Ok(out);
    }
    walk(channels_root, "", &mut out)?;
    out.sort_by(|a, b| b.latest_at.cmp(&a.latest_at));
    Ok(out)
}

fn walk(
    dir: &Path,
    handle_prefix: &str,
    out: &mut Vec<ChannelSummary>,
) -> Result<(), ChannelOpError> {
    for entry in read_dir(dir)? {
        let entry = entry.map_err(|e| ChannelOpError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(seg_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if seg_name.starts_with('_') || seg_name == "envelopes" {
            continue;
        }
        let handle_str = if handle_prefix.is_empty() {
            seg_name.to_string()
        } else {
            format!("{handle_prefix}:{seg_name}")
        };
        let envelopes_dir = path.join("envelopes");
        let has_def = channel_def_exists_in_dir(&path);
        let (count, latest_at) = if envelopes_dir.is_dir() {
            scan_envelopes_dir(&envelopes_dir)?
        } else {
            (0, None)
        };
        // Treat as a channel if it has explicit metadata (created via
        // `create_channel`) or any envelopes have landed in it.
        if has_def || count > 0 {
            let (name, description) = if has_def {
                read_channel_meta_in_dir(&path)
            } else {
                (String::new(), String::new())
            };
            out.push(ChannelSummary {
                handle: handle_str.clone(),
                name,
                description,
                envelope_count: count,
                latest_at,
            });
        }
        walk(&path, &handle_str, out)?;
    }
    Ok(())
}

fn scan_envelopes_dir(dir: &Path) -> Result<(usize, Option<DateTime<Utc>>), ChannelOpError> {
    let mut count = 0usize;
    let mut latest: Option<DateTime<Utc>> = None;
    for entry in walk_md_files(dir)? {
        count += 1;
        if let Some(ts) = parse_timestamp_from_filename(&entry) {
            latest = Some(latest.map_or(ts, |prev| prev.max(ts)));
        }
    }
    Ok((count, latest))
}

fn walk_md_files(dir: &Path) -> Result<Vec<PathBuf>, ChannelOpError> {
    let mut out = Vec::new();
    walk_md_inner(dir, &mut out)?;
    Ok(out)
}

fn walk_md_inner(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), ChannelOpError> {
    for entry in read_dir(dir)? {
        let entry = entry.map_err(|e| ChannelOpError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        let path = entry.path();
        if path.is_dir() {
            walk_md_inner(&path, out)?;
        } else if path.extension().and_then(|x| x.to_str()) == Some("md") {
            out.push(path);
        }
    }
    Ok(())
}

fn read_dir(p: &Path) -> Result<fs::ReadDir, ChannelOpError> {
    fs::read_dir(p).map_err(|e| ChannelOpError::Io {
        path: p.to_path_buf(),
        source: e,
    })
}

/// Filename shape: `<YYYY><MM><DD>T<HH><MM><SS>Z-<6base32>.md`.
fn parse_timestamp_from_filename(path: &Path) -> Option<DateTime<Utc>> {
    let stem = path.file_stem()?.to_str()?;
    let (ts_part, _suffix) = stem.split_once('-')?;
    let naive = NaiveDateTime::parse_from_str(ts_part, "%Y%m%dT%H%M%SZ").ok()?;
    Some(Utc.from_utc_datetime(&naive))
}

/// Resolve the on-disk envelopes directory for a channel handle.
fn channel_envelopes_dir(channels_root: &Path, handle: &QueueHandle) -> PathBuf {
    let mut dir = channels_root.to_path_buf();
    for seg in handle.segments() {
        dir.push(seg);
    }
    dir.push("envelopes");
    dir
}

/// Read the `limit` most-recent envelopes from a channel, newest-first.
/// Returns an empty vec if the channel exists but has no envelopes;
/// returns `ChannelNotFound` if the on-disk envelopes/ dir is absent.
pub fn read_channel(
    channels_root: &Path,
    handle: &QueueHandle,
    limit: usize,
) -> Result<Vec<ChannelEnvelope>, ChannelOpError> {
    let envelopes_dir = channel_envelopes_dir(channels_root, handle);
    if !envelopes_dir.exists() {
        return Err(ChannelOpError::ChannelNotFound(handle.as_str().to_string()));
    }

    let mut files = walk_md_files(&envelopes_dir)?;
    // Sort newest-first by parsed timestamp; unparseable sort last.
    files.sort_by_key(|p| std::cmp::Reverse(parse_timestamp_from_filename(p)));
    files.truncate(limit);

    let mut out = Vec::with_capacity(files.len());
    for path in files {
        out.push(read_one(&path)?);
    }
    Ok(out)
}

fn read_one(path: &Path) -> Result<ChannelEnvelope, ChannelOpError> {
    let raw = fs::read_to_string(path).map_err(|e| ChannelOpError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let parsed = parse_document(&raw).map_err(|source| ChannelOpError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    let (from, source, encrypted) = match &parsed.envelope {
        Some(e) => (
            Some(e.from.as_str().to_string()),
            e.source.clone(),
            e.is_encrypted(),
        ),
        None => (None, String::new(), false),
    };
    Ok(ChannelEnvelope {
        file_path: path.display().to_string(),
        from,
        captured_at: parse_timestamp_from_filename(path),
        source,
        stamped: parsed.stamp.is_some(),
        encrypted,
        body: parsed.body,
    })
}

/// Create a channel by writing its `channel.md` manifest, pre-creating
/// the `envelopes/` directory, and auto-scaffolding a stub
/// `contract.local.md` (empty frontmatter — no overrides, inherit from
/// ancestors). Errors if the channel already has a manifest.
/// `handle` must start with `channel:`.
///
/// The contract-stub write is idempotent: if a `contract.local.md` already
/// exists at the path (e.g. left from a previous incarnation of the
/// channel before delete + recreate, or hand-placed by the principal),
/// we leave it alone. Auto-scaffold never silently clobbers.
pub fn create_channel(
    channels_root: &Path,
    handle: QueueHandle,
    name: impl Into<String>,
    description: impl Into<String>,
    created_at: DateTime<Utc>,
    stub_override: Option<&Path>,
) -> Result<ChannelDef, ChannelOpError> {
    let def = ChannelDef::new(handle, name, description, created_at);
    save_channel_def(channels_root, &def, false)?;
    let contract_path = channel_contract_path(channels_root, &def.handle);
    save_stub_if_absent(&contract_path, stub_override)?;
    Ok(def)
}

/// Hard-delete a channel's tree (envelopes, def, nested sub-channels).
/// Idempotent — succeeds even if the channel doesn't exist. Caller
/// handles confirmation UX.
pub fn delete_channel(
    channels_root: &Path,
    handle: &QueueHandle,
) -> Result<(), ChannelOpError> {
    delete_channel_tree(channels_root, handle)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{capture_to_queue, CaptureRequest};
    use crate::domain::{Did, Root};
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn principal() -> Did {
        Did::from_ed25519_public_key(&[0xb1; 32])
    }

    /// Capture helper for tests. `vault_root` is the temp vault root;
    /// the resolver computes `<vault>/_self/channels/<segs>/...` for the
    /// supplied handle. The caller passes `channels` only to vivify the
    /// `channel.md` first (the existence gate refuses unknown channels).
    fn capture(vault_root: &Path, channels: &Path, handle: &str, body: &str, now: DateTime<Utc>) {
        let q = QueueHandle::parse(handle).unwrap();
        let _ = create_channel(channels, q.clone(), "", "", now, None);
        let req = CaptureRequest {
            from: principal(),
            queue: q,
            body: body.to_string(),
            source: "test".to_string(),
        };
        capture_to_queue(req, vault_root, &Root::Self_, now).unwrap();
    }

    /// Self-channels root under a temp vault, matching what `capture()`
    /// would resolve via `channels_root_for(vault, Root::Self_)`.
    fn self_channels(dir: &TempDir) -> PathBuf {
        dir.path().join("_self").join("channels")
    }

    #[test]
    fn list_channels_empty_root_returns_empty() {
        let dir = TempDir::new().unwrap();
        let out = list_channels(&self_channels(&dir)).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn list_channels_enumerates_tree() {
        let dir = TempDir::new().unwrap();
        let channels = self_channels(&dir);
        capture(
            dir.path(),
            &channels,
            "secretariat:dev",
            "one",
            Utc.with_ymd_and_hms(2026, 5, 12, 10, 0, 0).unwrap(),
        );
        capture(
            dir.path(),
            &channels,
            "secretariat:dev",
            "two",
            Utc.with_ymd_and_hms(2026, 5, 12, 14, 0, 0).unwrap(),
        );
        capture(
            dir.path(),
            &channels,
            "dommage-corporel:paris-cohort",
            "three",
            Utc.with_ymd_and_hms(2026, 5, 12, 12, 0, 0).unwrap(),
        );

        let out = list_channels(&channels).unwrap();
        assert_eq!(out.len(), 2);
        // Sorted newest-first.
        assert_eq!(out[0].handle, "secretariat:dev");
        assert_eq!(out[0].envelope_count, 2);
        assert_eq!(
            out[0].latest_at,
            Some(Utc.with_ymd_and_hms(2026, 5, 12, 14, 0, 0).unwrap())
        );
        assert_eq!(out[1].handle, "dommage-corporel:paris-cohort");
        assert_eq!(out[1].envelope_count, 1);
    }

    #[test]
    fn list_channels_skips_substrate_private_dirs() {
        let dir = TempDir::new().unwrap();
        let channels = self_channels(&dir);

        capture(
            dir.path(),
            &channels,
            "secretariat:dev",
            "ok",
            Utc.with_ymd_and_hms(2026, 5, 12, 10, 0, 0).unwrap(),
        );
        // Drop a private dir alongside.
        fs::create_dir_all(channels.join("_meta")).unwrap();
        fs::write(channels.join("_meta").join("note.md"), "private").unwrap();

        let out = list_channels(&channels).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].handle, "secretariat:dev");
    }

    #[test]
    fn read_channel_returns_newest_first() {
        let dir = TempDir::new().unwrap();
        let channels = self_channels(&dir);

        capture(
            dir.path(),
            &channels,
            "secretariat:dev",
            "first",
            Utc.with_ymd_and_hms(2026, 5, 12, 10, 0, 0).unwrap(),
        );
        capture(
            dir.path(),
            &channels,
            "secretariat:dev",
            "second",
            Utc.with_ymd_and_hms(2026, 5, 12, 14, 0, 0).unwrap(),
        );
        capture(
            dir.path(),
            &channels,
            "secretariat:dev",
            "third",
            Utc.with_ymd_and_hms(2026, 5, 12, 12, 0, 0).unwrap(),
        );

        let h = QueueHandle::parse("secretariat:dev").unwrap();
        let out = read_channel(&channels, &h, 10).unwrap();
        assert_eq!(out.len(), 3);
        // Newest-first: 14:00 → 12:00 → 10:00
        assert!(out[0].body.contains("second"));
        assert!(out[1].body.contains("third"));
        assert!(out[2].body.contains("first"));
    }

    #[test]
    fn read_channel_respects_limit() {
        let dir = TempDir::new().unwrap();
        let channels = self_channels(&dir);

        for h in 0..5 {
            capture(
                dir.path(),
                &channels,
                "secretariat:dev",
                &format!("body-{h}"),
                Utc.with_ymd_and_hms(2026, 5, 12, 10 + h, 0, 0).unwrap(),
            );
        }

        let handle = QueueHandle::parse("secretariat:dev").unwrap();
        let out = read_channel(&channels, &handle, 2).unwrap();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn read_channel_errors_when_dir_missing() {
        let dir = TempDir::new().unwrap();
        let channels = self_channels(&dir);
        let h = QueueHandle::parse("does:not:exist").unwrap();
        let r = read_channel(&channels, &h, 10);
        assert!(matches!(r, Err(ChannelOpError::ChannelNotFound(_))));
    }

    #[test]
    fn create_channel_writes_stub_contract_md() {
        let dir = TempDir::new().unwrap();
        let channels = self_channels(&dir);
        let h = QueueHandle::parse("dev:secretariat").unwrap();
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        create_channel(&channels, h.clone(), "Dev — Secretariat", "", when, None).unwrap();
        let contract_path =
            crate::infrastructure::contract_store::channel_contract_path(&channels, &h);
        assert!(contract_path.is_file(), "stub contract.local.md should be written");
        let (loaded, body) = crate::infrastructure::contract_store::load_contract(&contract_path)
            .unwrap()
            .unwrap();
        assert!(loaded.is_empty(), "stub frontmatter should contribute nothing");
        assert!(body.contains("# importance"));
    }

    #[test]
    fn create_channel_does_not_clobber_hand_edited_contract() {
        let dir = TempDir::new().unwrap();
        let channels = self_channels(&dir);
        let h = QueueHandle::parse("dev:secretariat").unwrap();
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();

        // Pre-stage a hand-edited contract on disk.
        let contract_path =
            crate::infrastructure::contract_store::channel_contract_path(&channels, &h);
        fs::create_dir_all(contract_path.parent().unwrap()).unwrap();
        fs::write(
            &contract_path,
            "---\ncadence_floor_minutes: 30\n---\nhand-edited prose\n",
        )
        .unwrap();

        create_channel(&channels, h.clone(), "Dev — Secretariat", "", when, None).unwrap();

        let (loaded, body) = crate::infrastructure::contract_store::load_contract(&contract_path)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.cadence_floor_minutes, Some(30));
        assert!(body.contains("hand-edited"));
    }

    #[test]
    fn create_channel_makes_empty_channel_visible_in_list() {
        let dir = TempDir::new().unwrap();
        let channels = self_channels(&dir);
        let h = QueueHandle::parse("product:data:baux-commerciaux").unwrap();
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        let def = create_channel(&channels, h, "Baux commerciaux", "Cohort tracking", when, None)
            .unwrap();
        assert_eq!(def.name, "Baux commerciaux");
        // Empty channel shows in list (no envelopes, name carried through).
        let out = list_channels(&channels).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].handle, "product:data:baux-commerciaux");
        assert_eq!(out[0].name, "Baux commerciaux");
        assert_eq!(out[0].envelope_count, 0);
    }

    #[test]
    fn create_channel_refuses_to_overwrite_existing() {
        let dir = TempDir::new().unwrap();
        let channels = self_channels(&dir);
        let h = QueueHandle::parse("secretariat:dev").unwrap();
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        create_channel(&channels, h.clone(), "", "", when, None).unwrap();
        let r = create_channel(&channels, h, "", "", when, None);
        assert!(matches!(
            r,
            Err(ChannelOpError::ChannelDefStore(
                ChannelDefStoreError::AlreadyExists(_)
            ))
        ));
    }

    #[test]
    fn delete_channel_removes_tree() {
        let dir = TempDir::new().unwrap();
        let channels = self_channels(&dir);
        let h = QueueHandle::parse("secretariat:dev").unwrap();
        let when = Utc.with_ymd_and_hms(2026, 5, 12, 0, 0, 0).unwrap();
        create_channel(&channels, h.clone(), "", "", when, None).unwrap();
        capture(
            dir.path(),
            &channels,
            "secretariat:dev",
            "an envelope",
            when,
        );
        delete_channel(&channels, &h).unwrap();
        let out = list_channels(&channels).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn delete_channel_is_idempotent_for_missing() {
        let dir = TempDir::new().unwrap();
        let channels = self_channels(&dir);
        let h = QueueHandle::parse("nothing:here").unwrap();
        delete_channel(&channels, &h).unwrap();
    }
}
