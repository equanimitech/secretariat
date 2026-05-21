//! `sec migrate outbox-to-drafts` — one-shot migration of the v0.8
//! `<queue>/outbox/*.md` substrate-staging dir to the v0.9 layout
//! (drafts under `<queue>/_drafts/`, stamped envelopes under
//! `<queue>/envelopes/YYYY/MM/DD/`, delivered archive under
//! `<queue>/sent/YYYY/MM/DD/`).
//!
//! See `docs/pitches/2026-05-18-drop-outbox.md`. Idempotent: if no
//! `outbox/` directories exist under the substrate root the command
//! is a clean no-op.
//!
//! # Data preservation
//!
//! Envelopes are never destroyed. Per hard rule
//! (`memory/feedback_envelopes_never_destroyed.md`):
//!
//! 1. Pre-flight `tar` snapshot of every queue dir that holds an
//!    `outbox/` subtree, written to
//!    `<root>/.archive/migrations/<timestamp>/<queue-slug>.tar`.
//! 2. Pre-count `.md` files under all `outbox/` subtrees.
//! 3. Move (`fs::rename`, atomic on a single filesystem) each file to
//!    its new home — unstamped to `_drafts/`, stamped to
//!    `envelopes/YYYY/MM/DD/` (sharded by `stamp.stamped_at` when
//!    available, falling back to the file's mtime), historical
//!    `outbox/sent/*.md` to `sent/YYYY/MM/DD/`.
//! 4. Post-count `.md` files reachable from the new locations. Abort
//!    if pre != post.
//! 5. Remove the now-empty `outbox/` directory (and its `sent/`
//!    subdir if present).

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use secretariat_core::infrastructure::markdown::parse_document;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: SubCmd,
}

#[derive(Subcommand, Debug)]
enum SubCmd {
    /// Migrate every `<queue>/outbox/` subtree to the v0.9 layout.
    /// Per the drop-outbox pitch (`docs/pitches/2026-05-18-drop-outbox.md`).
    OutboxToDrafts(OutboxToDraftsArgs),
}

#[derive(Parser, Debug)]
pub struct OutboxToDraftsArgs {
    /// Walk the substrate but don't move anything. Reports the file
    /// counts that *would* be moved and the destinations. Use this
    /// before the real migration to confirm scope.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}

pub fn run(args: Args) -> Result<()> {
    match args.cmd {
        SubCmd::OutboxToDrafts(a) => run_outbox_to_drafts(a),
    }
}

fn run_outbox_to_drafts(args: OutboxToDraftsArgs) -> Result<()> {
    let paths = key_paths()?;
    let root = paths.root.clone();
    if !root.exists() {
        eprintln!("[sec migrate] substrate root does not exist: {}", root.display());
        return Ok(());
    }

    // 1. Discover every `outbox/` directory under the substrate root.
    let outboxes = discover_outboxes(&root)?;
    if outboxes.is_empty() {
        eprintln!("[sec migrate] no outbox/ directories found under {} — nothing to do", root.display());
        return Ok(());
    }

    eprintln!("[sec migrate] found {} outbox/ directorie(s):", outboxes.len());
    for ob in &outboxes {
        eprintln!("  · {}", ob.display());
    }

    // 2. Pre-count .md files (recursive — includes outbox/sent/).
    let pre_count = count_md_files(&outboxes)?;
    eprintln!("[sec migrate] pre-count: {pre_count} `.md` envelope file(s)");

    if pre_count == 0 {
        // Empty outbox dirs — just clean them up.
        if !args.dry_run {
            for ob in &outboxes {
                let _ = remove_dir_if_empty_recursive(ob);
            }
        }
        eprintln!("[sec migrate] removed {} empty outbox dir(s)", outboxes.len());
        return Ok(());
    }

    // 3. Snapshot each queue dir holding an outbox.
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let snapshot_root = root.join(".archive").join("migrations").join(&timestamp);
    if !args.dry_run {
        std::fs::create_dir_all(&snapshot_root)
            .with_context(|| format!("creating {}", snapshot_root.display()))?;
        for ob in &outboxes {
            snapshot_queue(&root, ob, &snapshot_root)?;
        }
        eprintln!("[sec migrate] snapshots written under {}", snapshot_root.display());
    } else {
        eprintln!("[sec migrate] (dry-run) would tar each queue under {}", snapshot_root.display());
    }

    // 4. Move every file. Track destinations so we can post-count.
    let mut moved: Vec<PathBuf> = Vec::new();
    for ob in &outboxes {
        let queue_dir = ob.parent().ok_or_else(|| {
            anyhow!("outbox path has no parent: {}", ob.display())
        })?;
        migrate_one_outbox(queue_dir, ob, args.dry_run, &mut moved)?;
    }

    // 5. Post-count check.
    if !args.dry_run {
        let post_count = moved.iter().filter(|p| p.exists()).count();
        if post_count != pre_count {
            bail!(
                "[sec migrate] count mismatch: pre={pre_count} post={post_count} — \
                 ABORTED. Snapshots at {}; do not run again without verifying.",
                snapshot_root.display()
            );
        }
        eprintln!("[sec migrate] post-count: {post_count} (matches pre-count)");

        // 6. Remove now-empty outbox/ dirs.
        for ob in &outboxes {
            let _ = remove_dir_if_empty_recursive(ob);
        }
        eprintln!("[sec migrate] OK — removed {} empty outbox dir(s)", outboxes.len());
    } else {
        eprintln!("[sec migrate] (dry-run) would have moved {} file(s)", moved.len());
    }

    Ok(())
}

/// Walk the substrate root and return every directory named `outbox`.
/// Skips `.archive/` (snapshot location), other dotfiles, and standard
/// substrate trees that don't hold queues.
fn discover_outboxes(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_for_outboxes(root, &mut out)?;
    Ok(out)
}

fn walk_for_outboxes(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("read_dir {}", dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if name.starts_with('.') {
            // Skip .archive/, .claude/, etc.
            continue;
        }
        if matches!(name, "bin" | "logs" | "peers" | "_ciphertext" | "envelopes" | "_drafts" | "sent" | "deferred" | "archived" | "_unsorted") {
            continue;
        }
        if name == "outbox" {
            out.push(path);
            continue;
        }
        walk_for_outboxes(&path, out)?;
    }
    Ok(())
}

fn count_md_files(outboxes: &[PathBuf]) -> Result<usize> {
    let mut total = 0;
    for ob in outboxes {
        total += count_md_recursive(ob)?;
    }
    Ok(total)
}

fn count_md_recursive(dir: &Path) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut n = 0;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            n += count_md_recursive(&path)?;
        } else if path.extension() == Some(OsStr::new("md")) {
            n += 1;
        }
    }
    Ok(n)
}

