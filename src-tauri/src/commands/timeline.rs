//! Substrate timeline — read envelopes from a channel-dir for display.
//!
//! Reads `<channel_path>/envelopes/<YYYY>/<MM>/<DD>/<...>.md` and
//! surfaces a preview-shaped projection (first lines of body, stamp
//! state, encryption flag, source tag, sender DID, parsed timestamp).
//! Used by the channel-tab timeline pane.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct EnvelopePreview {
    /// Absolute path to the `.md` file — caller uses this to open the
    /// envelope in a markdown tab.
    pub file_path: String,
    /// Sender DID, when parseable from frontmatter.
    pub from: Option<String>,
    /// Captured-at timestamp parsed from the filename (RFC 3339).
    pub at: Option<String>,
    /// Free-form source tag, e.g. `idea-skill`, `mcp-capture`. Empty if
    /// no envelope frontmatter.
    pub source: String,
    /// True if a `$attestation` frontmatter block is present (stamped).
    pub stamped: bool,
    /// True if the body is a sealed wire form (cannot show plaintext
    /// preview).
    pub encrypted: bool,
    /// First few lines of the body, plain text. Empty for encrypted
    /// envelopes. The frontend renders this as markdown when the
    /// envelope has no sender-declared `lede`.
    pub preview: String,
    /// Filename basename — useful as a card title when no headline.
    pub filename: String,
    /// Sender-declared AG headline (envelope.title). Optional.
    /// Renderers SHOULD use this as the card title in timelines when
    /// present; otherwise fall back to first heading / filename.
    pub title: Option<String>,
    /// Sender-declared AG one-liner (envelope.lede). Optional.
    /// Renderers SHOULD use this as the preview line in compact
    /// timeline rows when present, in lieu of `preview` (body slice).
    pub lede: Option<String>,
    /// Sender-declared AG multi-sentence summary (envelope.summary).
    /// Optional. Surfaced in expanded views, not compact rows.
    pub summary: Option<String>,
    /// Best-effort human name for the sender DID — principal's
    /// `display_name`, an authorized agent's nickname, or an org's
    /// `name`. None when no record matches (callers should fall back
    /// to a shortened DID).
    pub from_name: Option<String>,
    /// Free-form tags lifted from envelope root frontmatter (not
    /// inside `$envelope`). Renderers MAY show these as chips.
    pub tags: Vec<String>,
}

const PREVIEW_LINES: usize = 3;
const PREVIEW_CHAR_BUDGET: usize = 280;

#[tauri::command]
#[specta::specta]
pub async fn read_channel_envelopes(
    channel_path: String,
    limit: u32,
) -> Result<Vec<EnvelopePreview>, String> {
    use secretariat_core::application::read_channel;
    use secretariat_core::domain::QueueHandle;

    let channel_dir = PathBuf::from(&channel_path);
    if !channel_dir.is_dir() {
        return Err(format!("not a channel directory: {channel_path}"));
    }

    let (channels_root, handle) = match split_into_channels_root_and_handle(&channel_dir) {
        Some(pair) => pair,
        None => {
            return Err(format!(
                "could not derive channel handle from path: {channel_path}"
            ))
        }
    };
    let parsed = QueueHandle::parse(&handle).map_err(|e| format!("parse handle: {e}"))?;
    let envelopes =
        read_channel(&channels_root, &parsed, limit.max(1) as usize).map_err(|e| format!("{e}"))?;

    let names = NameResolver::load();

    Ok(envelopes
        .into_iter()
        .map(|env| {
            let filename = std::path::Path::new(&env.file_path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let preview = if env.encrypted {
                String::new()
            } else {
                preview_of(&env.body)
            };
            let from_name = env.from.as_deref().and_then(|d| names.lookup(d));
            EnvelopePreview {
                file_path: env.file_path,
                from: env.from,
                at: env.captured_at.map(|t| t.to_rfc3339()),
                source: env.source,
                stamped: env.stamped,
                encrypted: env.encrypted,
                preview,
                filename,
                title: env.title,
                lede: env.lede,
                summary: env.summary,
                from_name,
                tags: env.tags,
            }
        })
        .collect())
}

/// Best-effort DID → human-name map built from the principal's
/// `identity.md` (self DID + authorized agents) and each `orgs/*/org.md`.
/// Loaded once per timeline read; cheap enough for the 100-envelope
/// pagination this command serves. Returns `None` for unknown DIDs —
/// callers fall back to a shortened DID.
struct NameResolver {
    by_did: HashMap<String, String>,
}

impl NameResolver {
    fn load() -> Self {
        use secretariat_core::infrastructure::identity_store::load_identity;
        use secretariat_core::infrastructure::keys::KeyPaths;
        use secretariat_core::infrastructure::org_store::list_org_dirs;

        let mut by_did = HashMap::new();
        let Ok(paths) = KeyPaths::discover() else {
            return Self { by_did };
        };
        if let Ok(Some(identity)) = load_identity(&paths.identity_md) {
            by_did.insert(
                identity.did.as_str().to_string(),
                identity.display_name.to_string(),
            );
            for agent in &identity.authorized_agents {
                by_did.insert(
                    agent.did.as_str().to_string(),
                    agent.name.as_str().to_string(),
                );
            }
        }
        if paths.orgs_root.exists() {
            if let Ok(orgs) = list_org_dirs(&paths.orgs_root) {
                for org in orgs {
                    let Some(did) = org.did else { continue };
                    let name = if org.name.trim().is_empty() {
                        org.alias.as_str().to_string()
                    } else {
                        org.name
                    };
                    by_did.insert(did.as_str().to_string(), name);
                }
            }
        }
        Self { by_did }
    }

    fn lookup(&self, did: &str) -> Option<String> {
        self.by_did.get(did).cloned()
    }
}

/// Walk up from a channel-dir path until we find a `channels` segment;
/// the parent of that segment is `channels_root`, and everything beneath
/// it (joined by `:`) is the handle. Returns None if no `channels`
/// ancestor is present.
fn split_into_channels_root_and_handle(channel_dir: &std::path::Path) -> Option<(PathBuf, String)> {
    let mut segments: Vec<String> = Vec::new();
    let mut cur = channel_dir;
    loop {
        let name = cur.file_name()?.to_string_lossy().into_owned();
        if name == "channels" {
            let parent = cur.parent()?;
            segments.reverse();
            return Some((cur.to_path_buf(), segments.join(":"))).map(|(channels_root, h)| {
                let _ = parent; // suppress unused; we use channels_root which IS the `channels` dir
                (channels_root, h)
            });
        }
        segments.push(name);
        cur = cur.parent()?;
    }
}

fn preview_of(body: &str) -> String {
    let mut out = String::new();
    let mut line_count = 0usize;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() && out.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(trimmed);
        line_count += 1;
        if line_count >= PREVIEW_LINES || out.chars().count() >= PREVIEW_CHAR_BUDGET {
            break;
        }
    }
    if out.chars().count() > PREVIEW_CHAR_BUDGET {
        let mut truncated: String = out.chars().take(PREVIEW_CHAR_BUDGET).collect();
        truncated.push('…');
        truncated
    } else {
        out
    }
}
