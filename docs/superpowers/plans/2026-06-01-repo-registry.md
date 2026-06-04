# Repo Registry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the git-native substrate manifest — a `[[repos]]` registry in `preferences.toml` with `path`/`role`/`tags`, exposed on use-case + CLI + MCP surfaces, so any repo can be quick-launched, background-dispatched, or reviewed.

**Architecture:** A `RepoEntry` value (path/role/tags) and a borrowed `RepoRegistry` query view live in a new `infrastructure/repo_registry.rs`. `Preferences` gains a direct `repos: Vec<RepoEntry>` field (renders top-level `[[repos]]`; no `#[serde(flatten)]`). A pure `application/repo_ops.rs` use case (register/list/unregister) does canonicalize + git-validate + upsert over `Preferences::load`/`save`. CLI `sec repo` and MCP `repo_*` tools wrap the use case identically.

**Tech Stack:** Rust, serde + `toml` crate, clap (derive), rmcp (`#[tool]`), thiserror, tempfile (tests).

**Spec:** `docs/superpowers/specs/2026-06-01-repo-registry-design.md`

---

## File Structure

- **Create** `crates/core/src/infrastructure/repo_registry.rs` — `RepoRole`, `RepoEntry`, `RepoRegistry<'a>` view + helpers. (Co-located with `preferences.rs`; config data, not domain.)
- **Modify** `crates/core/src/infrastructure/preferences.rs` — add `repos: Vec<RepoEntry>` to `Preferences`; add `Preferences::registry()`.
- **Modify** `crates/core/src/infrastructure/mod.rs` — `pub mod repo_registry;` + re-export.
- **Create** `crates/core/src/application/repo_ops.rs` — `register_repo` / `list_repos` / `unregister_repo` + `RepoOpsError`.
- **Modify** `crates/core/src/application/mod.rs` — `pub mod repo_ops;` + re-export.
- **Create** `crates/cli/src/commands/repo.rs` — `sec repo add/list/remove`.
- **Modify** `crates/cli/src/commands/mod.rs` — `pub mod repo;`.
- **Modify** `crates/cli/src/main.rs` — register `Repo` subcommand.
- **Modify** `crates/mcp/src/server.rs` — `repo_add` / `repo_list` / `repo_remove` tools + DTOs.
- **Create** `crates/cli/tests/repo_cli.rs` — CLI integration round-trip.
- **Modify** `AGENTS.md` — add `repo` to the CLI + MCP tool lists.

---

## Task 1: Data model — `RepoRole`, `RepoEntry`, `RepoRegistry` view

**Files:**
- Create: `crates/core/src/infrastructure/repo_registry.rs`
- Modify: `crates/core/src/infrastructure/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/infrastructure/repo_registry.rs`:

