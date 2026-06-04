---
tag: spec
status: design
date: 2026-06-01T00:00:00.000Z
slice_id: keystone
extends: docs/ideas/2026-05-31-git-native-substrate.md
---

# Spec — Repo registry (the git-native keystone)

## Premise

Post-teardown, Secretariat is "markdown editor + Touch-ID stamp over git
repos." The substrate *is* a set of repos. But nothing names that set — the
git-native note (`docs/ideas/2026-05-31-git-native-substrate.md`, Open
Question #6) flagged repo registration as the missing prerequisite for
everything downstream.

This spec builds that registry. It is the **substrate manifest**: the single
list of "which repos are in my world," owned by Secretariat (the HELM),
consumed by the review walker, the launcher, and background dispatch.

## North star — one list, three verbs

The registry exists to power three operations over **any** registered repo:

1. **Quick-launch a session** — `sec launch` / edit-with-Claude into a repo's cwd.
2. **Launch background sessions** — `sec dispatch` runs headless `claude -p` per repo on a schedule (`docs/pitches/2026-06-01-background-sessions.md`).
3. **Review knowledge across repos** — the altitude-aware review walker rolls up state + recency across every registered repo.

Penceive integration (index/search/enrich over the same repos) comes in a
**later slice** — see "Out of scope."

## Data model

A first-class `RepoRegistry` query concept, over a `repos` list **serialized
inside** **`preferences.toml`** (one config file; no rival file). Walker-facing
and (later) penceive-facing code takes a `RepoRegistry` view, never the whole
`Preferences`.

```rust
// crates/core/src/infrastructure/repo_registry.rs (new module)

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RepoRole {
    #[default]
    Project,   // designs live with code; pushable
    Home,      // cross-cutting PKM / journals; may never push; private
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoEntry {
    pub path: PathBuf,                       // absolute; the identity key
    #[serde(default)]
    pub role: RepoRole,                      // defaults to project
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,                   // free-form grouping (themia, equanimitech)
}

/// Borrowed query view over the registry slice. Constructed by
/// `Preferences::registry()`. The walker/launcher/penceive take this, not the
/// whole `Preferences`.
pub struct RepoRegistry<'a>(&'a [RepoEntry]);

impl<'a> RepoRegistry<'a> {
    pub fn iter(&self) -> impl Iterator<Item = &RepoEntry>;
    pub fn with_tag(&self, tag: &str) -> impl Iterator<Item = &RepoEntry>;
    pub fn home_repos(&self) -> impl Iterator<Item = &RepoEntry>;
    pub fn find(&self, path: &Path) -> Option<&RepoEntry>;   // canonicalized compare
}
```

Embedded in `Preferences` as a **direct field — no** **`#[serde(flatten)]`**.
A plain `Vec<RepoEntry>` renders top-level `[[repos]]` natively; flatten +
TOML array-of-tables is historically fragile, so we avoid it:

```rust
pub struct Preferences {
    #[serde(default)]
    pub composition: CompositionPrefs,
    #[serde(default)]
    pub cognition: CognitionPrefs,
    #[serde(default)]
    pub delivery: DeliveryPrefs,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<RepoEntry>,    // renders top-level `[[repos]]`
}

impl Preferences {
    pub fn registry(&self) -> RepoRegistry<'_> { RepoRegistry(&self.repos) }
}
```

**TOML key-ordering note:** the `toml` crate requires scalar keys before
tables/arrays-of-tables at a given level. `Preferences` has no top-level
scalars (every field is a sub-table or the `[[repos]]` array), so ordering is
safe. The implementation must still assert a save→load round-trip test to
lock this.

On-disk:

```toml
[cognition]
launch_command = "claude"

[[repos]]
path = "/Users/rafa/Developer/equanimitech/themia"
role = "project"
tags = ["themia"]

[[repos]]
path = "/Users/rafa/knowledge"
role = "home"
tags = ["equanimitech", "personal"]
```

### Decisions baked in

* **`path`** **is identity.** Add/remove/dedupe key. Canonicalized to absolute on
  insert (relative `.` from cwd resolves).

* **`role`** **gates behavior;** **`tags`** **group.** Per the project rule "roles are
  capabilities, not badges." `role` is a typed kind that changes behavior
  (`home` ⇒ private, may-never-push); `tags` are free-form labels that replace
  the deleted organizations concept.

* **`role = home`** **⇒ private signal.** No separate `private` field. The future
  penceive slice maps `home` → penceive `private-roots`. Documented here so
  the mapping is intentional, not incidental.

* **YAGNI.** No `background` / `review` sub-tables yet — those land with the
  background-sessions and review-walker slices. `#[serde(default)]` keeps the
  struct forward-compatible.

* **No** **`enabled`** **flag.** Presence = enrolled; `repo remove` = unenrolled.

* **No** **`[[sources]]`** **generalization.** The registry is git repos. Non-git
  knowledge sources (Supernote, zenborg JSON) are penceive's multi-source
  concern, not the registry's. Zenborg, if wanted in-substrate, is
  `git init`'d and enrolled as an ordinary `home` repo.

## Surfaces

Per AGENTS.md: every principal-facing primitive ships on use case + CLI + MCP

* tests.

### Use case — `crates/core/src/application/repo_ops.rs`

Pure orchestration; IO via existing `Preferences::load` / `save`.

```rust
pub fn register_repo(
    prefs_path: &Path, repo_path: &Path, role: RepoRole, tags: Vec<String>,
) -> Result<RepoEntry, RepoOpsError>;        // canonicalize → abs, validate git, upsert, save

pub fn list_repos(
    prefs_path: &Path, tag_filter: Option<&str>,
) -> Result<Vec<RepoEntry>, RepoOpsError>;

pub fn unregister_repo(
    prefs_path: &Path, repo_path: &Path,
) -> Result<bool, RepoOpsError>;             // Ok(false) if path absent
```

Behavior:

* **Upsert.** `register_repo` on an existing path updates its `role` + `tags`,
  idempotent — does not duplicate.

* **Validation.** `repo_path` must exist and contain `.git/`; else
  `RepoOpsError::NotAGitRepo { path }` ("run git init first"). The substrate is
  git-native; an unversioned dir is not substrate.

* **Canonicalization.** `std::fs::canonicalize` to an absolute path before
  store/compare, so `sec repo add .` and an absolute re-add resolve to one
  entry.

### CLI — `crates/cli/src/commands/repo.rs` (registered in `main.rs`)

```
sec repo add <path> [--role project|home] [--tag <t>]...
sec repo list [--tag <t>] [--json]
sec repo remove <path>
```

* `--role` defaults to `project`. `--tag` repeatable.

* `list` human table by default; `--json` for machine/agent consumption.

* `remove` reports whether anything was removed.

### MCP — `crates/mcp/src/server.rs`

`repo_add` / `repo_list` / `repo_remove` — same parameter shape as the CLI
flags, return `RepoEntry` / `Vec<RepoEntry>` (matching the use-case output).
Lets Claude (the HELM operator) enroll repos via MCP; principal via CLI.

## Error handling

`RepoOpsError` (thiserror):

* `NotAGitRepo { path }` — path missing or no `.git/`.

* `Io { path, source }` — canonicalize / prefs read-write failure.

* `Preferences(#[from] PreferencesError)` — load/save/validate propagation.

CLI maps these to non-zero exit + a one-line message; MCP maps to `ErrorData`.

## Testing

**Unit** (`repo_ops` + `repo_registry`):

* register canonicalizes a relative path to absolute and appends.

* register on a duplicate path upserts (role/tags updated, no dup).

* unregister removes by path; returns `false` for an absent path.

* list filters by tag; returns all when filter is `None`.

* TOML round-trip: `[[repos]]` save → load → equal.

* `role` defaults to `project` when omitted; empty `tags` skip-serialized.

* reject a non-git path with `NotAGitRepo`.

* omitted `[[repos]]` in an older `preferences.toml` deserializes to an empty
  registry (back-compat with the existing prefs-migration tests).

**CLI integration** (`SECRETARIAT_HOME` tmp): `add → list → remove` round-trip.

**MCP integration**: one tool round-trip (`repo_add` then `repo_list`).

**Quality gate:** `cargo test --workspace` + `cargo clippy -- -D warnings`.

## Out of scope (this slice)

* **Penceive wiring** — `penceive-core` dep, `reindex_repo` on enroll,
  `wake install-hook`, `role=home → private-roots`. Its own next slice.

* **Background / review sub-tables** in `RepoEntry` — land with their pitches.

* **Repoint the review walker** at the registry + the recency axis — next,
  consumes this.

* **Tauri UI** for repo management — CLI + MCP first; a Settings surface later.

* **`[[sources]]`** **/ non-git sources** — penceive's concern, never the
  registry's.

## Downstream consumers (after this lands)

| Consumer                        | Reads                                 | For                               |
| ------------------------------- | ------------------------------------- | --------------------------------- |
| `sec launch` / edit-with-Claude | `RepoEntry.path`                      | cwd to start a session            |
| `sec dispatch` (background)     | registry + per-repo bg config (later) | headless `claude -p` per repo     |
| review walker                   | registry + git + verify               | cross-repo state + recency rollup |
| penceive (later)                | `RepoEntry.path`, `role`              | index/search; `home → private`    |

