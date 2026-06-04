---
tag: spec
status: design
date: 2026-06-04T00:00:00.000Z
slice_id: docs-pipeline
extends: docs/superpowers/specs/2026-06-01-repo-registry-design.md
companion: docs/ideas/2026-06-04-conductor-teardown.md
---

# Spec — Docs worktree pipeline (always-on docs tree + ambient draft-PR)

## Premise — the floater problem

Today: one working tree, ~18 local branches. The principal branch-hops a single
checkout, so uncommitted work — **docs especially** — has nowhere to live but the
current tree. A spec written while on `feat/x` becomes an untracked file that
follows every checkout, contaminates unrelated diffs, and gets "messily added"
into the wrong commit or stranded.

The fix is not a cleanup hook firing *after* the mess. It is **placement at
birth**: a doc is created in the tree it belongs to, so it never pollutes the
branch the principal happens to be standing on.

This is Conductor's keystone borrow — *"Workspace = git worktree … the default
unit of work, invisible to the user. Structural answer to don't stash/checkout to
separate work."* (`docs/ideas/2026-06-04-conductor-teardown.md`, STEAL-core) —
scoped to docs, with the **compulsion gate rejected** (no notify, no chime, no
automerge) and the **trust layer added** (signature on commit, stamp at merge).

## North star — born right, flows ambient, sealed on cadence

```
new narrative doc
   │  editor writes into the docs worktree (not the current branch)   ← auto-PLACE
   ▼
docs worktree on permanent `docs` branch
   │  commit + DID signature                                          ← auto-COMMIT (safe: isolated tree)
   ▼
push → standing DRAFT PR (docs → main)                               ← ambient, no ping, doesn't interfere
   │
   │  principal visits on their own cadence — /review-repos
   ▼
stamp → merge                                                        ← the human SEAL (pull, never push)
```

Two gates, deliberately split (the conductor-teardown crux):

- **dispatch = signature** — cheap, every commit, *informational*.
- **PR-merge = stamp** — sober, once, *authoritative*.

Auto-commit was called a footgun earlier in design. It is not a footgun **here**,
because the docs worktree is **isolated** — it contains only the doc just written,
nothing co-mingled to clobber. The isolation is precisely what de-fangs
auto-commit. The 2026-05-26 incident was about destroying *mixed* working-tree
state; that precondition does not exist in a dedicated docs tree.

## Worktree layout — collected, hidden, derivable; NOT in the key home

```
~/Developer/.../secretariat        ← code: hops feature branches
~/.worktrees/secretariat/docs      ← docs: ALWAYS mounted, permanent `docs` branch
~/.worktrees/themia/docs           ← same formula, every registered repo
```

- **One hidden collector**, machine-derivable as `<worktrees_root>/<repo>/<branch>`.
  The repos directory stays clean — no `-docs` siblings littering it.
- **Permanent `docs` branch**, not a feature branch. It is never torn down and
  never tied to a feature's lifecycle, so docs are never stranded. It flows to
  `main` via the standing draft PR on the principal's cadence; after each merge it
  keeps living.
- **Never detached HEAD** — commits would orphan and a PR cannot be opened from
  detached HEAD. Always a real, dedicated branch.

### Why not under `~/.secretariat/`

`~/.secretariat/` is the **identity + key home**. The git-native teardown
deliberately moved substrate *out* of it (envelopes migrated to repos;
`.secretariat` now holds zero envelopes). A worktree is a repo checkout — it *is*
substrate (architectural invariant #5: filesystem authoritative, the **repo** is
the substrate). Burying worktrees back in the key home re-tangles exactly what the
teardown separated, drags multi-GB `target/` dirs (for code worktrees) into the
crypto home, and lands them in a directory governed by destructive-op landmines
(mv-not-rm, snapshot gates).

**Resolution — own it, don't host it.** Secretariat owns the *management* via the
registry config (which lives in `~/.secretariat/preferences.toml`); it does not
host the *bytes*. Config in the key home, checkouts in a neutral root.

## Data model — extends the repo registry

Companion spec `2026-06-01-repo-registry-design.md` defines the `[[repos]]`
registry inside `preferences.toml` and the `RepoRole { Project, Home }` kind.
This slice adds two things.

### 1. A `worktrees_root` on `Preferences`

```rust
pub struct Preferences {
    // … composition / cognition / delivery / repos (from the registry spec) …

    /// Root under which all managed worktrees mount: `<root>/<repo>/<branch>`.
    /// Defaults to `~/.worktrees`. Deliberately NOT under SECRETARIAT_HOME.
    #[serde(default = "default_worktrees_root")]
    pub worktrees_root: PathBuf,
}
```

### 2. A `docs_branch` on `RepoEntry`

```rust
pub struct RepoEntry {
    pub path: PathBuf,                 // identity key (from registry spec)
    #[serde(default)]
    pub role: RepoRole,                // Project | Home (from registry spec)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// The permanent docs branch for this repo. `None` ⇒ docs ride the code
    /// branch as today (opt-in per repo). `Some("docs")` is the typical value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs_branch: Option<String>,
}
```

Derived, never stored: the worktree path is always
`worktrees_root.join(repo_dir_name).join(branch)`. A formula, not a field — so the
registry computes it and nothing drifts.