```rust
//! The substrate manifest: the list of git repos Secretariat treats as its
//! world. Serialized as top-level `[[repos]]` inside `preferences.toml`
//! (see `preferences.rs`). `RepoRole` gates behavior; `tags` group (the
//! org-replacement). `RepoRegistry` is a borrowed query view over the slice.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What kind of repo this is. Gates behavior, per "roles are capabilities,
/// not badges": `Home` repos are private (cross-cutting PKM, may never push)
/// and map to penceive `private-roots` in the later penceive slice.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RepoRole {
    /// Designs live with code; pushable.
    #[default]
    Project,
    /// Cross-cutting personal-knowledge / journals; private.
    Home,
}

impl RepoRole {
    /// Parse a CLI/MCP string into a role.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "project" => Ok(Self::Project),
            "home" => Ok(Self::Home),
            other => Err(format!("unknown role `{other}` (expected project|home)")),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Home => "home",
        }
    }
}

/// One enrolled repo. `path` is the identity (canonicalized absolute).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoEntry {
    pub path: PathBuf,
    #[serde(default)]
    pub role: RepoRole,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// Borrowed query view over the registry slice. The walker / launcher /
/// (later) penceive take this, not the whole `Preferences`.
pub struct RepoRegistry<'a>(&'a [RepoEntry]);

impl<'a> RepoRegistry<'a> {
    pub fn new(entries: &'a [RepoEntry]) -> Self {
        Self(entries)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RepoEntry> {
        self.0.iter()
    }

    /// Repos carrying `tag`.
    pub fn with_tag(&self, tag: &str) -> impl Iterator<Item = &RepoEntry> {
        let tag = tag.to_string();
        self.0.iter().filter(move |e| e.tags.iter().any(|t| t == &tag))
    }

    /// Repos with `role = Home` (the private PKM repos).
    pub fn home_repos(&self) -> impl Iterator<Item = &RepoEntry> {
        self.0.iter().filter(|e| e.role == RepoRole::Home)
    }

    /// Find an entry by exact (already-canonicalized) path.
    pub fn find(&self, path: &Path) -> Option<&RepoEntry> {
        self.0.iter().find(|e| e.path == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<RepoEntry> {
        vec![
            RepoEntry {
                path: PathBuf::from("/a/themia"),
                role: RepoRole::Project,
                tags: vec!["themia".into()],
            },
            RepoEntry {
                path: PathBuf::from("/a/knowledge"),
                role: RepoRole::Home,
                tags: vec!["equanimitech".into(), "personal".into()],
            },
        ]
    }

    #[test]
    fn role_parse_roundtrips() {
        assert_eq!(RepoRole::parse("project").unwrap(), RepoRole::Project);
        assert_eq!(RepoRole::parse("home").unwrap(), RepoRole::Home);
        assert!(RepoRole::parse("nope").is_err());
        assert_eq!(RepoRole::Home.as_str(), "home");
    }

    #[test]
    fn role_defaults_to_project() {
        assert_eq!(RepoRole::default(), RepoRole::Project);
    }

    #[test]
    fn with_tag_filters() {
        let e = entries();
        let reg = RepoRegistry::new(&e);
        let hits: Vec<_> = reg.with_tag("themia").collect();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, PathBuf::from("/a/themia"));
        assert_eq!(reg.with_tag("nope").count(), 0);
    }

    #[test]
    fn home_repos_filters_by_role() {
        let e = entries();
        let reg = RepoRegistry::new(&e);
        let homes: Vec<_> = reg.home_repos().collect();
        assert_eq!(homes.len(), 1);
        assert_eq!(homes[0].path, PathBuf::from("/a/knowledge"));
    }

    #[test]
    fn find_by_path() {
        let e = entries();
        let reg = RepoRegistry::new(&e);
        assert!(reg.find(Path::new("/a/themia")).is_some());
        assert!(reg.find(Path::new("/a/missing")).is_none());
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/core/src/infrastructure/mod.rs`, add `pub mod repo_registry;` alphabetically (after `queue_dir;`), and add a re-export near the other `pub use` lines:

```rust
pub use repo_registry::{RepoEntry, RepoRegistry, RepoRole};
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p secretariat-core repo_registry`
Expected: PASS (5 tests).

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p secretariat-core -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/infrastructure/repo_registry.rs crates/core/src/infrastructure/mod.rs
git commit -m "feat(core): RepoEntry + RepoRegistry view (substrate manifest model)"
```

---

## Task 2: Embed `repos` in `Preferences`

**Files:**
- Modify: `crates/core/src/infrastructure/preferences.rs`

- [ ] **Step 1: Write the failing test**

In `crates/core/src/infrastructure/preferences.rs`, add to the `mod tests` block:

```rust
#[test]
fn repos_round_trip_via_toml() {
    use super::super::repo_registry::{RepoEntry, RepoRole};
    use std::path::PathBuf;
    let d = dir();
    let path = d.path().join("preferences.toml");
    let mut prefs = Preferences::default();
    prefs.cognition.launch_command = "claude".into();
    prefs.repos = vec![
        RepoEntry {
            path: PathBuf::from("/Users/rafa/Developer/themia"),
            role: RepoRole::Project,
            tags: vec!["themia".into()],
        },
        RepoEntry {
            path: PathBuf::from("/Users/rafa/knowledge"),
            role: RepoRole::Home,
            tags: vec!["equanimitech".into()],
        },
    ];
    prefs.save(&path).unwrap();
    let loaded = Preferences::load(&path).unwrap();
    assert_eq!(loaded, prefs);
    assert_eq!(loaded.registry().home_repos().count(), 1);
}

