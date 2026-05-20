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
    /// Top-level "Private" entry pointing at `_self`.
    Private,
    /// Top-level org root.
    Org,
    /// Directory containing a `channel.md` — clickable to open a session tab.
    ChannelLeaf,
    /// Directory inside the channel tree without a `channel.md` (a non-leaf
    /// handle segment, or a child dir like `envelopes/`, `outbox/`).
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
    /// Extension without the dot. Empty for dirs and extension-less files.
    pub ext: String,
    /// For channel leaves: the handle string (joined by `:`). None
    /// otherwise.
    pub handle: Option<String>,
    /// For channel leaves under an org: the org alias. None for `_self`
    /// or non-channel entries.
    pub org: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn list_explorer_roots() -> Result<Vec<TreeEntry>, String> {
    use secretariat_core::application::list_orgs;

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let mut out = Vec::new();

    // Private (_self) — show as "Private" pointing at the _self root so
    // children include channels/, capture queues, etc.
    let self_root = paths.root.join("_self");
    if self_root.is_dir() {
        out.push(TreeEntry {
            name: "Private".into(),
            path: self_root.to_string_lossy().into_owned(),
            kind: EntryKind::Private,
            has_children: dir_has_children(&self_root),
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
        let entry_path = dent.path();
        let metadata = match dent.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let (kind, has_children, ext, handle) = if metadata.is_dir() {
            let is_channel = entry_path.join("channel.md").is_file();
            (
                if is_channel {
                    EntryKind::ChannelLeaf
                } else {
                    EntryKind::Dir
                },
                dir_has_children(&entry_path),
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
            (EntryKind::File, false, ext, None)
        };
        entries.push(TreeEntry {
            name,
            path: entry_path.to_string_lossy().into_owned(),
            kind,
            has_children,
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

fn dir_has_children(p: &std::path::Path) -> bool {
    if let Ok(mut iter) = std::fs::read_dir(p) {
        iter.any(|dent| match dent {
            Ok(d) => {
                let n = d.file_name();
                let s = n.to_string_lossy();
                !s.starts_with('.') || s == ".claude"
            }
            Err(_) => false,
        })
    } else {
        false
    }
}

/// Reverse-engineer the handle from a channel-dir path by walking up
/// until we find a `channels` segment. e.g. `<vault>/_self/channels/dev/relay`
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
