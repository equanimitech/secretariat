//! `sec migrate` — one-shot vault migrations between substrate layouts.
//!
//! Subcommands:
//!
//! - `outbox-to-drafts` — v0.8 → v0.9 originally targeted
//!   `<queue>/_drafts/` + `<queue>/envelopes/YYYY/MM/DD/`. Post-Move-4
//!   (v0.11) the `_drafts/` and `sent/` subtrees are gone — every
//!   envelope lands at `<queue>/envelopes/YYYY/MM/DD/` regardless of
//!   stamped/delivered state. The command keeps its historical name
//!   for any v0.8 vault still being upgraded; the destination has been
//!   updated so migrated envelopes remain visible on a post-v0.11
//!   vault. See `docs/pitches/2026-05-18-drop-outbox.md` and Move 4
//!   of `docs/pitches/2026-05-21-substrate-for-themia.md`.
//! - `vault-v0-10-to-v0-11` — drops the `_self/` wrapper and the
//!   peer-alias channel-tree root. Per Move 3c of
//!   `docs/pitches/2026-05-21-substrate-for-themia.md` (element §2):
//!     * `<root>/_self/identity.md`         → `<root>/identity.md`
//!     * `<root>/_self/identity/`           → `<root>/identity/`
//!     * `<root>/_self/channels/`           → `<root>/channels/`
//!     * `<root>/_self/contract-stub.md`    → `<root>/contract-stub.md`
//!     * `<root>/_self/.contextification.log` → `<root>/.contextification.log`
//!     * `<root>/<alias>/` (with `channels/`) → `<root>/orgs/<alias>/`
//!     * Empty `<root>/_self/` removed.
//!
//! Each command is idempotent: re-running against an already-migrated
//! vault is a clean no-op.
//!
//! # Data preservation
//!
//! Envelopes are never destroyed. Per hard rule
//! (`memory/feedback_envelopes_never_destroyed.md`):
//!
//! 1. Pre-flight `tar` snapshot of every directory the migration will
//!    touch, written to `<root>/.archive/migrations/<timestamp>/`.
//! 2. Pre-count `.md` files in the affected subtrees.
//! 3. Move (`fs::rename`, atomic on a single filesystem) each file /
//!    directory to its new home.
//! 4. Post-count `.md` files reachable from the new locations. Abort
//!    if pre != post.
//! 5. Remove now-empty source directory shells.

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
    /// Migrate a vault from the v0.10 layout (`_self/` wrapper + bare
    /// `<alias>/` for orgs) to the v0.11 layout (no wrapper; orgs under
    /// `orgs/<alias>/`). Per Move 3c of substrate-for-themia
    /// (`docs/pitches/2026-05-21-substrate-for-themia.md`, element §2).
    #[command(name = "vault-v0-10-to-v0-11")]
    VaultV010ToV011(VaultV010ToV011Args),
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
        SubCmd::VaultV010ToV011(a) => run_vault_v0_10_to_v0_11(a),
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
/// - `outbox/*.md` (unstamped) → `<queue>/envelopes/YYYY/MM/DD/<file>.md` (drafts identified by absent `delivered:`)
/// - `outbox/*.md` (stamped)   → `<queue>/envelopes/YYYY/MM/DD/<file>.md`
/// - `outbox/sent/*.md`        → `<queue>/envelopes/YYYY/MM/DD/<file>.md` (Move 4 collapsed `sent/`)
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