#[test]
fn missing_repos_deserializes_to_empty() {
    let d = dir();
    let path = d.path().join("preferences.toml");
    // Older preferences.toml with no [[repos]].
    std::fs::write(&path, "[cognition]\nlaunch_command = \"claude\"\n").unwrap();
    let loaded = Preferences::load(&path).unwrap();
    assert!(loaded.repos.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p secretariat-core repos_round_trip_via_toml`
Expected: FAIL — `no field `repos` on type `Preferences``.

- [ ] **Step 3: Add the field + accessor**

In `preferences.rs`, add the import near the top (after the `use` block):

```rust
use crate::infrastructure::repo_registry::{RepoEntry, RepoRegistry};
```

Add the field to `struct Preferences` (after `delivery`):

```rust
    /// The substrate manifest — git repos Secretariat treats as its world.
    /// Renders top-level `[[repos]]`. No `#[serde(flatten)]`: a direct
    /// `Vec` renders the array-of-tables natively and dodges flatten's TOML
    /// fragility. `Preferences` has no top-level scalar keys, so the array
    /// ordering constraint is satisfied.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<RepoEntry>,
```

Add the accessor to `impl Preferences` (after `validate`):

```rust
    /// Borrowed query view over the enrolled repos.
    pub fn registry(&self) -> RepoRegistry<'_> {
        RepoRegistry::new(&self.repos)
    }
```

- [ ] **Step 4: Run the full preferences test module**

Run: `cargo test -p secretariat-core preferences`
Expected: PASS — both new tests plus all pre-existing ones (the existing `roundtrip_save_load` etc. still pass because `repos` defaults to empty + is skip-serialized).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/infrastructure/preferences.rs
git commit -m "feat(core): embed [[repos]] manifest in Preferences"
```

---

## Task 3: Use case — `repo_ops`

**Files:**
- Create: `crates/core/src/application/repo_ops.rs`
- Modify: `crates/core/src/application/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/application/repo_ops.rs`:

```rust
//! Repo-registry use case: enroll / list / unenroll repos in the substrate
//! manifest (`preferences.toml` `[[repos]]`). Pure orchestration; IO is
//! `Preferences::load` / `save` + a `.git/` existence check.
//!
//! `path` is identity: `register_repo` upserts (updates role/tags on an
//! existing path, never duplicates). Paths are canonicalized to absolute so
//! `sec repo add .` and an absolute re-add resolve to one entry.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::infrastructure::preferences::{Preferences, PreferencesError};
use crate::infrastructure::repo_registry::{RepoEntry, RepoRole};

#[derive(Debug, Error)]
pub enum RepoOpsError {
    #[error("not a git repo: {path} — run `git init` there first")]
    NotAGitRepo { path: PathBuf },
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Preferences(#[from] PreferencesError),
}

/// Enroll (or update) a repo. Canonicalizes `repo_path`, requires it contain
/// `.git/`, then upserts by path and saves.
pub fn register_repo(
    prefs_path: &Path,
    repo_path: &Path,
    role: RepoRole,
    tags: Vec<String>,
) -> Result<RepoEntry, RepoOpsError> {
    let abs = std::fs::canonicalize(repo_path).map_err(|source| RepoOpsError::Io {
        path: repo_path.to_path_buf(),
        source,
    })?;
    if !abs.join(".git").exists() {
        return Err(RepoOpsError::NotAGitRepo { path: abs });
    }

    let mut prefs = Preferences::load(prefs_path)?;
    let entry = RepoEntry {
        path: abs.clone(),
        role,
        tags,
    };
    if let Some(existing) = prefs.repos.iter_mut().find(|e| e.path == abs) {
        *existing = entry.clone();
    } else {
        prefs.repos.push(entry.clone());
    }
    prefs.save(prefs_path)?;
    Ok(entry)
}

/// List enrolled repos, optionally filtered to those carrying `tag`.
pub fn list_repos(
    prefs_path: &Path,
    tag_filter: Option<&str>,
) -> Result<Vec<RepoEntry>, RepoOpsError> {
    let prefs = Preferences::load(prefs_path)?;
    let out = match tag_filter {
        Some(tag) => prefs.registry().with_tag(tag).cloned().collect(),
        None => prefs.repos.clone(),
    };
    Ok(out)
}

/// Unenroll a repo by path. Canonicalizes first so `.` matches the stored
/// absolute path. Returns `false` if nothing matched.
pub fn unregister_repo(prefs_path: &Path, repo_path: &Path) -> Result<bool, RepoOpsError> {
    // Best-effort canonicalize: if the dir is gone we can still remove a
    // stale entry by its literal path.
    let target = std::fs::canonicalize(repo_path).unwrap_or_else(|_| repo_path.to_path_buf());
    let mut prefs = Preferences::load(prefs_path)?;
    let before = prefs.repos.len();
    prefs.repos.retain(|e| e.path != target);
    let removed = prefs.repos.len() != before;
    if removed {
        prefs.save(prefs_path)?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A tempdir that is a git repo (has `.git/`), plus the prefs path.
    fn repo_and_prefs() -> (TempDir, PathBuf, PathBuf) {
        let d = TempDir::new().unwrap();
        let repo = d.path().join("themia");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let prefs = d.path().join("preferences.toml");
        (d, repo, prefs)
    }

    #[test]
    fn register_canonicalizes_and_appends() {
        let (_d, repo, prefs) = repo_and_prefs();
        let entry = register_repo(&prefs, &repo, RepoRole::Project, vec!["themia".into()]).unwrap();
        assert!(entry.path.is_absolute());
        assert_eq!(entry.role, RepoRole::Project);
        let listed = list_repos(&prefs, None).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].tags, vec!["themia".to_string()]);
    }

    #[test]
    fn register_upserts_on_duplicate_path() {
        let (_d, repo, prefs) = repo_and_prefs();
        register_repo(&prefs, &repo, RepoRole::Project, vec!["a".into()]).unwrap();
        register_repo(&prefs, &repo, RepoRole::Home, vec!["b".into()]).unwrap();
        let listed = list_repos(&prefs, None).unwrap();
        assert_eq!(listed.len(), 1, "upsert, not duplicate");
        assert_eq!(listed[0].role, RepoRole::Home);
        assert_eq!(listed[0].tags, vec!["b".to_string()]);
    }

    #[test]
    fn register_rejects_non_git_dir() {
        let d = TempDir::new().unwrap();
        let plain = d.path().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let prefs = d.path().join("preferences.toml");
        let err = register_repo(&prefs, &plain, RepoRole::Project, vec![]).unwrap_err();
        assert!(matches!(err, RepoOpsError::NotAGitRepo { .. }));
    }

    #[test]
    fn list_filters_by_tag() {
        let d = TempDir::new().unwrap();
        let prefs = d.path().join("preferences.toml");
        for (name, tag) in [("themia", "themia"), ("zen", "equanimitech")] {
            let r = d.path().join(name);
            std::fs::create_dir_all(r.join(".git")).unwrap();
            register_repo(&prefs, &r, RepoRole::Project, vec![tag.into()]).unwrap();
        }
        assert_eq!(list_repos(&prefs, Some("themia")).unwrap().len(), 1);
        assert_eq!(list_repos(&prefs, Some("equanimitech")).unwrap().len(), 1);
        assert_eq!(list_repos(&prefs, None).unwrap().len(), 2);
    }

    #[test]
    fn unregister_removes_and_reports() {
        let (_d, repo, prefs) = repo_and_prefs();
        register_repo(&prefs, &repo, RepoRole::Project, vec![]).unwrap();
        assert!(unregister_repo(&prefs, &repo).unwrap());
        assert!(list_repos(&prefs, None).unwrap().is_empty());
        // Second remove is a no-op.
        assert!(!unregister_repo(&prefs, &repo).unwrap());
    }
}
```

- [ ] **Step 2: Register the module + re-export**

In `crates/core/src/application/mod.rs` add `pub mod repo_ops;` (after `org_ops;`) and a re-export:

```rust
pub use repo_ops::{list_repos, register_repo, unregister_repo, RepoOpsError};
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p secretariat-core repo_ops`
Expected: PASS (5 tests).

- [ ] **Step 4: Clippy**

Run: `cargo clippy -p secretariat-core -- -D warnings`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/application/repo_ops.rs crates/core/src/application/mod.rs
git commit -m "feat(core): repo_ops use case (register/list/unregister)"
```

---

## Task 4: CLI — `sec repo add/list/remove`

**Files:**
- Create: `crates/cli/src/commands/repo.rs`
- Modify: `crates/cli/src/commands/mod.rs`
- Modify: `crates/cli/src/main.rs`
- Create: `crates/cli/tests/repo_cli.rs`

- [ ] **Step 1: Write the command module**

Create `crates/cli/src/commands/repo.rs`:

```rust
//! `sec repo` — manage the substrate manifest (`preferences.toml` `[[repos]]`).
//!
//! - `sec repo add <path> [--role project|home] [--tag <t>]...`
//! - `sec repo list [--tag <t>] [--json]`
//! - `sec repo remove <path>`

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

use secretariat_core::application::repo_ops;
use secretariat_core::infrastructure::RepoRole;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Enroll (or update) a repo in the substrate manifest.
    Add {
        /// Path to the repo (must be a git repo).
        path: PathBuf,
        /// project (default) or home. `home` = private cross-cutting PKM.
        #[arg(long, default_value = "project")]
        role: String,
        /// Free-form grouping tag; repeatable (e.g. --tag themia).
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// List enrolled repos.
    List {
        /// Only repos carrying this tag.
        #[arg(long)]
        tag: Option<String>,
        /// Emit JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Unenroll a repo by path.
    Remove {
        /// Path to the repo to unenroll.
        path: PathBuf,
    },
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;
    match args.cmd {
        Cmd::Add { path, role, tags } => add(&paths.preferences, path, role, tags),
        Cmd::List { tag, json } => list(&paths.preferences, tag, json),
        Cmd::Remove { path } => remove(&paths.preferences, path),
    }
}

fn add(prefs: &std::path::Path, path: PathBuf, role: String, tags: Vec<String>) -> Result<()> {
    let role = RepoRole::parse(&role).map_err(|e| anyhow!("invalid role: {e}"))?;
    let entry = repo_ops::register_repo(prefs, &path, role, tags)?;
    eprintln!(
        "[sec] repo enrolled: {} ({}){}",
        entry.path.display(),
        entry.role.as_str(),
        if entry.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", entry.tags.join(", "))
        }
    );
    Ok(())
}

fn list(prefs: &std::path::Path, tag: Option<String>, json: bool) -> Result<()> {
    let repos = repo_ops::list_repos(prefs, tag.as_deref())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&repos)?);
        return Ok(());
    }
    if repos.is_empty() {
        eprintln!("[sec] no repos enrolled — `sec repo add <path>` to enroll one");
        return Ok(());
    }
    for r in &repos {
        println!(
            "{role}\t{tags}\t{path}",
            role = r.role.as_str(),
            tags = r.tags.join(","),
            path = r.path.display()
        );
    }
    Ok(())
}

