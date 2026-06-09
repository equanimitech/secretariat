# Stamp-Triggered Workflows (v0) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a document is stamped, run a principal-authored `.secretariat/workflows/*.md` against it (v0: app-side trigger, with a CLI dry-run path for testing).

**Architecture:** Pure domain value objects (`Workflow`, `WorkflowMatch`, `doc_type_from_path`) + an `application/workflow_ops.rs` use case that loads/parses workflow files and resolves matches (type from path, tags from the repo registry). Two callers: `sec workflow` CLI (list + dry-run + run) and the Tauri stamp command's post-stamp step. No daemon, no worktree, no MCP — all deferred per the spec.

**Tech Stack:** Rust, `serde_yaml` (already a core dep), `thiserror`, `clap`, Tauri v2. Mirrors the existing `repo_ops` / `repo.rs` patterns.

**Spec:** `docs/superpowers/specs/2026-06-09-stamp-workflow-trigger-design.md` (stamped).

---

## File structure

- Create `crates/core/src/domain/workflow.rs` — value objects + match logic + `doc_type_from_path` (pure).
- Modify `crates/core/src/domain/mod.rs` — `pub mod workflow;` + re-export.
- Create `crates/core/src/application/workflow_ops.rs` — load/parse/match (IO).
- Modify `crates/core/src/application/mod.rs` — `pub mod workflow_ops;` + re-export.
- Create `crates/cli/src/commands/workflow.rs` — `sec workflow list|run`.
- Modify `crates/cli/src/main.rs` — register `Workflow` subcommand.
- Modify `crates/cli/src/commands/mod.rs` — `pub mod workflow;`.
- Modify `src-tauri/src/commands/secretariat.rs` — post-stamp trigger step.

---

### Task 1: Domain — `WorkflowMatch` and the match logic

**Files:**
- Create: `crates/core/src/domain/workflow.rs`
- Modify: `crates/core/src/domain/mod.rs`

- [ ] **Step 1: Write the failing test** (append to a new `workflow.rs`)

