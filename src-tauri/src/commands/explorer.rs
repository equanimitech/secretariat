//! Filesystem-explorer commands powering the left-sidebar tree.
//!
//! Two commands: `list_explorer_roots` returns the top-level entries
//! (private vault + every org); `list_dir` lazy-loads children for any
//! absolute path. The frontend tree caches results per-path.
//!
//! Filesystem-authoritative per [[project_filesystem_authoritative]] —
//! we walk on demand; no DB, no read-cache.

use std::path::PathBuf;

use secretariat_core::infrastructure::keys::KeyPaths;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// Top-level "Private" entry pointing at the principal's self
    /// channels root (`<root>/channels/`).
    Private,
    /// Top-level org root.
    Org,
    /// Directory containing a `channel.md` — clickable to open a session tab.
    ChannelLeaf,
    /// Directory inside the channel tree without a `channel.md` (a non-leaf
    /// handle segment, or a child dir like `envelopes/`).
    Dir,
    /// Regular file. The extension is exposed so the renderer can decide
    /// how to open it (markdown editor for `.md`, Finder for others).
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub kind: EntryKind,
    /// Cheap to compute; true for dirs that have at least one visible
    /// child. Lets the tree render disclosure triangles without expanding.
    pub has_children: bool,
    /// True for directories that contain (at any depth) at least one
    /// directory with a `channel.md`. Used by the frontend to filter
    /// the tree to channel-only view, and to detect "parent channel"
    /// entries (channel-leaves whose descendants include further
    /// channel-leaves — those should expand/collapse, not open).
    pub has_channel_descendants: bool,
    /// Extension without the dot. Empty for dirs and extension-less files.
    pub ext: String,
    /// For channel leaves: the handle string (joined by `:`). None
    /// otherwise.
    pub handle: Option<String>,
    /// For channel leaves under an org: the org alias. None for
    /// self channels or non-channel entries.
    pub org: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn list_explorer_roots() -> Result<Vec<TreeEntry>, String> {
    use secretariat_core::application::list_orgs;

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let mut out = Vec::new();

    // Private — point at `<root>/channels/` so children show the
    // principal's own channel tree (Move 3c — substrate-for-themia
    // element §2 drops the `_self/` wrapper).
    let self_channels = paths.personal_channels_root();
    if self_channels.is_dir() {
        out.push(TreeEntry {
            name: "Private".into(),
            path: self_channels.to_string_lossy().into_owned(),
            kind: EntryKind::Private,
            has_children: dir_has_children(&self_channels),
            has_channel_descendants: dir_has_channel_descendants(&self_channels),
            ext: String::new(),
            handle: None,
            org: None,
        });
    }

    // Each org alias.
    if paths.orgs_root.is_dir() {
        let orgs = list_orgs(&paths.orgs_root).map_err(|e| format!("list_orgs: {e}"))?;
        for o in orgs {
            let org_root = paths.orgs_root.join(o.alias.as_str());
            out.push(TreeEntry {
                name: o.alias.as_str().to_string(),
                path: org_root.to_string_lossy().into_owned(),
                kind: EntryKind::Org,
                has_children: dir_has_children(&org_root),
                has_channel_descendants: dir_has_channel_descendants(&org_root),
                ext: String::new(),
                handle: None,
                org: Some(o.alias.as_str().to_string()),
            });
        }
    }

    Ok(out)
}