/// `tar` the queue directory (parent of `outbox/`) into the snapshot
/// root. Filename: `<sanitized-relative-path>.tar`. Uses the system
/// `tar` so we don't pull in a tar crate just for the migration. The
/// snapshot is a defensive copy; the move ops below are atomic, but
/// the snapshot protects against operator panic or partial migration.
fn snapshot_queue(root: &Path, outbox: &Path, snapshot_root: &Path) -> Result<()> {
    let queue_dir = outbox
        .parent()
        .ok_or_else(|| anyhow!("outbox has no parent: {}", outbox.display()))?;
    let rel = queue_dir.strip_prefix(root).unwrap_or(queue_dir);
    let slug = rel
        .to_string_lossy()
        .replace(['/', '\\'], "_")
        .replace(':', "_");
    let archive_path = snapshot_root.join(format!("{slug}.tar"));
    let status = Command::new("tar")
        .arg("-cf")
        .arg(&archive_path)
        .arg("-C")
        .arg(queue_dir)
        .arg("outbox")
        .status()
        .with_context(|| format!("invoking tar for {}", queue_dir.display()))?;
    if !status.success() {
        bail!(
            "tar failed for {} (status {})",
            queue_dir.display(),
            status
        );
    }
    Ok(())
}

/// Migrate one `<queue>/outbox/` subtree to the new layout.
/// - `outbox/*.md` (unstamped) → `<queue>/_drafts/<file>.md`
/// - `outbox/*.md` (stamped)   → `<queue>/envelopes/YYYY/MM/DD/<file>.md`
/// - `outbox/sent/*.md`        → `<queue>/sent/YYYY/MM/DD/<file>.md`
fn migrate_one_outbox(
    queue_dir: &Path,
    outbox: &Path,
    dry_run: bool,
    moved: &mut Vec<PathBuf>,
) -> Result<()> {
    let mut seen_dest_dirs: HashSet<PathBuf> = HashSet::new();
    // Top-level `outbox/*.md`.
    for entry in std::fs::read_dir(outbox)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            continue;
        }
        if p.extension() != Some(OsStr::new("md")) {
            continue;
        }
        let dest = destination_for_outbox_md(queue_dir, &p)?;
        ensure_parent(&dest, dry_run, &mut seen_dest_dirs)?;
        if dry_run {
            eprintln!("[sec migrate] would move {} -> {}", p.display(), dest.display());
        } else {
            std::fs::rename(&p, &dest)
                .with_context(|| format!("rename {} -> {}", p.display(), dest.display()))?;
        }
        moved.push(dest);
    }
    // `outbox/sent/*.md` — historical deliveries.
    let sent_subdir = outbox.join("sent");
    if sent_subdir.is_dir() {
        for entry in std::fs::read_dir(&sent_subdir)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() || p.extension() != Some(OsStr::new("md")) {
                continue;
            }
            let dest = destination_for_sent_md(queue_dir, &p)?;
            ensure_parent(&dest, dry_run, &mut seen_dest_dirs)?;
            if dry_run {
                eprintln!("[sec migrate] would move {} -> {}", p.display(), dest.display());
            } else {
                std::fs::rename(&p, &dest)
                    .with_context(|| format!("rename {} -> {}", p.display(), dest.display()))?;
            }
            moved.push(dest);
        }
    }
    Ok(())
}

