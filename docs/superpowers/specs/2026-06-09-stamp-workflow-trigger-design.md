---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:6255fb522c3289c2c6f57affe0ed2372c597ff55da3c2e2b400b729c8c8bf06b
  docFilename: 2026-06-09-stamp-workflow-trigger-design.md
  stampedAt: 2026-06-09T13:10:40.643515Z
  signature: ed25519:ST83r3jSqeaQHEpU7v/jW6NGD072z5Z+aNFAA6yLC8ORqZozhZEYjEhBxU1udfhbeErM+GiXe4BUWtYqP8H/Ag==
---

# Spec — Stamp-triggered workflows (v0, app-side)

## Premise — the seal is the trigger

The stamp is already the principal's consent gate — "the subset they elect to
elevate" (hard rule #4). This slice turns that seal into a **firing pin**: when
a document is stamped, Secretariat runs a principal-authored workflow against it
(e.g. publish the idea to Linear). Automation that fires **only on
cryptographically-attested human consent** — "the seal is the webhook."

The selectivity is the stamp itself: nothing reaches Linear that the principal
did not seal. Fully on-grain — the principal seals, the scribe acts, never the
reverse.

## North star

```
principal stamps doc (UI)
   │  app embeds $attestation → post-stamp step runs                 ← app-side, v0
   ▼
match `.secretariat/workflows/*.md` in the doc's repo
   │  on: stamp · match: {type (from path), tags (from registry)}
   ▼
dispatch a scribe with the workflow's prompt (cwd = repo, doc path passed)
   │  scribe acts (creates Linear issue), writes `linear:` back-link
   ▼
commit the stamped doc + the write-back                              ← stamp ⟹ commit
```

## Scope — why app-side

Two facts from the current codebase set v0's shape:

* **The daemon is keepalive-only.** `crates/daemon/src/serve.rs` is a bare
  LaunchAgent loop — no watcher, no IPC, no supervisor. Routing the trigger
  through it means building its first real subsystem.

* **Scribe-dispatch lives app-side.** Cognition is wired in the Tauri app
  (`src-tauri/src/commands/dispatch.rs` + the `cognition-claude-sdk` sidecar),
  not in `core`. The app already holds everything needed to run a scribe.

So v0 fires the trigger **in the app**, reusing the dispatch mechanism that
already exists. Robust daemon supervision (survives app-close, retries, async
queue) is **v1**, named in Out of scope.

## Workflow file — `.secretariat/workflows/*.md`

In-repo, one file per flow. Trigger in the file's **frontmatter**; the scribe
**prompt is the body**. (The *document's own* frontmatter stays clean — routing
is config, not per-doc.) Analogous to `.github/workflows`: "Actions for stamps."

```markdown
---
on: stamp
match:
  type: [idea, pain]      # any-of; matched against the doc's type
  tags: [themia]          # any-of; matched against the repo's registry tags
---
Read the stamped document. Create a Linear issue in the Engineering team … then
write the issue URL back as a `linear:` frontmatter key (skip if already present).
```

**Match semantics (pure, given inputs):**

* **`type`** is derived from the doc's path — the immediate subdir under `docs/`
  (`docs/pain/x.md` → `pain`). A `type:` key in the doc's frontmatter overrides.
  *Known v0 limit:* a flat `docs/x.md` with no subdir and no frontmatter type is
  **untyped** and won't match a type-filtered workflow. Acceptable for v0.

* **`tags`** come from the repo's registry entry (`RepoEntry.tags`), not the doc.

* A workflow matches when `on` fires **and** every present filter has a non-empty
  intersection. Absent filter = unconstrained.

## Surfaces (parallel-surfaces rule, minus MCP)

### Domain — value objects

`Workflow { trigger: Trigger, prompt: String }`, `Trigger { on: StampEvent,
match: WorkflowMatch { types: Vec<DocType>, tags: Vec<String> } }`. Parsed +
validated at construction; unknown `on:`/malformed frontmatter → a typed error,
never a panic. No IO in domain.

### Application — `crates/core/src/application/workflow_ops.rs` (new)

Pure orchestration; IO via existing `Preferences`/registry + a filesystem read.

```rust
/// Load + parse every `.secretariat/workflows/*.md` in `repo`.
pub fn load_workflows(repo: &Path) -> Result<Vec<Workflow>, WorkflowError>;

/// Workflows whose trigger fires for a just-stamped doc. Type from path,
/// tags from the registry entry for `repo`.
pub fn match_workflows(
    prefs_path: &Path, repo: &Path, doc_rel: &Path,
) -> Result<Vec<Workflow>, WorkflowError>;
```

The scribe **run** itself is dispatched by the caller (app) through the existing
cognition path — `workflow_ops` decides *what* matches; it does not own the
agent runtime.

### App trigger — Tauri stamp command

After the stamp command embeds the `$attestation` and the stamp succeeds, a
post-stamp step calls `match_workflows`, then for each match dispatches a scribe
via `dispatch.rs`: cwd = the repo, the workflow prompt as the task, the stamped
doc's path passed in. Non-blocking w\.r.t. the Touch-ID ceremony (fires after the
seal lands).

### CLI — `crates/cli/src/commands/workflow.rs` (registered in `main.rs`)

```
sec workflow list [<repo>]            # parsed workflows + their triggers
sec workflow run <doc> [--dry-run]    # fire matching workflows WITHOUT a stamp
```

`run` is the test/escape hatch: it lets a workflow be exercised (or previewed
with `--dry-run`) independently of the UI stamp. `--dry-run` resolves matches +
renders the prompt that *would* dispatch, runs nothing external.

### Persistence — stamp ⟹ commit

A stamp is only coherent against committed state (`SEALED` = unchanged since its
commit). So the post-stamp step ensures the stamped doc **and** the scribe's
write-back are committed (current branch, v0). No worktree required. The clean
`docs`-branch worktree (`~/.worktrees/<repo>/docs`) is the v1 upgrade for
separating doc commits from feature diffs — deferred (see Out of scope).

## Idempotency / loop prevention

* **Only stamps fire workflows** — the scribe's `linear:` write-back is an edit,
  not a stamp, so it cannot re-trigger.

* The prompt is instructed to **no-op if** **`linear:`** **is already present** — a
  second stamp won't duplicate the issue.

## Trust / safety

* The **stamp is the authorization boundary**: dispatching a scribe on a sealed
  doc is consent-gated by definition. The workflow file is principal-authored
  config, committed and reviewable.

* External side effects (a Linear issue) are real and not easily reversible —
  `--dry-run` exists for preview, and v0 ships with exactly one principal-written
  workflow, not a marketplace.

* Fail-closed on parse errors: a malformed workflow is skipped + logged, never
  guessed.

## Testing

* **Workflow parse**: valid frontmatter → `Workflow`; unknown `on:` / malformed
  YAML → typed error; body becomes the prompt verbatim.

* **Match (pure)**: type-from-path derivation (`docs/pain/x.md` → `pain`);
  frontmatter `type:` override; untyped flat doc misses a type filter; tags read
  from the registry entry; absent filter = unconstrained; any-of intersection.

* **`sec workflow run --dry-run`**: resolves matches + renders the prompt, no
  external call (assert nothing dispatched).

* **App trigger wiring**: post-stamp step calls `match_workflows` and dispatches
  once per match (mock the cognition dispatch; assert call shape + cwd + doc path).

* **Quality gate:** `cargo test --workspace` + `cargo clippy -- -D warnings`.

## Out of scope (this slice)

* **Daemon supervision (v1)** — stamp-event queue + headless scribe under the
  daemon, survives app-close, retries. The robust path; not v0.

* **Docs-branch worktree / placement-at-birth** — `~/.worktrees/<repo>/docs`,
  the `2026-06-04-docs-worktree-pipeline` spec. v0 commits on the current branch.

* **MCP surface** — the scribe is *dispatched*, not a tool it calls; no
  `mcp__secretariat__*` addition. Add only if a scribe needs faceting in-session.

* **Multi-action chaining / scheduling / non-`stamp`** **triggers** — one `on:
  stamp` verb only.

* **Personal journals / private substrate** — a `home`-role store indexed by
  penceive/Saperene, never a shared repo. Different substrate; explore later.

<br />