#[tauri::command]
#[specta::specta]
pub async fn list_dir(path: String) -> Result<Vec<TreeEntry>, String> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    let mut entries = Vec::new();
    for dent in std::fs::read_dir(&p).map_err(|e| format!("read_dir: {e}"))? {
        let dent = dent.map_err(|e| format!("dirent: {e}"))?;
        let name = dent.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') && name != ".claude" {
            continue;
        }
        // Hide substrate-staging dirs from the principal-facing tree.
        // `_ciphertext/` is wire-form cache, never principal-readable.
        // `envelopes/` is materialized timeline content surfaced via
        // the channel-tab timeline, not via the explorer tree.
        // (Substrate-for-themia Move 4: `_drafts/` and `sent/` are gone
        // — drafts and delivered envelopes share `envelopes/`, with
        // `delivered:` frontmatter as the state marker.)
        if name == "_ciphertext" || name == "envelopes" {
            continue;
        }
        let entry_path = dent.path();
        let metadata = match dent.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let (kind, has_children, has_channel_descendants, ext, handle) = if metadata.is_dir() {
            let is_channel = entry_path.join("channel.md").is_file();
            (
                if is_channel {
                    EntryKind::ChannelLeaf
                } else {
                    EntryKind::Dir
                },
                dir_has_children(&entry_path),
                dir_has_channel_descendants(&entry_path),
                String::new(),
                if is_channel {
                    derive_handle(&entry_path)
                } else {
                    None
                },
            )
        } else {
            let ext = entry_path
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default();
            (EntryKind::File, false, false, ext, None)
        };
        entries.push(TreeEntry {
            name,
            path: entry_path.to_string_lossy().into_owned(),
            kind,
            has_children,
            has_channel_descendants,
            ext,
            handle,
            org: None,
        });
    }
    // Dirs first, alpha within each group.
    entries.sort_by(|a, b| {
        let a_is_dir = !matches!(a.kind, EntryKind::File);
        let b_is_dir = !matches!(b.kind, EntryKind::File);
        b_is_dir
            .cmp(&a_is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// True if `p` itself contains a `channel.md`, or any descendant
/// directory does. Walks bounded subtrees only — skips `envelopes`,
/// `_ciphertext`, `.claude` which are non-channel substrate. Bounded
/// depth (8) to avoid pathological walks.
fn dir_has_channel_descendants(p: &std::path::Path) -> bool {
    fn walk(p: &std::path::Path, depth: usize) -> bool {
        if depth == 0 {
            return false;
        }
        let iter = match std::fs::read_dir(p) {
            Ok(it) => it,
            Err(_) => return false,
        };
        for dent in iter.flatten() {
            let name = dent.file_name();
            let s = name.to_string_lossy();
            if s.starts_with('.') && s != ".claude" {
                continue;
            }
            // Skip envelope/transport substrate — pure leaf storage,
            // never holds channel subdirs.
            if matches!(s.as_ref(), "envelopes" | "_ciphertext") {
                continue;
            }
            let child = dent.path();
            if child.is_dir() {
                if child.join("channel.md").is_file() {
                    return true;
                }
                if walk(&child, depth - 1) {
                    return true;
                }
            }
        }
        false
    }
    walk(p, 8)
}

fn dir_has_children(p: &std::path::Path) -> bool {
    if let Ok(mut iter) = std::fs::read_dir(p) {
        iter.any(|dent| match dent {
            Ok(d) => {
                let n = d.file_name();
                let s = n.to_string_lossy();
                let hidden = s.starts_with('.') && s != ".claude";
                let staging = matches!(s.as_ref(), "_ciphertext" | "envelopes");
                !hidden && !staging
            }
            Err(_) => false,
        })
    } else {
        false
    }
}

/// Collect every envelope file path under the given root directory
/// — recursively walks any `envelopes/` subtree(s) and returns all
/// `.md` files. Used by the explorer to compute unread counts for
/// channel-leaf entries (and their parent folders by descendant
/// aggregation).
///
/// Bounded depth (16) to guard against pathological symlink loops.
#[tauri::command]
#[specta::specta]
pub async fn list_envelopes_under(root: String) -> Result<Vec<String>, String> {
    let p = PathBuf::from(&root);
    if !p.is_dir() {
        return Err(format!("not a directory: {root}"));
    }
    let mut out = Vec::new();
    walk_envelopes(&p, 16, &mut out);
    Ok(out)
}

fn walk_envelopes(dir: &std::path::Path, depth: usize, out: &mut Vec<String>) {
    if depth == 0 {
        return;
    }
    let iter = match std::fs::read_dir(dir) {
        Ok(it) => it,
        Err(_) => return,
    };
    for dent in iter.flatten() {
        let name = dent.file_name();
        let s = name.to_string_lossy();
        if s.starts_with('.') {
            continue;
        }
        let child = dent.path();
        if child.is_dir() {
            walk_envelopes(&child, depth - 1, out);
        } else if child.extension().and_then(|e| e.to_str()) == Some("md") {
            // Only count envelopes — files sitting inside an
            // `envelopes/` ancestor.
            if has_envelopes_ancestor(&child) {
                out.push(child.to_string_lossy().into_owned());
            }
        }
    }
}

fn has_envelopes_ancestor(p: &std::path::Path) -> bool {
    let mut cur = p.parent();
    while let Some(d) = cur {
        if d.file_name().and_then(|s| s.to_str()) == Some("envelopes") {
            return true;
        }
        cur = d.parent();
    }
    false
}

/// Rename a directory or file on disk. The frontend supplies the
/// absolute current path and the new basename (not a full path). The
/// resulting path is the sibling of the original with the supplied
/// name. Intentionally minimal — no DID/handle rewriting, no envelope
/// fixups; the user is responsible for keeping consistency with the
/// channel handle if they rename a channel-dir.
#[tauri::command]
#[specta::specta]
pub async fn rename_path(path: String, new_name: String) -> Result<String, String> {
    let src = PathBuf::from(&path);
    if !src.exists() {
        return Err(format!("path does not exist: {path}"));
    }
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err("new name cannot be empty".into());
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains('\0') {
        return Err("new name cannot contain path separators".into());
    }
    if trimmed == "." || trimmed == ".." {
        return Err("invalid name".into());
    }
    let parent = src
        .parent()
        .ok_or_else(|| format!("no parent for {path}"))?;
    let dst = parent.join(trimmed);
    if dst == src {
        return Ok(dst.to_string_lossy().into_owned());
    }
    if dst.exists() {
        return Err(format!(
            "destination already exists: {}",
            dst.to_string_lossy()
        ));
    }
    std::fs::rename(&src, &dst).map_err(|e| format!("rename: {e}"))?;
    Ok(dst.to_string_lossy().into_owned())
}

/// Move a directory or file under a new parent directory. The basename
/// is preserved (use `rename_path` first if you want to rename + move).
/// Intentionally minimal — no DID/handle rewriting, no envelope fixups.
/// The caller (frontend) is responsible for cycle / cross-root /
/// duplicate-name validation; this command also re-checks the cheap
/// invariants on the Rust side.
#[tauri::command]
#[specta::specta]
pub async fn move_path(src: String, dest_parent: String) -> Result<String, String> {
    let src_path = PathBuf::from(&src);
    let dest_parent_path = PathBuf::from(&dest_parent);
    if !src_path.exists() {
        return Err(format!("source does not exist: {src}"));
    }
    if !dest_parent_path.is_dir() {
        return Err(format!(
            "destination parent is not a directory: {dest_parent}"
        ));
    }
    let basename = src_path
        .file_name()
        .ok_or_else(|| format!("source has no basename: {src}"))?;
    let dst = dest_parent_path.join(basename);
    if dst == src_path {
        return Ok(dst.to_string_lossy().into_owned());
    }
    // Cycle guard: refuse to move a dir into itself or one of its
    // descendants. Canonicalize to dodge `./` and symlink games.
    let src_canon =
        std::fs::canonicalize(&src_path).map_err(|e| format!("canonicalize src: {e}"))?;
    let dest_canon =
        std::fs::canonicalize(&dest_parent_path).map_err(|e| format!("canonicalize dest: {e}"))?;
    if dest_canon == src_canon || dest_canon.starts_with(&src_canon) {
        return Err("cannot move a directory into itself or one of its descendants".into());
    }
    if dst.exists() {
        return Err(format!(
            "destination already exists: {}",
            dst.to_string_lossy()
        ));
    }
    std::fs::rename(&src_path, &dst).map_err(|e| format!("rename: {e}"))?;
    Ok(dst.to_string_lossy().into_owned())
}

/// Reverse-engineer the handle from a channel-dir path by walking up
/// until we find a `channels` segment. e.g. `<vault>/channels/dev/relay`
/// → `dev:relay`. Returns None if no `channels` ancestor is found.
fn derive_handle(channel_dir: &std::path::Path) -> Option<String> {
    let mut segments: Vec<String> = Vec::new();
    let mut cur = channel_dir;
    loop {
        let name = cur.file_name()?.to_string_lossy().into_owned();
        if name == "channels" {
            break;
        }
        segments.push(name);
        cur = cur.parent()?;
    }
    if segments.is_empty() {
        return None;
    }
    segments.reverse();
    Some(segments.join(":"))
}