/// Decide where an `outbox/<file>.md` should land:
///   - stamped → `<queue>/envelopes/YYYY/MM/DD/<file>.md`
///   - unstamped → `<queue>/_drafts/<file>.md`
fn destination_for_outbox_md(queue_dir: &Path, file: &Path) -> Result<PathBuf> {
    let (stamped, when) = inspect_envelope(file)?;
    let name = file
        .file_name()
        .ok_or_else(|| anyhow!("no filename: {}", file.display()))?;
    if stamped {
        let day = when.format("%Y/%m/%d").to_string();
        Ok(queue_dir.join("envelopes").join(day).join(name))
    } else {
        Ok(queue_dir.join("_drafts").join(name))
    }
}

/// `<queue>/sent/YYYY/MM/DD/<file>.md`. Sent envelopes always carry a
/// stamp — we shard by the stamp's `stampedAt` when present, falling
/// back to file mtime.
fn destination_for_sent_md(queue_dir: &Path, file: &Path) -> Result<PathBuf> {
    let (_stamped, when) = inspect_envelope(file)?;
    let name = file
        .file_name()
        .ok_or_else(|| anyhow!("no filename: {}", file.display()))?;
    let day = when.format("%Y/%m/%d").to_string();
    Ok(queue_dir.join("sent").join(day).join(name))
}

/// Return `(stamped, when)` for a markdown envelope. `when` is the
/// stamp's `stampedAt` if present and parseable, else the file's
/// mtime, else `now`. Defensive against malformed frontmatter — never
/// fails the migration on a parse error; falls back to "unstamped, mtime".
fn inspect_envelope(file: &Path) -> Result<(bool, DateTime<Utc>)> {
    let raw = std::fs::read_to_string(file)
        .with_context(|| format!("reading {}", file.display()))?;
    let parsed = parse_document(&raw).ok();
    let stamped = parsed.as_ref().and_then(|p| p.stamp.as_ref()).is_some();
    let when = parsed
        .as_ref()
        .and_then(|p| p.stamp.as_ref())
        .map(|s| s.stamped_at)
        .or_else(|| mtime_of(file))
        .unwrap_or_else(Utc::now);
    Ok((stamped, when))
}

fn mtime_of(p: &Path) -> Option<DateTime<Utc>> {
    let meta = std::fs::metadata(p).ok()?;
    let mtime = meta.modified().ok()?;
    Some(DateTime::<Utc>::from(mtime))
}

fn ensure_parent(
    dest: &Path,
    dry_run: bool,
    seen: &mut HashSet<PathBuf>,
) -> Result<()> {
    let parent = match dest.parent() {
        Some(p) => p,
        None => return Ok(()),
    };
    if seen.contains(parent) {
        return Ok(());
    }
    seen.insert(parent.to_path_buf());
    if dry_run {
        return Ok(());
    }
    std::fs::create_dir_all(parent)
        .with_context(|| format!("creating {}", parent.display()))
}

/// Recursively remove a directory only if it (and all its descendants)
/// are empty. Used to clean up the empty `outbox/` shells after a
/// successful move.
fn remove_dir_if_empty_recursive(dir: &Path) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    // Recurse into children first.
    let entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    for child in entries {
        if child.is_dir() {
            let _ = remove_dir_if_empty_recursive(&child);
        }
    }
    // If now empty, remove.
    let mut iter = std::fs::read_dir(dir)?;
    if iter.next().is_none() {
        std::fs::remove_dir(dir).ok();
    }
    Ok(())
}