```rust
//! Workflow value objects — the in-repo `.secretariat/workflows/*.md` shape.
//! Pure: no IO. File reading + YAML parsing live in `application::workflow_ops`.

use std::path::Path;

/// The trigger event. Only `stamp` ships in v0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StampEvent {
    Stamp,
}

impl StampEvent {
    pub fn parse(s: &str) -> Result<Self, WorkflowParseError> {
        match s {
            "stamp" => Ok(Self::Stamp),
            other => Err(WorkflowParseError::UnknownTrigger(other.to_string())),
        }
    }
}

/// Any-of filters. An empty vec means "unconstrained".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkflowMatch {
    pub types: Vec<String>,
    pub tags: Vec<String>,
}

impl WorkflowMatch {
    /// True when every PRESENT filter has a non-empty intersection with the
    /// inputs. `doc_type = None` only passes a type filter that is empty.
    pub fn matches(&self, doc_type: Option<&str>, repo_tags: &[String]) -> bool {
        let type_ok = self.types.is_empty()
            || doc_type.is_some_and(|t| self.types.iter().any(|x| x == t));
        let tag_ok = self.tags.is_empty()
            || self.tags.iter().any(|x| repo_tags.iter().any(|rt| rt == x));
        type_ok && tag_ok
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub on: StampEvent,
    pub match_: WorkflowMatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workflow {
    pub name: String,
    pub trigger: Trigger,
    pub prompt: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkflowParseError {
    #[error("unknown trigger `{0}` (expected: stamp)")]
    UnknownTrigger(String),
    #[error("missing or malformed frontmatter")]
    BadFrontmatter,
    #[error("yaml error: {0}")]
    Yaml(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_intersects_present_filters() {
        let m = WorkflowMatch {
            types: vec!["idea".into(), "pain".into()],
            tags: vec!["themia".into()],
        };
        assert!(m.matches(Some("pain"), &["themia".into()]));
        assert!(!m.matches(Some("spec"), &["themia".into()])); // type miss
        assert!(!m.matches(Some("idea"), &["equanimitech".into()])); // tag miss
        assert!(!m.matches(None, &["themia".into()])); // untyped doc, type filter present
    }

    #[test]
    fn empty_filter_is_unconstrained() {
        let m = WorkflowMatch::default();
        assert!(m.matches(None, &[]));
        assert!(m.matches(Some("anything"), &["whatever".into()]));
    }
}
```

- [ ] **Step 2: Wire the module.** In `crates/core/src/domain/mod.rs` add:

```rust
pub mod workflow;
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test -p secretariat-core domain::workflow -- --nocapture`
Expected: PASS (2 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/domain/workflow.rs crates/core/src/domain/mod.rs
git commit -m "feat(workflow): domain value objects + match logic"
```

---

### Task 2: Domain — `doc_type_from_path`

**Files:**
- Modify: `crates/core/src/domain/workflow.rs`

- [ ] **Step 1: Write the failing test** (add inside the existing `tests` module)

```rust
    #[test]
    fn type_is_immediate_subdir_under_docs() {
        use std::path::Path;
        assert_eq!(doc_type_from_path(Path::new("docs/pain/x.md")).as_deref(), Some("pain"));
        assert_eq!(doc_type_from_path(Path::new("docs/ideas/y.md")).as_deref(), Some("ideas"));
        // nested → immediate child only (per spec)
        assert_eq!(doc_type_from_path(Path::new("docs/superpowers/specs/z.md")).as_deref(), Some("superpowers"));
        // flat doc → untyped
        assert_eq!(doc_type_from_path(Path::new("docs/flat.md")), None);
        // not under docs/ → untyped
        assert_eq!(doc_type_from_path(Path::new("README.md")), None);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p secretariat-core domain::workflow::tests::type_is_immediate_subdir_under_docs`
Expected: FAIL — `doc_type_from_path` not found.

- [ ] **Step 3: Implement** (add to `workflow.rs`, above the `tests` module)

```rust
/// A doc's type = the immediate subdir under `docs/`. `None` for a flat
/// `docs/x.md` or any path not nested under `docs/`. Frontmatter `type:` (read
/// in `application`) overrides this.
pub fn doc_type_from_path(doc_rel: &Path) -> Option<String> {
    let mut comps = doc_rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    if comps.next().as_deref() != Some("docs") {
        return None;
    }
    let sub = comps.next()?; // immediate child of docs/
    // It is a directory only if at least one more component (the file) follows.
    comps.next().map(|_| sub)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p secretariat-core domain::workflow`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/domain/workflow.rs
git commit -m "feat(workflow): derive doc type from path"
```

---

### Task 3: Application — parse a workflow file

**Files:**
- Create: `crates/core/src/application/workflow_ops.rs`
- Modify: `crates/core/src/application/mod.rs`

- [ ] **Step 1: Write the failing test** (new `workflow_ops.rs`)

```rust
//! Workflow use case: load + parse `.secretariat/workflows/*.md`, resolve which
//! fire for a stamped doc. Pure orchestration; IO is fs reads + `Preferences`.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::domain::workflow::{
    doc_type_from_path, StampEvent, Trigger, Workflow, WorkflowMatch, WorkflowParseError,
};
use crate::infrastructure::preferences::{Preferences, PreferencesError};

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("workflow `{name}`: {source}")]
    Parse {
        name: String,
        #[source]
        source: WorkflowParseError,
    },
    #[error(transparent)]
    Preferences(#[from] PreferencesError),
}

#[derive(serde::Deserialize)]
struct RawTrigger {
    on: String,
    #[serde(default)]
    r#match: RawMatch,
}

#[derive(serde::Deserialize, Default)]
struct RawMatch {
    #[serde(default)]
    r#type: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

/// Split a leading `---\n…\n---` frontmatter block from the body.
fn split_frontmatter(s: &str) -> Option<(&str, &str)> {
    let rest = s.strip_prefix("---\n")?;
    let idx = rest.find("\n---")?;
    let yaml = &rest[..idx];
    let after = &rest[idx + 4..]; // skip "\n---"
    let body = after.strip_prefix('\n').unwrap_or(after);
    Some((yaml, body))
}

/// Parse one workflow file's content into a `Workflow`.
pub fn parse_workflow(name: &str, content: &str) -> Result<Workflow, WorkflowParseError> {
    let (yaml, body) = split_frontmatter(content).ok_or(WorkflowParseError::BadFrontmatter)?;
    let raw: RawTrigger =
        serde_yaml::from_str(yaml).map_err(|e| WorkflowParseError::Yaml(e.to_string()))?;
    let on = StampEvent::parse(&raw.on)?;
    Ok(Workflow {
        name: name.to_string(),
        trigger: Trigger {
            on,
            match_: WorkflowMatch {
                types: raw.r#match.r#type,
                tags: raw.r#match.tags,
            },
        },
        prompt: body.trim_start().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\non: stamp\nmatch:\n  type: [idea, pain]\n  tags: [themia]\n---\nDo the thing.\n";

    #[test]
    fn parse_extracts_trigger_and_prompt() {
        let wf = parse_workflow("to-linear", SAMPLE).unwrap();
        assert_eq!(wf.name, "to-linear");
        assert_eq!(wf.trigger.on, StampEvent::Stamp);
        assert_eq!(wf.trigger.match_.types, vec!["idea", "pain"]);
        assert_eq!(wf.trigger.match_.tags, vec!["themia"]);
        assert_eq!(wf.prompt, "Do the thing.");
    }

    #[test]
    fn parse_rejects_unknown_trigger() {
        let bad = "---\non: push\n---\nx";
        assert!(matches!(
            parse_workflow("x", bad),
            Err(WorkflowParseError::UnknownTrigger(_))
        ));
    }

    #[test]
    fn parse_rejects_missing_frontmatter() {
        assert!(matches!(
            parse_workflow("x", "no frontmatter here"),
            Err(WorkflowParseError::BadFrontmatter)
        ));
    }
}
```

- [ ] **Step 2: Wire the module.** In `crates/core/src/application/mod.rs` add `pub mod workflow_ops;` (alphabetical, after `verify_document`) and the re-export line:

```rust
pub mod workflow_ops;
pub use workflow_ops::{load_workflows, match_workflows, parse_workflow, WorkflowError};
```

(`load_workflows` / `match_workflows` are added in Task 4 — add the `pub use` now so Task 4 just fills them in; if the compiler complains about unresolved names, complete Task 4 before building.)

- [ ] **Step 3: Run to verify it passes**

Run: `cargo test -p secretariat-core application::workflow_ops`
Expected: PASS (3 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/application/workflow_ops.rs crates/core/src/application/mod.rs
git commit -m "feat(workflow): parse workflow files"
```

---

### Task 4: Application — load + match workflows

**Files:**
- Modify: `crates/core/src/application/workflow_ops.rs`

- [ ] **Step 1: Write the failing test** (add to the `tests` module)

```rust
    use tempfile::TempDir;

    /// A git repo with one workflow file + a registered prefs entry tagged `themia`.
    fn repo_with_workflow() -> (TempDir, PathBuf, PathBuf) {
        let d = TempDir::new().unwrap();
        let repo = d.path().join("minerva");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join(".secretariat/workflows")).unwrap();
        std::fs::write(repo.join(".secretariat/workflows/to-linear.md"), SAMPLE).unwrap();
        let prefs = d.path().join("preferences.toml");
        crate::application::repo_ops::register_repo(
            &prefs,
            &repo,
            crate::infrastructure::RepoRole::Project,
            vec!["themia".into()],
        )
        .unwrap();
        (d, repo, prefs)
    }

    #[test]
    fn load_reads_all_workflow_files() {
        let (_d, repo, _prefs) = repo_with_workflow();
        let wfs = load_workflows(&repo).unwrap();
        assert_eq!(wfs.len(), 1);
        assert_eq!(wfs[0].name, "to-linear");
    }

    #[test]
    fn load_absent_dir_is_empty_not_error() {
        let d = TempDir::new().unwrap();
        assert!(load_workflows(d.path()).unwrap().is_empty());
    }

    #[test]
    fn match_fires_for_typed_tagged_doc() {
        let (_d, repo, prefs) = repo_with_workflow();
        // type from path = "pain", repo tag = "themia" → matches
        let hits =
            match_workflows(&prefs, &repo, Path::new("docs/pain/x.md")).unwrap();
        assert_eq!(hits.len(), 1);
        // flat doc → untyped → type filter present → no match
        let none =
            match_workflows(&prefs, &repo, Path::new("docs/flat.md")).unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn match_respects_frontmatter_type_override() {
        let (_d, repo, prefs) = repo_with_workflow();
        // a flat doc that DECLARES type: idea in its own frontmatter → matches
        std::fs::create_dir_all(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/flat.md"), "---\ntype: idea\n---\nbody").unwrap();
        let hits = match_workflows(&prefs, &repo, Path::new("docs/flat.md")).unwrap();
        assert_eq!(hits.len(), 1);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p secretariat-core application::workflow_ops`
Expected: FAIL — `load_workflows` / `match_workflows` not found.

- [ ] **Step 3: Implement** (add to `workflow_ops.rs`, above `#[cfg(test)]`)

```rust
/// Load + parse every `.secretariat/workflows/*.md` in `repo`. An absent
/// directory is not an error — it means "no workflows".
pub fn load_workflows(repo: &Path) -> Result<Vec<Workflow>, WorkflowError> {
    let dir = repo.join(".secretariat/workflows");
    let rd = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for entry in rd {
        let entry = entry.map_err(|source| WorkflowError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let content = fs::read_to_string(&path).map_err(|source| WorkflowError::Io {
            path: path.clone(),
            source,
        })?;
        let wf = parse_workflow(&name, &content).map_err(|source| WorkflowError::Parse {
            name: name.clone(),
            source,
        })?;
        out.push(wf);
    }
    Ok(out)
}

/// Read a `type:` value from a doc's own frontmatter, if present.
fn frontmatter_type(doc_abs: &Path) -> Option<String> {
    let content = fs::read_to_string(doc_abs).ok()?;
    let (yaml, _) = split_frontmatter(&content)?;
    #[derive(serde::Deserialize)]
    struct Fm {
        r#type: Option<String>,
    }
    serde_yaml::from_str::<Fm>(yaml).ok()?.r#type
}

/// Workflows that fire for a just-stamped doc. Type = frontmatter `type:` if
/// present, else the path's immediate `docs/` subdir. Tags from the registry.
pub fn match_workflows(
    prefs_path: &Path,
    repo: &Path,
    doc_rel: &Path,
) -> Result<Vec<Workflow>, WorkflowError> {
    let workflows = load_workflows(repo)?;
    let doc_abs = repo.join(doc_rel);
    let doc_type = frontmatter_type(&doc_abs).or_else(|| doc_type_from_path(doc_rel));

    let prefs = Preferences::load(prefs_path)?;
    let abs = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    let tags = prefs
        .registry()
        .find(&abs)
        .map(|e| e.tags.clone())
        .unwrap_or_default();

    Ok(workflows
        .into_iter()
        .filter(|w| {
            matches!(w.trigger.on, StampEvent::Stamp)
                && w.trigger.match_.matches(doc_type.as_deref(), &tags)
        })
        .collect())
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p secretariat-core application::workflow_ops`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/application/workflow_ops.rs
git commit -m "feat(workflow): load + match workflows against a stamped doc"
```

---

### Task 5: CLI — `sec workflow list` and `sec workflow run --dry-run`

**Files:**
- Create: `crates/cli/src/commands/workflow.rs`
- Modify: `crates/cli/src/commands/mod.rs`, `crates/cli/src/main.rs`

- [ ] **Step 1: Implement the command** (`workflow.rs`)

```rust
//! `sec workflow` — inspect + fire `.secretariat/workflows/*.md`.
//!
//! - `sec workflow list [<repo>]`            — parsed workflows in a repo
//! - `sec workflow run <doc> [--dry-run]`    — fire matching workflows for a doc

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

use secretariat_core::application::workflow_ops;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// List parsed workflows in a repo (defaults to cwd).
    List { repo: Option<PathBuf> },
    /// Fire workflows matching a doc. `--dry-run` renders without dispatching.
    Run {
        /// Path to the stamped doc.
        doc: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    paths.ensure_dirs()?;
    match args.cmd {
        Cmd::List { repo } => list(repo),
        Cmd::Run { doc, dry_run } => run_doc(&paths.preferences, doc, dry_run),
    }
}

fn list(repo: Option<PathBuf>) -> Result<()> {
    let repo = repo.unwrap_or(std::env::current_dir()?);
    let wfs = workflow_ops::load_workflows(&repo).context("loading workflows")?;
    if wfs.is_empty() {
        eprintln!("[sec] no workflows in {}/.secretariat/workflows/", repo.display());
        return Ok(());
    }
    for w in &wfs {
        println!(
            "{name}\ton={on:?}\ttype={types:?}\ttags={tags:?}",
            name = w.name,
            on = w.trigger.on,
            types = w.trigger.match_.types,
            tags = w.trigger.match_.tags,
        );
    }
    Ok(())
}

/// Walk up from `doc` to the nearest enclosing git repo root.
fn repo_root_of(doc: &Path) -> Result<PathBuf> {
    let abs = std::fs::canonicalize(doc).context("resolving doc path")?;
    let mut dir = abs.parent();
    while let Some(d) = dir {
        if d.join(".git").exists() {
            return Ok(d.to_path_buf());
        }
        dir = d.parent();
    }
    Err(anyhow!("{} is not inside a git repo", doc.display()))
}

fn run_doc(prefs: &Path, doc: PathBuf, dry_run: bool) -> Result<()> {
    let repo = repo_root_of(&doc)?;
    let abs_doc = std::fs::canonicalize(&doc)?;
    let doc_rel = abs_doc
        .strip_prefix(&repo)
        .context("doc not under its repo root")?;
    let hits = workflow_ops::match_workflows(prefs, &repo, doc_rel)
        .context("matching workflows")?;
    if hits.is_empty() {
        eprintln!("[sec] no workflows match {}", doc.display());
        return Ok(());
    }
    for w in &hits {
        if dry_run {
            println!("--- would dispatch workflow `{}` ---", w.name);
            println!("cwd: {}", repo.display());
            println!("doc: {}", doc_rel.display());
            println!("prompt:\n{}", w.prompt);
        } else {
            // Real dispatch is wired in Task 6 (shared scribe-run path).
            eprintln!(
                "[sec] dispatch not yet wired for `{}` — use --dry-run (Task 6)",
                w.name
            );
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Register the command.** In `crates/cli/src/commands/mod.rs` add `pub mod workflow;`. In `crates/cli/src/main.rs`, add a variant to the `Cmd` enum (after `View`):

```rust
    /// Inspect + fire `.secretariat/workflows/*.md`.
    Workflow(commands::workflow::Args),
```

and a match arm (after the `View` arm):

```rust
        Cmd::Workflow(a) => commands::workflow::run(a),
```

- [ ] **Step 3: Build + manual dry-run check**

Run:
```bash
cargo build -p secretariat-cli
./target/debug/sec workflow list ~/Developer/themia/minerva
./target/debug/sec workflow run ~/Developer/themia/minerva/docs/pain/SOME-PAIN.md --dry-run
```
Expected: `list` prints `to-linear  on=Stamp  type=["idea", "pain"]  tags=["themia"]`. `run --dry-run` prints the rendered prompt for a `docs/pain/*.md` doc (no dispatch).

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/commands/workflow.rs crates/cli/src/commands/mod.rs crates/cli/src/main.rs
git commit -m "feat(cli): sec workflow list + run --dry-run"
```

---

### Task 6: Wire the real scribe dispatch (CLI `run` + Tauri post-stamp)

This is the one integration point that hits external services (spawns the cognition CLI, creates a Linear issue). It is verified manually, not by unit test — the spec's testing section mocks dispatch and asserts the dry-run dispatches nothing (already covered in Tasks 4–5).

**Files:**
- Modify: `crates/cli/src/commands/workflow.rs` (real branch of `run_doc`)
- Modify: `src-tauri/src/commands/secretariat.rs` (post-stamp step)

- [ ] **Step 1: Add a shared scribe-run helper.** Reuse the existing cognition path rather than inventing one. Read `src-tauri/src/commands/dispatch.rs` (`dispatch_send` / `compose_prompt`) and `crates/core/src/infrastructure/cognition` to find the headless-run entrypoint (`launch_command` from `Preferences`, spawned as `claude -p <prompt>` with `cwd`). Implement, in `workflow.rs`, the real branch of `run_doc`:

```rust
// Replace the `eprintln!("[sec] dispatch not yet wired …")` branch with:
let output = std::process::Command::new(cognition_command(prefs)?)
    .arg("-p")
    .arg(&w.prompt)
    .current_dir(&repo)
    .output()
    .with_context(|| format!("dispatching workflow `{}`", w.name))?;
if !output.status.success() {
    anyhow::bail!(
        "workflow `{}` failed: {}",
        w.name,
        String::from_utf8_lossy(&output.stderr)
    );
}
eprintln!("[sec] workflow `{}` dispatched", w.name);
```

with a small `cognition_command(prefs: &Path) -> Result<String>` that reads `launch_command` from `Preferences` (default `"claude"`). Confirm the exact `Preferences` field name against `crates/core/src/infrastructure/preferences.rs` before writing — do not guess it.

- [ ] **Step 2: Wire the Tauri post-stamp step.** In `src-tauri/src/commands/secretariat.rs`, locate the stamp command (the `#[tauri::command]` near the "stamp path" comment, ~line 143). After the stamp succeeds and returns the stamped path, add:

```rust
// Post-stamp: fire any matching workflow (app-side trigger, v0).
if let Ok(repo) = repo_root_of(&stamped_path) {
    if let Ok(rel) = stamped_path.strip_prefix(&repo) {
        if let Ok(hits) = secretariat_core::application::match_workflows(&prefs_path, &repo, rel) {
            for w in hits {
                // Dispatch via the existing app cognition path (dispatch.rs).
                // Non-blocking: spawn, do not await the Touch-ID ceremony on it.
                spawn_workflow_scribe(&repo, &w.prompt);
            }
        }
    }
}
```

Reuse `repo_root_of` (lift it to a shared helper or duplicate the few lines) and implement `spawn_workflow_scribe` using the same dispatch mechanism `dispatch_send` already uses. Confirm `prefs_path`/`stamped_path` are in scope in that command before writing.

- [ ] **Step 3: Manual end-to-end verification**

1. Ensure `~/Developer/themia/minerva/docs/pain/` has a real pain doc (or move a stamped idea into `docs/ideas/`).
2. `./target/debug/sec workflow run <that-doc>` (no `--dry-run`) → confirm a Linear issue appears in **Engineering** and a `linear:` key is written back.
3. In the app: stamp a `docs/pain/*.md` doc in a themia repo → confirm the same flow fires.
4. Re-stamp the same doc → confirm **no duplicate** issue (the `linear:` no-op guard in the workflow prompt).

- [ ] **Step 4: Commit**

```bash
git add crates/cli/src/commands/workflow.rs src-tauri/src/commands/secretariat.rs
git commit -m "feat(workflow): dispatch scribe on stamp (CLI run + app post-stamp)"
```

---

## Final quality gate

- [ ] Run: `cargo test --workspace`  → all pass.
- [ ] Run: `cargo clippy -- -D warnings`  → clean.
- [ ] Run: `cargo fmt`  → committed.

## Self-review notes (coverage vs spec)

- Workflow file shape (frontmatter trigger + body prompt) → Task 3 `parse_workflow`.
- Match semantics (type from path, frontmatter override, tags from registry, any-of, absent = unconstrained) → Tasks 1, 2, 4.
- `sec workflow list` / `run --dry-run` → Task 5. Real dispatch → Task 6.
- App-side post-stamp trigger → Task 6.
- Idempotency (`linear:` no-op, only stamps fire) → enforced in the workflow *prompt* (already in `minerva/.secretariat/workflows/to-linear.md`), verified in Task 6 Step 3.4.
- Deferred per spec (NOT in this plan): daemon supervision, docs-branch worktree, MCP surface, multi-action chaining, journals.
```