fn remove(prefs: &std::path::Path, path: PathBuf) -> Result<()> {
    let removed = repo_ops::unregister_repo(prefs, &path)?;
    if removed {
        eprintln!("[sec] repo unenrolled: {}", path.display());
    } else {
        eprintln!("[sec] not enrolled: {}", path.display());
    }
    Ok(())
}
```

- [ ] **Step 2: Register the command**

In `crates/cli/src/commands/mod.rs`, add `pub mod repo;` (after `read;`).

In `crates/cli/src/main.rs`, add the variant to `enum Cmd` (after `Read`):

```rust
    /// Manage the substrate manifest: enroll / list / unenroll git repos.
    Repo(commands::repo::Args),
```

and the match arm in `main()` (after the `Read` arm):

```rust
        Cmd::Repo(a) => commands::repo::run(a),
```

- [ ] **Step 3: Build to verify it compiles**

Run: `cargo build -p secretariat-cli`
Expected: success.

- [ ] **Step 4: Write the CLI integration test**

Create `crates/cli/tests/repo_cli.rs`:

```rust
//! `sec repo add → list → remove` round-trip under a temp SECRETARIAT_HOME.

use std::process::Command;

use tempfile::TempDir;

fn sec() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sec"))
}

#[test]
fn repo_add_list_remove_roundtrip() {
    let home = TempDir::new().unwrap();
    // The repo to enroll: a git dir inside the temp home.
    let repo = home.path().join("themia");
    std::fs::create_dir_all(repo.join(".git")).unwrap();

    // add
    let out = sec()
        .env("SECRETARIAT_HOME", home.path())
        .args(["repo", "add"])
        .arg(&repo)
        .args(["--role", "project", "--tag", "themia"])
        .output()
        .unwrap();
    assert!(out.status.success(), "add failed: {out:?}");

    // list --json contains the repo
    let out = sec()
        .env("SECRETARIAT_HOME", home.path())
        .args(["repo", "list", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("themia"), "list missing repo: {stdout}");

    // remove
    let out = sec()
        .env("SECRETARIAT_HOME", home.path())
        .args(["repo", "remove"])
        .arg(&repo)
        .output()
        .unwrap();
    assert!(out.status.success());

    // list is now empty
    let out = sec()
        .env("SECRETARIAT_HOME", home.path())
        .args(["repo", "list", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "[]");
}
```

- [ ] **Step 5: Add the dev-dependency**

`tempfile` was dropped from `crates/cli`'s `[dev-dependencies]` in the teardown but survives as a runtime dep. Confirm it resolves for tests:

Run: `cargo test -p secretariat-cli --test repo_cli`
Expected: PASS. If it fails with `unresolved import tempfile`, add to `crates/cli/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = { workspace = true }
```

then re-run.

- [ ] **Step 6: Clippy**

Run: `cargo clippy -p secretariat-cli -- -D warnings`
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add crates/cli/src/commands/repo.rs crates/cli/src/commands/mod.rs crates/cli/src/main.rs crates/cli/tests/repo_cli.rs crates/cli/Cargo.toml
git commit -m "feat(cli): sec repo add/list/remove"
```

---

## Task 5: MCP — `repo_add` / `repo_list` / `repo_remove`

**Files:**
- Modify: `crates/mcp/src/server.rs`

- [ ] **Step 1: Add the parameter + output DTOs**

In `crates/mcp/src/server.rs`, near the other param/output structs (after `ListAgentsOutput`, ~line 210), add:

```rust
#[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RepoDto {
    /// Absolute path to the repo.
    pub path: String,
    /// `project` or `home`.
    pub role: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepoAddParams {
    /// Path to the repo (must be a git repo). Absolute preferred.
    pub path: String,
    /// `project` (default) or `home`.
    #[serde(default)]
    pub role: Option<String>,
    /// Free-form grouping tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepoListParams {
    /// Only repos carrying this tag.
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RepoRemoveParams {
    /// Path to the repo to unenroll.
    pub path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RepoListOutput {
    pub repos: Vec<RepoDto>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RepoRemoveOutput {
    pub removed: bool,
}
```

Add a conversion helper near the bottom (next to `invalid_request`):

```rust
fn repo_to_dto(e: secretariat_core::infrastructure::RepoEntry) -> RepoDto {
    RepoDto {
        path: e.path.display().to_string(),
        role: e.role.as_str().to_string(),
        tags: e.tags,
    }
}
```

- [ ] **Step 2: Add the three tools**

In the `#[tool_router] impl SecretariatServer` block (where `stamp` / `read` / `verify` / `agent_*` live), add:

```rust
    #[tool(
        name = "repo_add",
        annotations(destructive_hint = false, idempotent_hint = true, open_world_hint = false),
        description = "Enroll (or update) a git repo in the substrate manifest. `path` is \
        the identity — calling again with the same path updates its role/tags (upsert, no \
        duplicate). `role` is `project` (default) or `home` (private cross-cutting PKM). \
        `tags` are free-form grouping labels (e.g. themia, equanimitech). Fails if `path` \
        is not a git repo."
    )]
    async fn repo_add(
        &self,
        Parameters(params): Parameters<RepoAddParams>,
    ) -> Result<Json<RepoDto>, ErrorData> {
        use secretariat_core::application::repo_ops::register_repo;
        use secretariat_core::infrastructure::RepoRole;
        let role = RepoRole::parse(params.role.as_deref().unwrap_or("project"))
            .map_err(|e| invalid_request(format!("invalid role: {e}")))?;
        let entry = register_repo(
            &self.paths.preferences,
            std::path::Path::new(&params.path),
            role,
            params.tags,
        )
        .map_err(|e| invalid_request(format!("repo_add failed: {e}")))?;
        info!(path = %entry.path.display(), "repo enrolled via MCP");
        Ok(Json(repo_to_dto(entry)))
    }

    #[tool(
        name = "repo_list",
        annotations(read_only_hint = true, idempotent_hint = true, open_world_hint = false),
        description = "List repos enrolled in the substrate manifest, optionally filtered \
        to those carrying a given tag. Each entry: absolute path, role (project|home), tags."
    )]
    async fn repo_list(
        &self,
        Parameters(params): Parameters<RepoListParams>,
    ) -> Result<Json<RepoListOutput>, ErrorData> {
        use secretariat_core::application::repo_ops::list_repos;
        let repos = list_repos(&self.paths.preferences, params.tag.as_deref())
            .map_err(|e| invalid_request(format!("repo_list failed: {e}")))?;
        Ok(Json(RepoListOutput {
            repos: repos.into_iter().map(repo_to_dto).collect(),
        }))
    }

    #[tool(
        name = "repo_remove",
        annotations(destructive_hint = true, idempotent_hint = true, open_world_hint = false),
        description = "Unenroll a repo from the substrate manifest by path. Returns \
        `removed: false` if the path was not enrolled. Does not touch the repo's files — \
        only the manifest entry."
    )]
    async fn repo_remove(
        &self,
        Parameters(params): Parameters<RepoRemoveParams>,
    ) -> Result<Json<RepoRemoveOutput>, ErrorData> {
        use secretariat_core::application::repo_ops::unregister_repo;
        let removed = unregister_repo(&self.paths.preferences, std::path::Path::new(&params.path))
            .map_err(|e| invalid_request(format!("repo_remove failed: {e}")))?;
        Ok(Json(RepoRemoveOutput { removed }))
    }
```

- [ ] **Step 3: Build to verify the tools register**

Run: `cargo build -p secretariat-mcp`
Expected: success. (The `#[tool_router]` macro auto-registers the three new `#[tool]` methods.)

- [ ] **Step 4: Write a use-case-level MCP integration test**

In `crates/mcp/src/server.rs`, add a test module at the end (or extend an existing one). This exercises the same `repo_ops` path the tools call, under a temp prefs file:

```rust
#[cfg(test)]
mod repo_tool_tests {
    use secretariat_core::application::repo_ops::{list_repos, register_repo, unregister_repo};
    use secretariat_core::infrastructure::RepoRole;
    use tempfile::TempDir;

    #[test]
    fn repo_ops_roundtrip_under_temp_prefs() {
        let d = TempDir::new().unwrap();
        let repo = d.path().join("themia");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let prefs = d.path().join("preferences.toml");

        register_repo(&prefs, &repo, RepoRole::Home, vec!["themia".into()]).unwrap();
        let listed = list_repos(&prefs, Some("themia")).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].role, RepoRole::Home);

        assert!(unregister_repo(&prefs, &repo).unwrap());
        assert!(list_repos(&prefs, None).unwrap().is_empty());
    }
}
```

If `tempfile` is not a dev-dependency of `crates/mcp`, add it:

```toml
[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 5: Run the test + clippy**

Run: `cargo test -p secretariat-mcp repo_tool_tests`
Expected: PASS.

Run: `cargo clippy -p secretariat-mcp -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/mcp/src/server.rs crates/mcp/Cargo.toml
git commit -m "feat(mcp): repo_add/repo_list/repo_remove tools"
```

---

## Task 6: Docs — update AGENTS.md

**Files:**
- Modify: `AGENTS.md`

- [ ] **Step 1: Add `repo` to the CLI subcommand list**

In `AGENTS.md`, the "What's here today" CLI line lists subcommands. Add `repo` to it:

```
`init` / `agent` / `repo` / `stamp` / `verify` / `read` / `launch` / `mcp` /
`daemon` / `profile` / `view`
```

- [ ] **Step 2: Add the MCP tools to the MCP server line**

Update the MCP tools list to append the three new tools:

```
Tools: `stamp`, `read`, `verify`, `agent_add`, `agent_list`, `agent_remove`,
`agent_rotate`, `repo_add`, `repo_list`, `repo_remove`.
```

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "docs: register sec repo + repo_* MCP tools in AGENTS.md"
```

---

## Task 7: Final workspace gate

- [ ] **Step 1: Full workspace test**

Run: `cargo test --workspace`
Expected: all green (existing suite + the new repo_registry / preferences / repo_ops / repo_cli / repo_tool_tests).

- [ ] **Step 2: Full clippy**

Run: `cargo clippy --workspace -- -D warnings`
Expected: clean.

- [ ] **Step 3: Format**

Run: `cargo fmt --all`
Expected: no diff (or commit the formatting).

- [ ] **Step 4: Final commit (if fmt produced changes)**

```bash
git add -A
git commit -m "chore: cargo fmt after repo-registry slice"
```

---

## Verification / dogfood (manual, after merge)

Using the prod binary (`/Applications/Secretariat.app/Contents/MacOS/sec` once shipped, or `cargo run -p secretariat-cli --` locally):

```bash
sec repo add ~/Developer/equanimitech/secretariat --role project --tag equanimitech
sec repo list
```

> **Home repo note:** the `home`-role repo to enroll is a **new** personal-knowledge repo (saperene/Logseq is being abandoned), created separately — name/location TBD with the principal. Not part of this slice.

## Out of scope (confirms spec)

- Penceive wiring (`reindex_repo`, `wake install-hook`, `home → private-roots`) — next slice.
- `background` / `review` per-repo sub-tables — land with their pitches.
- Repointing the review walker + recency axis — consumes this, separate slice.
- Tauri settings UI for repo management.