## Role-gating — the two tiers

| Role | docs worktree | docs branch | flows to `main` via |
| --- | --- | --- | --- |
| **`Project`** (pushable, designs-with-code) | `<root>/<repo>/docs` | `docs` (permanent) | standing **draft PR**, principal's cadence |
| **`Home`** (private, may-never-push) | `<root>/<repo>/docs` | `docs` (permanent) | **no PR** (no remote) — reviewed locally via `/review-repos` |

`RepoRole` keeps paying rent: a draft PR only makes sense for a pushable repo with
a remote, so the PR automation is `Project`-only by construction.

## The carve-out — code-coupled docs stay on the feature branch

Architectural hard rule #3: a `lexicons/` edit **lands in the same commit** as the
Rust change. Any doc that *must* co-commit with code — schema, contract, a design
note that ships inside a feature PR — stays on the feature branch and does **not**
route to the `docs` branch. Only **non-coupled narrative docs** (ideas, specs,
plans, decisions) flow to the docs tree. The role split keeps the two trees from
colliding; collisions on the same path across both branches are the signal that a
doc was misrouted.

## Calibration — worktree per *concern*, not per *branch*

The trap is a worktree per feature branch. Two costs specific to this repo:

- **`target/` duplication** — each *code* worktree carries its own multi-GB build.
- **Sidecar tax** — a fresh code worktree needs `build-sidecars.sh` run once or
  `cargo check -p secretariat` fails (`binaries/sec-<triple>` missing).

So worktrees pay only when two tracks are **genuinely live at once**, or a surface
must be **always-on**. The **docs worktree is the unambiguous win**: no build, no
sidecar tax, and "always-on" is its defining property. Everything else is
discipline, not tooling — most branches are serial; just switch.

## Surfaces

Per AGENTS.md, a principal-facing primitive ships on use case + CLI + MCP + tests.

### Use case — `crates/core/src/application/docs_ops.rs` (new)

Pure orchestration; git IO via a port.

```rust
/// Idempotent: create the docs worktree for `repo` if absent, return its path.
/// Validates the repo is registered + git-native; creates the `docs` branch off
/// the repo's default branch on first call.
pub fn ensure_docs_worktree(
    prefs_path: &Path, repo_path: &Path,
) -> Result<PathBuf, DocsOpsError>;

/// Resolve where a NEW narrative doc should be written for `repo` — the docs
/// worktree path joined with the doc's relative path. The editor calls this on
/// "new doc" so placement happens at birth.
pub fn resolve_doc_target(
    prefs_path: &Path, repo_path: &Path, rel: &Path,
) -> Result<PathBuf, DocsOpsError>;
```

### CLI — `crates/cli/src/commands/docs.rs`

```
sec docs ensure <repo>     # create/return the docs worktree (idempotent)
sec docs path <repo>       # print the docs worktree path (scripting)
```

### MCP — `crates/mcp/src/server.rs`

`docs_ensure` — lets the scribe stand up a repo's docs worktree before writing
into it. Same shape as the CLI.

### Editor wiring

The Tauri editor's "new doc" action calls `resolve_doc_target` and writes there
instead of the live checkout's cwd. This is **auto-placement only** — the principal
still saves; commit/sign happens on the docs branch; the stamp stays a deliberate
act at merge.

## Out of scope (this slice)

- **Draft-PR automation** (`gh pr create --draft` on first push of the docs
  branch; keep-open accumulation). Its own sub-slice — needs a GitHub port and the
  push trigger. This slice lands the worktree + placement; the PR is the next step.
- **Per-concern code-worktree automation** — manual `git worktree` for now;
  registry-driven later.
- **Tauri settings UI** for worktree management — CLI + MCP first.
- **The stamp-on-merge gate mechanics** — defined by the existing stamp ceremony;
  this slice does not change it.

## Testing

- prefs round-trip: `worktrees_root` + `docs_branch` save → load → equal; omitted
  fields default (`~/.worktrees`, `docs_branch = None`).
- `ensure_docs_worktree` idempotent: second call returns the same path, no error,
  no duplicate worktree.
- `resolve_doc_target` joins `<root>/<repo>/docs` + rel correctly; canonicalized.
- role-gating: a `Home` repo resolves a worktree but is flagged no-PR; a `Project`
  repo is PR-eligible. (PR automation itself is the next slice; assert the flag.)
- reject an unregistered or non-git repo with `DocsOpsError::NotRegistered` /
  `NotAGitRepo`.

**Quality gate:** `cargo test --workspace` + `cargo clippy -- -D warnings`.

## Conductor lineage (provenance)

Steal: worktree-per-task, repo registry, launch/open-in picker. Reject: desktop
notifications, completion sounds, automerge — the compulsion layer Secretariat
exists to *not* have. The one-line summary from the teardown holds:

> Conductor is the delegation gauntlet with the seal removed and the compulsion
> layer added. Steal the mechanics; reject the gate.

This slice is the on-grain half: the mechanics (worktree-per-task) with the seal
intact (signature on commit, stamp at merge) and the compulsion absent (ambient
draft PR, pulled never pushed).