/// Post-Move-4: there is one envelope state. Every envelope — stamped
/// or unstamped, sent or unsent — lands at
/// `<queue>/envelopes/YYYY/MM/DD/<file>.md`. Drafts are identified by
/// the absence of a `delivered:` frontmatter field, not by a separate
/// `_drafts/` subdirectory. Routing migrated content into `_drafts/`
/// or `sent/` would make it invisible to the daemon watcher and to
/// `list_draft_files` on post-v0.11 vaults.
///
/// Shard by the stamp's `stampedAt` when present, else file mtime,
/// else `now`.
fn destination_for_outbox_md(queue_dir: &Path, file: &Path) -> Result<PathBuf> {
    let (_stamped, when) = inspect_envelope(file)?;
    let name = file
        .file_name()
        .ok_or_else(|| anyhow!("no filename: {}", file.display()))?;
    let day = when.format("%Y/%m/%d").to_string();
    Ok(queue_dir.join("envelopes").join(day).join(name))
}

/// Same target as `destination_for_outbox_md` — Move 4 collapsed the
/// `sent/` subtree into the unified `envelopes/` tree.
fn destination_for_sent_md(queue_dir: &Path, file: &Path) -> Result<PathBuf> {
    destination_for_outbox_md(queue_dir, file)
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

// ---------------------------------------------------------------------------
// vault-v0-10-to-v0-11 — Move 3c layout migration
// ---------------------------------------------------------------------------

/// Names at the substrate root that ARE NOT org-alias dirs to be wrapped
/// into `orgs/`. Everything else at root level holding `channels/` gets
/// folded under `orgs/<alias>/`.
const ROOT_RESERVED_NAMES: &[&str] = &[
    "orgs",
    "channels",
    "identity",
    "peers",
    "bin",
    ".archive",
    "_self",
];

#[derive(Parser, Debug)]
pub struct VaultV010ToV011Args {
    /// Walk the vault but don't move anything. Reports the planned
    /// `mv` operations + the snapshot it would take.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}

fn run_vault_v0_10_to_v0_11(args: VaultV010ToV011Args) -> Result<()> {
    let paths = key_paths()?;
    let root = paths.root.clone();
    if !root.exists() {
        eprintln!(
            "[sec migrate] substrate root does not exist: {}",
            root.display()
        );
        return Ok(());
    }

    // Plan the moves up front so dry-run prints the full picture and the
    // real run can pre-flight tar before touching anything.
    let plan = plan_vault_v0_10_to_v0_11(&root)?;
    if plan.is_empty() {
        eprintln!(
            "[sec migrate] vault at {} already on the v0.11 layout — nothing to do",
            root.display()
        );
        return Ok(());
    }

    eprintln!("[sec migrate] planned move(s):");
    for (src, dst) in &plan {
        eprintln!("  · {} -> {}", src.display(), dst.display());
    }

    // 1. Pre-count `.md` files in every source.
    let pre_count: usize = plan
        .iter()
        .map(|(src, _)| count_md_recursive(src).unwrap_or(0))
        .sum();
    eprintln!("[sec migrate] pre-count: {pre_count} `.md` file(s) in sources");

    if args.dry_run {
        eprintln!(
            "[sec migrate] (dry-run) would snapshot then perform {} move(s)",
            plan.len()
        );
        return Ok(());
    }

    // 2. tar snapshot per source.
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let snapshot_root = root
        .join(".archive")
        .join("migrations")
        .join(&timestamp);
    std::fs::create_dir_all(&snapshot_root)
        .with_context(|| format!("creating {}", snapshot_root.display()))?;
    for (src, _) in &plan {
        snapshot_path(&root, src, &snapshot_root)?;
    }
    eprintln!(
        "[sec migrate] snapshots written under {}",
        snapshot_root.display()
    );

    // 3. Perform the moves. `fs::rename` is atomic within one filesystem.
    apply_vault_moves(&plan, &snapshot_root)?;

    // 4. Post-count.
    let post_count: usize = plan
        .iter()
        .map(|(_, dst)| count_md_recursive(dst).unwrap_or(0))
        .sum();
    if post_count != pre_count {
        bail!(
            "[sec migrate] count mismatch: pre={pre_count} post={post_count} — \
             ABORTED. Snapshots at {}; verify before re-running.",
            snapshot_root.display()
        );
    }
    eprintln!("[sec migrate] post-count: {post_count} (matches pre-count)");

    // 5. Clean up the now-empty `_self/` shell (and any other empty dirs
    //    left behind by the migration).
    let legacy_self = root.join("_self");
    let _ = remove_dir_if_empty_recursive(&legacy_self);

    eprintln!("[sec migrate] OK — vault is on the v0.11 layout");
    Ok(())
}

/// Execute the planned (src, dst) moves with crash-resume tolerance.
///
/// `(src exists, dst exists)` decision table:
///   - `(true, false)`  → happy path — `fs::rename`.
///   - `(false, true)`  → already moved by a prior run. Skip and continue
///     (preserves the module-docstring "idempotent re-run" invariant).
///   - `(true, true)`   → genuine ambiguity. Bail for human resolution.
///   - `(false, false)` → source vanished between plan + execute. Bail.
///
/// Parents of every destination are created on demand.
fn apply_vault_moves(plan: &[(PathBuf, PathBuf)], snapshot_root: &Path) -> Result<()> {
    for (src, dst) in plan {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        match (src.exists(), dst.exists()) {
            (true, true) => bail!(
                "[sec migrate] both source and destination exist: \
                 src={} dst={} — refuse to overwrite. Snapshots at {}",
                src.display(),
                dst.display(),
                snapshot_root.display()
            ),
            (false, true) => {
                eprintln!(
                    "[sec migrate] skip already-moved: {} (dst {} present)",
                    src.display(),
                    dst.display()
                );
                continue;
            }
            (false, false) => bail!(
                "[sec migrate] source vanished before move: {} \
                 (dst {} also missing). Snapshots at {}",
                src.display(),
                dst.display(),
                snapshot_root.display()
            ),
            (true, false) => {
                std::fs::rename(src, dst)
                    .with_context(|| format!("rename {} -> {}", src.display(), dst.display()))?;
            }
        }
    }
    Ok(())
}

/// Compute the (src, dst) moves required to bring `root` from the
/// v0.10 layout to v0.11. Returns an empty Vec if the vault is already
/// migrated. Pure compute — no IO beyond `read_dir`.
fn plan_vault_v0_10_to_v0_11(root: &Path) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();

    // 1. `_self/` contents.
    let legacy_self = root.join("_self");
    if legacy_self.is_dir() {
        let candidates = [
            ("identity.md", "identity.md"),
            ("identity", "identity"),
            ("channels", "channels"),
            ("contract-stub.md", "contract-stub.md"),
            (".contextification.log", ".contextification.log"),
            ("contacts.md", "contacts.md"),
        ];
        for (rel_src, rel_dst) in candidates {
            let src = legacy_self.join(rel_src);
            if src.exists() {
                moves.push((src, root.join(rel_dst)));
            }
        }
    }

    // 2. Bare `<alias>/` dirs at root with a `channels/` sub-dir get
    //    wrapped under `orgs/<alias>/`. Skip the names that already
    //    belong at root (`orgs`, `channels`, `identity`, `peers`,
    //    `bin`, `.archive`, `_self`) and dotfiles.
    for entry in std::fs::read_dir(root)
        .with_context(|| format!("read_dir {}", root.display()))?
    {
        let entry = entry?;
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if name.starts_with('.') {
            continue;
        }
        if ROOT_RESERVED_NAMES.contains(&name) {
            continue;
        }
        // Only treat as a legacy org if it actually carries a
        // `channels/` sub-dir (i.e. it's a queue-root by shape).
        // Otherwise leave alone — likely user-managed scratch.
        if p.join("channels").is_dir() {
            moves.push((p.clone(), root.join("orgs").join(name)));
        }
    }

    Ok(moves)
}

/// `tar` an arbitrary file or directory under `root` into the snapshot
/// dir. Filename: `<sanitized-relative-path>.tar`. Uses the system
/// `tar` so we don't pull in a tar crate just for the migration.
fn snapshot_path(root: &Path, src: &Path, snapshot_root: &Path) -> Result<()> {
    let rel = src.strip_prefix(root).unwrap_or(src);
    let slug = rel
        .to_string_lossy()
        .replace(['/', '\\'], "_")
        .replace(':', "_");
    let archive_path = snapshot_root.join(format!("{slug}.tar"));
    let parent = src
        .parent()
        .ok_or_else(|| anyhow!("path has no parent: {}", src.display()))?;
    let name = src
        .file_name()
        .ok_or_else(|| anyhow!("path has no filename: {}", src.display()))?;
    let status = Command::new("tar")
        .arg("-cf")
        .arg(&archive_path)
        .arg("-C")
        .arg(parent)
        .arg(name)
        .status()
        .with_context(|| format!("invoking tar for {}", src.display()))?;
    if !status.success() {
        bail!("tar failed for {} (status {})", src.display(), status);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn touch(p: &Path) {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, "x\n").unwrap();
    }

    #[test]
    fn plan_handles_full_legacy_vault() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        touch(&root.join("_self/identity.md"));
        touch(&root.join("_self/identity/key"));
        touch(&root.join("_self/identity/did.json"));
        touch(&root.join("_self/identity/agents/claude/key"));
        touch(&root.join("_self/channels/journal/channel.md"));
        touch(&root.join("_self/contract-stub.md"));
        touch(&root.join("_self/.contextification.log"));
        // An org dir at root with channels/.
        touch(&root.join("themia.pro/contract.md"));
        touch(&root.join("themia.pro/channels/finance/channel.md"));
        // Reserved name — must not be wrapped.
        touch(&root.join("peers/cached.json"));

        let plan = plan_vault_v0_10_to_v0_11(root).unwrap();
        let dests: Vec<_> = plan
            .iter()
            .map(|(_, d)| d.strip_prefix(root).unwrap().to_path_buf())
            .collect();
        assert!(dests.contains(&PathBuf::from("identity.md")));
        assert!(dests.contains(&PathBuf::from("identity")));
        assert!(dests.contains(&PathBuf::from("channels")));
        assert!(dests.contains(&PathBuf::from("contract-stub.md")));
        assert!(dests.contains(&PathBuf::from(".contextification.log")));
        assert!(dests.contains(&PathBuf::from("orgs/themia.pro")));
        // `peers/` must NOT have been planned for wrapping.
        for (_src, dst) in &plan {
            assert!(
                !dst.to_string_lossy().contains("orgs/peers"),
                "peers/ wrongly wrapped: {}",
                dst.display()
            );
        }
    }

    #[test]
    fn plan_for_already_migrated_vault_is_empty() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        touch(&root.join("identity.md"));
        touch(&root.join("identity/key"));
        touch(&root.join("channels/journal/channel.md"));
        touch(&root.join("orgs/themia.pro/channels/finance/channel.md"));
        let plan = plan_vault_v0_10_to_v0_11(root).unwrap();
        assert!(plan.is_empty(), "plan should be empty: {plan:?}");
    }

    #[test]
    fn outbox_destination_lands_in_envelopes_tree_for_unstamped() {
        // Move 4 collapsed `_drafts/` and `sent/`. The outbox-to-drafts
        // migrator must route both stamped and unstamped envelopes into
        // `<queue>/envelopes/YYYY/MM/DD/` so they remain visible to the
        // daemon watcher and `list_draft_files` on post-v0.11 vaults.
        let dir = TempDir::new().unwrap();
        let queue = dir.path().join("q");
        let outbox_md = queue.join("outbox").join("draft.md");
        std::fs::create_dir_all(outbox_md.parent().unwrap()).unwrap();
        // Minimal unstamped envelope — no stamp frontmatter.
        std::fs::write(
            &outbox_md,
            "---\n$type: tech.equanimi.secretariat.envelope\n---\nbody\n",
        )
        .unwrap();
        let dest = destination_for_outbox_md(&queue, &outbox_md).unwrap();
        let rel = dest.strip_prefix(&queue).unwrap();
        let rel_s = rel.to_string_lossy();
        assert!(
            rel_s.starts_with("envelopes/"),
            "expected envelopes/.../, got {rel_s}"
        );
        assert!(
            !rel_s.contains("_drafts"),
            "destination must not use abolished _drafts/: {rel_s}"
        );
        assert!(
            !rel_s.contains("sent/"),
            "destination must not use abolished sent/: {rel_s}"
        );
    }

    #[test]
    fn apply_vault_moves_resumes_after_partial_crash() {
        // Simulates: prior `sec migrate` run did one of two planned moves
        // then crashed. Re-running must skip the already-moved entry and
        // complete the remainder — idempotency-on-resume per module docstring.
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let snapshot_root = root.join(".archive").join("migrations").join("ts");
        std::fs::create_dir_all(&snapshot_root).unwrap();

        // First move: src absent, dst already present (the "crashed
        // after this one" case).
        touch(&root.join("identity.md"));
        // Second move: src present, dst absent (the work that needs
        // to finish).
        touch(&root.join("_self/contract-stub.md"));

        let plan = vec![
            (
                root.join("_self/identity.md"),
                root.join("identity.md"),
            ),
            (
                root.join("_self/contract-stub.md"),
                root.join("contract-stub.md"),
            ),
        ];
        apply_vault_moves(&plan, &snapshot_root).unwrap();
        assert!(root.join("identity.md").exists(), "already-moved kept");
        assert!(root.join("contract-stub.md").exists(), "second move completed");
    }

    #[test]
    fn apply_vault_moves_bails_when_both_ends_exist() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let snapshot_root = root.join(".archive").join("migrations").join("ts");
        std::fs::create_dir_all(&snapshot_root).unwrap();

        // Both src and dst exist — genuine ambiguity.
        touch(&root.join("_self/identity.md"));
        touch(&root.join("identity.md"));

        let plan = vec![(
            root.join("_self/identity.md"),
            root.join("identity.md"),
        )];
        let err = apply_vault_moves(&plan, &snapshot_root).unwrap_err();
        assert!(
            err.to_string().contains("both source and destination exist"),
            "expected ambiguity bail, got: {err}"
        );
    }

    #[test]
    fn migration_moves_files_and_preserves_count() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        // Set up a legacy vault with envelopes to count.
        touch(&root.join("_self/identity.md"));
        touch(&root.join("_self/identity/key"));
        touch(&root.join(
            "_self/channels/journal/envelopes/2026/05/24/note.md",
        ));
        touch(&root.join("themia.pro/contract.md"));
        touch(&root.join(
            "themia.pro/channels/finance/envelopes/2026/05/24/e.md",
        ));

        let plan = plan_vault_v0_10_to_v0_11(root).unwrap();
        assert!(!plan.is_empty());

        // Execute the plan manually, mirroring the real command's
        // mv-and-create-parent flow but skipping tar (no system call).
        for (src, dst) in &plan {
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::rename(src, dst).unwrap();
        }

        assert!(root.join("identity.md").exists());
        assert!(root.join("identity/key").exists());
        assert!(root
            .join("channels/journal/envelopes/2026/05/24/note.md")
            .exists());
        assert!(root.join("orgs/themia.pro/contract.md").exists());
        assert!(root
            .join("orgs/themia.pro/channels/finance/envelopes/2026/05/24/e.md")
            .exists());
        // No legacy paths remain at top level.
        assert!(!root.join("themia.pro").exists());
        // Cleanup the now-empty `_self/` shell.
        let _ = remove_dir_if_empty_recursive(&root.join("_self"));
        assert!(!root.join("_self").exists());
    }
}
