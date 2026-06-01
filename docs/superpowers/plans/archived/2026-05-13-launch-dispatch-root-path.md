# launch + dispatch + root_path — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bind a Secretariat channel to a host-filesystem directory (typically a git repo) via a `root_path` field in the channel's `contract.local.md`, then ship `bind` / `launch` / `dispatch` verbs (CLI + MCP) so a parent Claude session can fan headless subagents into the right channel context.

**Architecture:** New `ChannelBinding` value object (sibling to `ChannelContract`, not part of its merge algebra — leaf-only override). Both VOs co-tenant the same `contract.local.md` frontmatter; only the parser changes. A `BindingLookup` infrastructure helper queries the leaf-level binding for any `Recipient`; `queue_dir` callers consult it before falling back to default substrate paths. A new `CognitionLauncher` port has one implementation today (`claude -p` subprocess). `dispatch --background` is explicitly out of this plan — defers to the daemon's `AgentSupervisor` design.

**Tech Stack:** Rust workspace (`crates/core`, `crates/cli`, `crates/mcp`, `crates/daemon`). Tauri v2 for the GUI shell (unaffected by this slice). `serde_yaml` for frontmatter, `std::process::Command` for subprocess, `tempfile` + `insta` already in test deps.

**Pitch reference:** `docs/pitches/2026-05-13-launch-dispatch-root-path.md`

---

## File map

### Domain (`crates/core/src/domain/`)

- **Create:** `channel_binding.rs` — `ChannelBinding` VO.
- **Modify:** `mod.rs` — re-export `ChannelBinding`.
- **Unchanged:** `channel_contract.rs` — merge algebra stays as-is.

### Application (`crates/core/src/application/`)

- **Modify:** `contract_ops.rs` — extend frontmatter parse/write to round-trip `root_path` alongside `ChannelContract`. Add `resolve_channel_binding(channel_uri) -> ChannelBinding` (leaf-only lookup, no merge).
- **Create:** `bind_channel.rs` — `bind_channel` use case: validate channel exists, write/update `<root_path>/contract.local.md`, ensure default-location symlink, write `.gitignore` block when `.git/` present.
- **Create:** `launch_channel.rs` — `launch_channel` use case: resolve binding → cwd, return `LaunchInvocation { cwd, command }`.
- **Create:** `dispatch_channel.rs` — `dispatch_channel` use case: resolve binding → cwd, delegate to `CognitionLauncher::dispatch`.

### Ports (`crates/core/src/ports/`)

- **Create:** `cognition_launcher.rs` — `CognitionLauncher` trait: `launch(&self, cwd) -> Result<LaunchPlan>` (interactive plan, not exec) + `dispatch(&self, cwd, task) -> Result<String>` (headless, returns stdout).

### Infrastructure (`crates/core/src/infrastructure/`)

- **Create:** `binding_store.rs` — read `root_path` from a channel-dir's `contract.local.md`; `BindingLookup` struct cached per session.
- **Create:** `gitignore_writer.rs` — idempotent fenced-block writer.
- **Create:** `cognition/headless_claude.rs` — `HeadlessClaudeLauncher` adapter wrapping `claude -p`.
- **Modify:** `queue_dir.rs` — add a thin `resolve_queue_dir(aliases, recipient, root, bindings)` wrapper that consults `BindingLookup` before the existing default. Existing `queue_dir` stays as the no-binding pure function.
- **Modify:** `preferences.rs` — add `CognitionPrefs::launch_command: String` (default `"claude"`).
- **Modify:** `mod.rs` — re-exports.

### CLI (`crates/cli/src/`)

- **Create:** `commands/bind.rs`, `commands/launch.rs`, `commands/dispatch.rs`.
- **Modify:** `main.rs` — register three subcommands.

### MCP (`crates/mcp/src/`)

- **Modify:** `server.rs` — three new `#[tool]` entries: `bind_channel`, `launch_channel`, `dispatch`.

### Tests

- Unit tests live with each module via `#[cfg(test)]`.
- Integration tests: `crates/cli/tests/bind_launch_dispatch.rs`, `crates/mcp/tests/bind_launch_dispatch.rs`.

### Docs

- **Modify:** `AGENTS.md` — extend "What's here today" with the three new verbs.
- **Create:** `docs/developer/launch-dispatch.md` — operator notes (binding flow, gitignore block shape, dispatch auth caveats).

---

## Task 0: Spike — verify `claude -p` headless contract

**Files:** none (research only — record findings inline in this plan).

- [ ] **Step 1: Confirm CLI invocation shape**

Run from any cwd:

```bash
claude -p "say hi in 5 words" --output-format text
```

Expected: process exits 0, prints a short string to stdout, no TUI.

Record actual stdout/stderr behavior. If `--output-format json` is available, record its shape (presence of `result`, `cost_usd`, `session_id` fields). This determines whether `dispatch_channel` parses JSON or returns raw stdout.

- [ ] **Step 2: Confirm cwd inheritance**

```bash
cd /tmp && mkdir -p sp-spike/.claude && echo "Project: sp-spike test" > sp-spike/.claude/CLAUDE.md
cd sp-spike && claude -p "what's in your project memory?" --output-format text
```

Expected: response mentions "sp-spike test", confirming Claude Code walks up from cwd to load `.claude/`.

- [ ] **Step 3: Confirm auth inheritance from non-TTY parent**

```bash
nohup bash -c 'cd /tmp/sp-spike && claude -p "hi" --output-format text > /tmp/sp-spike.out 2>&1' &
wait
cat /tmp/sp-spike.out
```

Expected: same hi response, no `Please run claude login` prompt. If this fails, the daemon-side `dispatch` arm needs an auth-bootstrap UX — flag in plan as risk for `--background` work; the foreground `dispatch` (initiated by user-attended Claude Code) is unaffected.

- [ ] **Step 4: Edit this plan with findings**

Append a "Task 0 findings" block to this file under this task, recording: (a) chosen `--output-format`, (b) auth result, (c) any unexpected behavior. No commit — research notes only.

---

## Task 1: Domain — `ChannelBinding` VO

**Files:**

- Create: `crates/core/src/domain/channel_binding.rs`
- Modify: `crates/core/src/domain/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/domain/channel_binding.rs`:

```rust
//! Per-channel binding: a leaf-only mapping from a channel to a host
//! filesystem directory.
//!
//! Lives in `contract.local.md` frontmatter alongside [`ChannelContract`]
//! but does NOT participate in the accumulate merge — bindings are
//! per-device, per-principal, and never inherited from ancestors.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChannelBinding {
    /// Absolute host path the channel-dir should resolve to. None =
    /// fall through to the default `<root>/<alias>/<handle-segments>/`.
    pub root_path: Option<PathBuf>,
}

impl ChannelBinding {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.root_path.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn empty_binding_is_empty() {
        assert!(ChannelBinding::empty().is_empty());
    }

    #[test]
    fn any_root_path_makes_not_empty() {
        let b = ChannelBinding {
            root_path: Some(PathBuf::from("/Users/rafa/Developer/secretariat")),
        };
        assert!(!b.is_empty());
    }
}
```

Add to `crates/core/src/domain/mod.rs`:

```rust
pub mod channel_binding;
pub use channel_binding::ChannelBinding;
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p secretariat-core --lib domain::channel_binding
```

Expected: 2 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/domain/channel_binding.rs crates/core/src/domain/mod.rs
git commit -m "feat(core): ChannelBinding value object — leaf-only root_path"
```

---

## Task 2: Application — parse `root_path` from `contract.local.md`

**Files:**

- Modify: `crates/core/src/application/contract_ops.rs`

**Context:** The contract file parser today reads `cadence_floor_minutes` and `min_trust`. We add `root_path`. The parser becomes a `ContractFrontmatter { contract: ChannelContract, binding: ChannelBinding }` struct so callers can keep the two value objects separated cleanly.

- [ ] **Step 1: Write the failing test**

In `crates/core/src/application/contract_ops.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn parser_extracts_root_path_into_binding() {
    let frontmatter = r#"---
$type: tech.equanimi.secretariat.consumptionContract
cadence_floor_minutes: 15
min_trust: signed-only
root_path: /Users/rafa/Developer/secretariat
---
body prose
"#;
    let parsed = parse_contract_frontmatter(frontmatter).unwrap();
    assert_eq!(parsed.contract.cadence_floor_minutes, Some(15));
    assert_eq!(
        parsed.binding.root_path,
        Some(std::path::PathBuf::from("/Users/rafa/Developer/secretariat"))
    );
}

#[test]
fn parser_handles_missing_root_path() {
    let frontmatter = r#"---
cadence_floor_minutes: 15
---
"#;
    let parsed = parse_contract_frontmatter(frontmatter).unwrap();
    assert!(parsed.binding.is_empty());
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cargo test -p secretariat-core parser_extracts_root_path_into_binding
```

Expected: FAIL (`parse_contract_frontmatter` not found or missing field).

- [ ] **Step 3: Implement `ContractFrontmatter` + parser**

Add to `contract_ops.rs`:

```rust
use crate::domain::ChannelBinding;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractFrontmatter {
    pub contract: ChannelContract,
    pub binding: ChannelBinding,
}

pub(crate) fn parse_contract_frontmatter(
    raw: &str,
) -> Result<ContractFrontmatter, ContractOpsError> {
    // Reuse the existing frontmatter splitter from contract_store; extend
    // its YAML deserializer with an optional `root_path: PathBuf` field.
    // Body prose is discarded here — callers that need it use
    // `load_contract_with_body`.
    // [Engineer: locate the current YAML struct in contract_store.rs and
    // add `root_path: Option<PathBuf>`. Map it into ChannelBinding.]
    todo!()
}
```

Update the existing `load_contract` / `save_contract` calls in `contract_store.rs` to round-trip `root_path` (read + write). The save path MUST preserve `root_path` across `ContractPatch` applications (only `ChannelContract` fields are patchable in v1; binding is set by the dedicated `bind_channel` use case).

- [ ] **Step 4: Run tests — verify they pass**

```bash
cargo test -p secretariat-core contract_ops::
```

Expected: all green, including existing accumulate-merge tests (no regressions).

- [ ] **Step 5: Add `resolve_channel_binding` use case**

Append to `contract_ops.rs`:

```rust
/// Leaf-only lookup of the binding for one channel. No merge — bindings
/// do not inherit from ancestors.
pub fn resolve_channel_binding(
    paths: &KeyPaths,
    aliases: &AliasMap,
    channel_uri: &str,
) -> Result<ChannelBinding, ContractOpsError> {
    // Parse channel_uri into (owner_did, handle). Compute the channel-dir
    // via the existing default queue_dir (NOT the binding-aware one — we
    // need the on-disk file that stores the binding itself). Load
    // contract.local.md from that path, parse frontmatter, return
    // binding. Missing file or empty frontmatter -> ChannelBinding::empty().
}
```

Test:

```rust
#[test]
fn resolve_binding_returns_empty_when_no_contract_file() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = KeyPaths::test_at(tmp.path());
    let aliases = AliasMap::test_with(vec![("themia", themia_did())]);
    let binding = resolve_channel_binding(&paths, &aliases, "did:web:themia.pro#channel:dev:secretariat").unwrap();
    assert!(binding.is_empty());
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/application/contract_ops.rs crates/core/src/infrastructure/contract_store.rs
git commit -m "feat(core): contract.local.md frontmatter round-trips root_path"
```

---

## Task 3: Infrastructure — `gitignore_writer`

**Files:**

- Create: `crates/core/src/infrastructure/gitignore_writer.rs`
- Modify: `crates/core/src/infrastructure/mod.rs` — re-export.

**Why fenced:** idempotent rewrites without clobbering user-managed lines.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/infrastructure/gitignore_writer.rs`:

```rust
//! Idempotent fenced-block writer for `.gitignore` under bound channel
//! directories. The block lists Secretariat receiver-private artifacts:
//! `contract.local.md`, `envelopes/`, `outbox/`, `_ciphertext/`.

use std::fs;
use std::path::Path;

use thiserror::Error;

const FENCE_START: &str = "# === secretariat ===";
const FENCE_END: &str = "# === /secretariat ===";

const ENTRIES: &[&str] = &[
    "contract.local.md",
    "envelopes/",
    "outbox/",
    "_ciphertext/",
];

#[derive(Debug, Error)]
pub enum GitignoreWriterError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Append or replace the Secretariat fenced block in
/// `<repo_root>/.gitignore`. Creates the file if absent. Idempotent.
pub fn apply_secretariat_block(repo_root: &Path) -> Result<(), GitignoreWriterError> {
    let path = repo_root.join(".gitignore");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let updated = upsert_block(&existing);
    fs::write(&path, updated)?;
    Ok(())
}

fn upsert_block(existing: &str) -> String {
    let block = build_block();
    if let Some((before, rest)) = existing.split_once(FENCE_START) {
        if let Some((_, after)) = rest.split_once(FENCE_END) {
            // Replace fenced region; trim trailing newline of `after`
            // to avoid double blank lines on repeated applies.
            let after = after.strip_prefix('\n').unwrap_or(after);
            return format!("{}{}{}", before, block, after);
        }
    }
    // No fence yet — append.
    let sep = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    format!("{existing}{sep}{block}")
}

fn build_block() -> String {
    let mut s = String::from(FENCE_START);
    s.push('\n');
    for entry in ENTRIES {
        s.push_str(entry);
        s.push('\n');
    }
    s.push_str(FENCE_END);
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_block_when_no_gitignore_exists() {
        let tmp = tempdir().unwrap();
        apply_secretariat_block(tmp.path()).unwrap();
        let out = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(out.contains(FENCE_START));
        assert!(out.contains("outbox/"));
    }

    #[test]
    fn appends_block_to_existing_gitignore_preserving_user_lines() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "node_modules/\ntarget/\n").unwrap();
        apply_secretariat_block(tmp.path()).unwrap();
        let out = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(out.starts_with("node_modules/\ntarget/\n"));
        assert!(out.contains(FENCE_START));
    }

    #[test]
    fn is_idempotent_when_block_already_present() {
        let tmp = tempdir().unwrap();
        apply_secretariat_block(tmp.path()).unwrap();
        apply_secretariat_block(tmp.path()).unwrap();
        let out = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        let occurrences = out.matches(FENCE_START).count();
        assert_eq!(occurrences, 1);
    }

    #[test]
    fn replaces_stale_block_contents() {
        let tmp = tempdir().unwrap();
        let stale = format!("foo/\n{FENCE_START}\nold-entry\n{FENCE_END}\nbar/\n");
        fs::write(tmp.path().join(".gitignore"), &stale).unwrap();
        apply_secretariat_block(tmp.path()).unwrap();
        let out = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(out.contains("outbox/"));
        assert!(!out.contains("old-entry"));
        assert!(out.starts_with("foo/\n"));
        assert!(out.contains("bar/\n"));
    }
}
```

Add to `crates/core/src/infrastructure/mod.rs`:

```rust
pub mod gitignore_writer;
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p secretariat-core --lib infrastructure::gitignore_writer
```

Expected: 4 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/infrastructure/gitignore_writer.rs crates/core/src/infrastructure/mod.rs
git commit -m "feat(core): idempotent fenced .gitignore writer"
```

---

## Task 4: Infrastructure — `BindingLookup` + binding-aware queue_dir

**Files:**

- Create: `crates/core/src/infrastructure/binding_store.rs`
- Modify: `crates/core/src/infrastructure/queue_dir.rs`
- Modify: `crates/core/src/infrastructure/mod.rs`

**Why a lookup not a direct call:** `queue_dir` is a pure no-IO function called from many places. We keep it pure and introduce a `resolve_queue_dir` wrapper that takes a `&BindingLookup` (which has done the IO once, upfront). Cheap per-call.

- [ ] **Step 1: Write the failing tests**

Create `crates/core/src/infrastructure/binding_store.rs`:

```rust
//! Receiver-side lookup of `ChannelBinding` overrides for queue
//! resolution. Construct once per operation from the on-disk
//! contract.local.md files; query as a pure function thereafter.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::domain::{ChannelBinding, Recipient};
use crate::infrastructure::queue_dir::queue_dir;
use crate::infrastructure::AliasMap;

#[derive(Debug, Default, Clone)]
pub struct BindingLookup {
    by_default_path: HashMap<PathBuf, ChannelBinding>,
}

impl BindingLookup {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Walk the substrate root, load every channel's contract.local.md,
    /// keep only those with a non-empty binding. Called by daemon
    /// startup, MCP tool calls, CLI command entries.
    pub fn load_from(
        _substrate_root: &Path,
        _aliases: &AliasMap,
    ) -> std::io::Result<Self> {
        // Walk <root>/*/<handle-segments>/contract.local.md; parse;
        // index by computed default queue_dir path.
        // [Engineer: implement using walkdir; cap depth to handle nesting.]
        todo!()
    }

    pub fn binding_for(&self, default_path: &Path) -> Option<&ChannelBinding> {
        self.by_default_path.get(default_path)
    }
}

/// Binding-aware queue resolver. Consults `bindings` first; falls back
/// to the default substrate layout. Returns the absolute on-disk path
/// of the channel-dir.
pub fn resolve_queue_dir(
    aliases: &AliasMap,
    recipient: &Recipient,
    substrate_root: &Path,
    bindings: &BindingLookup,
) -> PathBuf {
    let default = queue_dir(aliases, recipient, substrate_root);
    match bindings.binding_for(&default).and_then(|b| b.root_path.as_ref()) {
        Some(bound) => bound.clone(),
        None => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // [Engineer: tests use tempdir + a hand-built BindingLookup. Verify:
    //  (a) empty lookup → resolve_queue_dir == queue_dir
    //  (b) lookup with binding → returns bound path
    //  (c) load_from walks tempdir and finds one binding
    // ]
}
```

Add to `crates/core/src/infrastructure/mod.rs`:

```rust
pub mod binding_store;
pub use binding_store::{resolve_queue_dir, BindingLookup};
```

- [ ] **Step 2: Implement `load_from` test fixture**

Write a helper in tests that lays out a tempdir with a channel-dir containing a `contract.local.md` with `root_path: /tmp/some-repo`. Assert `BindingLookup::load_from(...)` returns one binding indexed by the channel-dir path.

- [ ] **Step 3: Run tests**

```bash
cargo test -p secretariat-core --lib infrastructure::binding_store
```

Expected: 3 passed.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/infrastructure/binding_store.rs crates/core/src/infrastructure/mod.rs
git commit -m "feat(core): BindingLookup + resolve_queue_dir wrapper"
```

---

## Task 5: Application — `bind_channel` use case

**Files:**

- Create: `crates/core/src/application/bind_channel.rs`
- Modify: `crates/core/src/application/mod.rs`

**Behavior:**

1. Parse channel URI → `(owner_did, handle)`. Verify channel exists (channel def present).
2. Compute the _default_ channel-dir (via `queue_dir`).
3. `create_dir_all(bound_path)`.
4. If `<default>/contract.local.md` exists with body, MOVE the body across to `<bound>/contract.local.md` along with the existing frontmatter; otherwise scaffold a fresh `contract.local.md` carrying only `root_path`.
5. Write the binding's `root_path` field into `<bound>/contract.local.md` frontmatter.
6. Symlink default-path → bound-path (so existing daemon watchers + readers traverse transparently). On macOS, `std::os::unix::fs::symlink`.
7. If `<bound>/.git/` exists → call `gitignore_writer::apply_secretariat_block(bound_path)`.

Idempotent: re-binding the same channel to the same path is a no-op; re-binding to a different path moves contents + redirects symlink.

- [ ] **Step 1: Write the failing tests**

`crates/core/src/application/bind_channel.rs`:

```rust
//! Bind a channel's on-disk home to a host-filesystem directory
//! (typically a git repo). See pitch
//! `docs/pitches/2026-05-13-launch-dispatch-root-path.md`.

// [Engineer: full implementation per the seven-step behavior list
// above. Public entry:
//
//     pub fn bind_channel(
//         paths: &KeyPaths,
//         aliases: &AliasMap,
//         channel_uri: &str,
//         bound_path: &Path,
//     ) -> Result<BindOutcome, BindChannelError>
//
// `BindOutcome { bound_path: PathBuf, gitignore_written: bool,
// existing_binding_replaced: Option<PathBuf> }` for observable
// CLI/MCP output.
// ]
```

Required tests:

```rust
#[test]
fn binds_fresh_channel_creates_symlink_and_writes_frontmatter() { /* ... */ }

#[test]
fn binds_existing_channel_moves_body_preserving_prose() { /* ... */ }

#[test]
fn writes_gitignore_when_bound_path_is_git_repo() { /* ... */ }

#[test]
fn skips_gitignore_when_bound_path_has_no_git_dir() { /* ... */ }

#[test]
fn rebind_to_same_path_is_idempotent() { /* ... */ }

#[test]
fn rebind_to_different_path_redirects_symlink_and_moves_contents() { /* ... */ }

#[test]
fn errors_when_channel_does_not_exist() { /* ... */ }
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cargo test -p secretariat-core bind_channel
```

Expected: FAIL.

- [ ] **Step 3: Implement**

Implement per the 7-step behavior list. Reuse `gitignore_writer::apply_secretariat_block` and `contract_store` helpers. Use `std::os::unix::fs::symlink` (Mac-only per AGENTS.md).

- [ ] **Step 4: Run tests**

```bash
cargo test -p secretariat-core bind_channel
```

Expected: all 7 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/application/bind_channel.rs crates/core/src/application/mod.rs
git commit -m "feat(core): bind_channel use case — symlink, gitignore, contract move"
```

---

## Task 6: Port — `CognitionLauncher`

**Files:**

- Create: `crates/core/src/ports/cognition_launcher.rs`
- Modify: `crates/core/src/ports/mod.rs`

- [ ] **Step 1: Define the trait**

```rust
//! Port for substrate-agnostic cognition invocation. One impl ships
//! today (`HeadlessClaudeLauncher` wrapping `claude -p`); others
//! (Ollama CLI, BYOK API runner) plug in without changing the
//! application layer.

use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("spawn failed: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("subprocess exited non-zero: {code:?} — stderr: {stderr}")]
    NonZero { code: Option<i32>, stderr: String },
    #[error("invalid utf-8 in subprocess output")]
    InvalidUtf8,
}

/// Plan for an interactive launch — the host executes this, since
/// `exec` semantics differ between CLI (process replacement) and MCP
/// (return path for the caller to spawn a terminal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

pub trait CognitionLauncher: Send + Sync {
    /// Produce a plan describing how to launch an interactive session
    /// rooted at `cwd`. Caller decides whether to exec, spawn-detach,
    /// or return to a higher-level orchestrator.
    fn plan_launch(&self, cwd: &Path) -> Result<LaunchPlan, LauncherError>;

    /// Run a one-shot headless task in `cwd`. Returns the final
    /// response as a string. Blocks until the subprocess exits.
    fn dispatch(&self, cwd: &Path, task: &str) -> Result<String, LauncherError>;
}
```

- [ ] **Step 2: Re-export**

`ports/mod.rs`:

```rust
pub mod cognition_launcher;
pub use cognition_launcher::{CognitionLauncher, LaunchPlan, LauncherError};
```

- [ ] **Step 3: Compile check**

```bash
cargo check -p secretariat-core
```

Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/core/src/ports/cognition_launcher.rs crates/core/src/ports/mod.rs
git commit -m "feat(core): CognitionLauncher port — plan_launch + dispatch"
```

---

## Task 7: Infrastructure — `HeadlessClaudeLauncher` adapter

**Files:**

- Create: `crates/core/src/infrastructure/cognition/headless_claude.rs`
- Modify: `crates/core/src/infrastructure/cognition/mod.rs`

- [ ] **Step 1: Implement the adapter**

```rust
//! `CognitionLauncher` impl wrapping the user-installed `claude` CLI
//! in headless mode (`claude -p <task>`). Inherits the principal's
//! Claude Code login from `~/.claude/`.

use std::path::Path;
use std::process::Command;

use crate::ports::cognition_launcher::{CognitionLauncher, LaunchPlan, LauncherError};

#[derive(Debug, Clone)]
pub struct HeadlessClaudeLauncher {
    /// Binary name resolved against `$PATH`, or absolute path. Sourced
    /// from `Preferences::cognition::launch_command` (default `"claude"`).
    pub command: String,
}

impl HeadlessClaudeLauncher {
    pub fn new(command: impl Into<String>) -> Self {
        Self { command: command.into() }
    }
}

impl CognitionLauncher for HeadlessClaudeLauncher {
    fn plan_launch(&self, cwd: &Path) -> Result<LaunchPlan, LauncherError> {
        Ok(LaunchPlan {
            command: self.command.clone(),
            args: vec![],
            cwd: cwd.to_path_buf(),
        })
    }

    fn dispatch(&self, cwd: &Path, task: &str) -> Result<String, LauncherError> {
        let output = Command::new(&self.command)
            .arg("-p")
            .arg(task)
            .current_dir(cwd)
            .output()?;
        if !output.status.success() {
            return Err(LauncherError::NonZero {
                code: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }
        String::from_utf8(output.stdout).map_err(|_| LauncherError::InvalidUtf8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn plan_launch_returns_command_and_cwd() {
        let l = HeadlessClaudeLauncher::new("claude");
        let tmp = tempdir().unwrap();
        let plan = l.plan_launch(tmp.path()).unwrap();
        assert_eq!(plan.command, "claude");
        assert!(plan.args.is_empty());
        assert_eq!(plan.cwd, tmp.path());
    }

    #[test]
    fn dispatch_invokes_command_with_p_flag() {
        // Use `/bin/echo` as a stand-in for `claude` to validate the
        // invocation shape without depending on the real CLI in CI.
        let l = HeadlessClaudeLauncher::new("/bin/echo");
        let tmp = tempdir().unwrap();
        let out = l.dispatch(tmp.path(), "hello").unwrap();
        // `/bin/echo -p hello` prints `-p hello` on macOS.
        assert!(out.contains("hello"));
    }
}
```

Add to `crates/core/src/infrastructure/cognition/mod.rs`:

```rust
pub mod headless_claude;
pub use headless_claude::HeadlessClaudeLauncher;
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p secretariat-core --lib cognition::headless_claude
```

Expected: 2 passed.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/infrastructure/cognition/
git commit -m "feat(core): HeadlessClaudeLauncher adapter — claude -p subprocess"
```

---

## Task 8: Infrastructure — add `launch_command` to preferences

**Files:**

- Modify: `crates/core/src/infrastructure/preferences.rs`

- [ ] **Step 1: Write the failing test**

In the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn launch_command_defaults_to_claude() {
    let prefs = Preferences::default();
    assert_eq!(prefs.cognition.launch_command, "claude");
}

#[test]
fn launch_command_round_trips_via_toml() {
    let mut prefs = Preferences::default();
    prefs.cognition.launch_command = "/usr/local/bin/claude".to_string();
    let serialized = toml::to_string(&prefs).unwrap();
    let back: Preferences = toml::from_str(&serialized).unwrap();
    assert_eq!(back.cognition.launch_command, "/usr/local/bin/claude");
}
```

- [ ] **Step 2: Run — verify fail**

Expected: `launch_command` field missing.

- [ ] **Step 3: Add the field**

Find `CognitionPrefs` in `preferences.rs`. Add:

```rust
#[serde(default = "default_launch_command")]
pub launch_command: String,
```

and:

```rust
fn default_launch_command() -> String {
    "claude".to_string()
}
```

Wire it into the `Default` impl of `CognitionPrefs`. No migration shim — older `preferences.toml` files without the field deserialize to the default.

- [ ] **Step 4: Run tests**

```bash
cargo test -p secretariat-core preferences
```

Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/infrastructure/preferences.rs
git commit -m "feat(core): preferences cognition.launch_command (default claude)"
```

---

## Task 9: Application — `launch_channel` + `dispatch_channel` use cases

**Files:**

- Create: `crates/core/src/application/launch_channel.rs`
- Create: `crates/core/src/application/dispatch_channel.rs`
- Modify: `crates/core/src/application/mod.rs`

- [ ] **Step 1: Implement `launch_channel`**

```rust
//! Resolve a channel's bound cwd and produce a launch plan. Pure
//! orchestration — no subprocess spawned here; the caller (CLI or MCP
//! host) decides exec vs spawn-detach.

use std::path::Path;

use crate::infrastructure::{AliasMap, BindingLookup};
use crate::ports::cognition_launcher::{CognitionLauncher, LaunchPlan, LauncherError};
// [Engineer: thread the same channel-uri parser used by bind_channel.]

pub fn launch_channel(
    aliases: &AliasMap,
    bindings: &BindingLookup,
    substrate_root: &Path,
    launcher: &dyn CognitionLauncher,
    channel_uri: &str,
) -> Result<LaunchPlan, LaunchChannelError> {
    let recipient = parse_channel_uri(channel_uri)?;
    let cwd = crate::infrastructure::resolve_queue_dir(
        aliases, &recipient, substrate_root, bindings,
    );
    Ok(launcher.plan_launch(&cwd)?)
}
```

- [ ] **Step 2: Implement `dispatch_channel`**

```rust
//! Headless invocation of cognition inside a channel's bound cwd.

pub fn dispatch_channel(
    aliases: &AliasMap,
    bindings: &BindingLookup,
    substrate_root: &Path,
    launcher: &dyn CognitionLauncher,
    channel_uri: &str,
    task: &str,
) -> Result<String, DispatchChannelError> {
    let recipient = parse_channel_uri(channel_uri)?;
    let cwd = crate::infrastructure::resolve_queue_dir(
        aliases, &recipient, substrate_root, bindings,
    );
    Ok(launcher.dispatch(&cwd, task)?)
}
```

- [ ] **Step 3: Tests with mock launcher**

```rust
struct MockLauncher {
    last_cwd: std::sync::Mutex<Option<PathBuf>>,
    last_task: std::sync::Mutex<Option<String>>,
    response: String,
}

impl CognitionLauncher for MockLauncher { /* record call args, return self.response */ }

#[test]
fn dispatch_routes_to_bound_cwd() {
    // Build BindingLookup with one entry, dispatch, assert MockLauncher
    // saw the bound path (not the default substrate path).
}

#[test]
fn launch_unbound_channel_falls_back_to_default_substrate_path() { /* ... */ }
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p secretariat-core launch_channel dispatch_channel
```

Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/application/launch_channel.rs crates/core/src/application/dispatch_channel.rs crates/core/src/application/mod.rs
git commit -m "feat(core): launch_channel + dispatch_channel use cases"
```

---

## Task 10: CLI — `sec bind`, `sec launch`, `sec dispatch`

**Files:**

- Create: `crates/cli/src/commands/bind.rs`
- Create: `crates/cli/src/commands/launch.rs`
- Create: `crates/cli/src/commands/dispatch.rs`
- Modify: `crates/cli/src/main.rs`
- Create: `crates/cli/tests/bind_launch_dispatch.rs`

- [ ] **Step 1: Define the subcommands**

In `main.rs`, register:

```
sec bind <channel-uri> --path <dir>
sec launch <channel-uri>
sec dispatch <channel-uri> <task...>
```

`launch` uses `exec` (process replacement via `std::os::unix::process::CommandExt::exec`) after computing the plan. `dispatch` prints stdout to the parent.

- [ ] **Step 2: Implement each command**

Each command:

1. Loads `KeyPaths` + `AliasMap` + `Preferences` + builds `BindingLookup::load_from(...)`.
2. Constructs a `HeadlessClaudeLauncher::new(prefs.cognition.launch_command.clone())`.
3. Calls the application use case.
4. Renders human + `--json` output.

`bind`'s `--json` output: `{ "bound_path": "...", "gitignore_written": true, "existing_binding_replaced": null }`.

- [ ] **Step 3: Integration test**

`crates/cli/tests/bind_launch_dispatch.rs`:

```rust
// Spawn a tempdir HOME, run `sec init`, `sec create_org`, `sec create_channel`,
// `sec bind <channel> --path <tmp/repo>`. Assert:
//   - <tmp/repo>/contract.local.md exists with root_path frontmatter
//   - <default-channel-dir> is a symlink pointing at <tmp/repo>
//   - <tmp/repo>/.gitignore (when .git/ present) contains the fenced block
// Run `sec launch <channel> --print-plan` (don't actually exec in tests):
//   - asserts the JSON plan's cwd == <tmp/repo>
// Run `sec dispatch <channel> "echo-test"` with PREFERRED_COGNITION_COMMAND=/bin/echo:
//   - asserts stdout contains "echo-test"
```

For the dispatch test, override the launch command via a CLI flag `--launch-command /bin/echo` (add to dispatch.rs) so tests don't depend on `claude` being installed.

- [ ] **Step 4: Run tests**

```bash
cargo test -p sec --tests
```

Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/commands/ crates/cli/src/main.rs crates/cli/tests/bind_launch_dispatch.rs
git commit -m "feat(cli): bind / launch / dispatch verbs for channel-to-dir binding"
```

---

## Task 11: MCP — `bind_channel`, `launch_channel`, `dispatch` tools

**Files:**

- Modify: `crates/mcp/src/server.rs`
- Create: `crates/mcp/tests/bind_launch_dispatch.rs`

- [ ] **Step 1: Add the three `#[tool]` entries**

```rust
/// Bind a channel's on-disk home to a host filesystem directory.
/// Idempotent: re-binding moves contents and redirects the symlink.
/// Writes a fenced `.gitignore` block when the target is a git repo.
#[tool]
async fn bind_channel(&self, params: BindChannelParams) -> McpResult<BindChannelOutput> {
    // delegate to application::bind_channel
}

#[derive(Deserialize, JsonSchema)]
struct BindChannelParams {
    /// Channel URI, e.g. `did:web:themia.pro#channel:dev:secretariat`.
    channel: String,
    /// Absolute host path to bind the channel-dir to.
    path: PathBuf,
}

#[derive(Serialize, JsonSchema)]
struct BindChannelOutput {
    bound_path: PathBuf,
    gitignore_written: bool,
    existing_binding_replaced: Option<PathBuf>,
}

/// Plan an interactive launch in the channel's bound cwd. Returns the
/// command + cwd so the host (Claude Code) can open a terminal there.
#[tool]
async fn launch_channel(&self, params: LaunchChannelParams) -> McpResult<LaunchChannelOutput> {
    // delegate to application::launch_channel; return LaunchPlan
}

/// Headless: run `claude -p <task>` (or configured cognition command)
/// in the channel's bound cwd. Returns the final response. Cheap way
/// for a parent Claude session to fan subagents into different
/// channel contexts.
#[tool]
async fn dispatch(&self, params: DispatchParams) -> McpResult<DispatchOutput> {
    // delegate to application::dispatch_channel
}

#[derive(Deserialize, JsonSchema)]
struct DispatchParams {
    channel: String,
    task: String,
}

#[derive(Serialize, JsonSchema)]
struct DispatchOutput {
    response: String,
    cwd: PathBuf,
}
```

Tool descriptions should match Secretariat's bounded-context tone (see existing tool comments for length and shape).

- [ ] **Step 2: Integration test**

`crates/mcp/tests/bind_launch_dispatch.rs`: spawn an MCP server against a tempdir HOME, call `bind_channel`, then `dispatch` with the test override pointing at `/bin/echo`. Assert the dispatched response contains the task string.

- [ ] **Step 3: Run tests**

```bash
cargo test -p sec-mcp --tests
```

Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add crates/mcp/src/server.rs crates/mcp/tests/bind_launch_dispatch.rs
git commit -m "feat(mcp): bind_channel / launch_channel / dispatch tools"
```

---

## Task 12: Workspace verification

**Files:** none.

- [ ] **Step 1: Full workspace test**

```bash
cargo test --workspace
```

Expected: all green.

- [ ] **Step 2: Clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Expected: clean.

- [ ] **Step 3: Manual smoke test against a real repo**

```bash
# In a fresh shell:
cargo run -p sec -- init   # if a fresh HOME, otherwise skip
cargo run -p sec -- create_org themia.pro
cargo run -p sec -- create_channel channel:dev:secretariat --org themia.pro
cargo run -p sec -- bind channel:dev:secretariat --path /Users/rafa/Developer/equanimitech/secretariat

# Verify
ls -la ~/.secretariat/themia.pro/channel/dev/secretariat   # should be a symlink
cat ~/Developer/equanimitech/secretariat/.gitignore | grep secretariat

cargo run -p sec -- dispatch channel:dev:secretariat "what's in CLAUDE.md? one sentence."
# Expected: claude -p response summarizing AGENTS.md (loaded as project memory).
```

- [ ] **Step 4: No commit**

Smoke test only; if anything broke, file a follow-up task. Do not modify code as part of this step.

---

## Task 13: Docs

**Files:**

- Modify: `AGENTS.md`
- Create: `docs/developer/launch-dispatch.md`

- [ ] **Step 1: Update AGENTS.md "What's here today"**

Add a bullet after the MCP server bullet:

```markdown
- **Channel binding (`sec bind` / `sec launch` / `sec dispatch`)** —
  bind a channel's on-disk home to a host directory (typically a git
  repo). `launch` opens Claude Code interactively in that cwd;
  `dispatch` runs `claude -p <task>` headless and returns the result —
  parent Claude sessions use it to fan subagents into different
  channel contexts. Binding lives in `contract.local.md` (receiver-
  private, per-device); `.gitignore` block auto-written when the bound
  path is a git repo.
```

- [ ] **Step 2: Write operator notes**

`docs/developer/launch-dispatch.md`: one short page covering binding flow, gitignore block shape, and the auth caveat for daemon-spawned dispatch (refer back to Task 0 findings).

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md docs/developer/launch-dispatch.md
git commit -m "docs: launch/dispatch/bind — operator notes + AGENTS update"
```

---

## Out of scope (explicit no-gos)

- `dispatch --background` — defers to `AgentSupervisor` (separate plan, not this one).
- GUI binding flow — MCP + CLI only for this slice.
- Multi-device path federation — bindings are receiver-private.
- Cross-channel global ordering — channels remain independent logs.
- Migration of existing `~/.secretariat/<channel>/` content into bound paths _automatically_ — `bind_channel` moves contract.local.md but leaves `envelopes/` and `outbox/` to follow the symlink. If users want eager moves later, add a `--move-contents` flag in a follow-up.

## Risks recorded

- **Symlink + FS watcher behavior on macOS.** `notify` should follow the symlink target; verify under daemon load before declaring done. If broken, fall back to per-channel watcher registration keyed off the resolved (bound) path.
- **`claude -p` auth in non-TTY contexts.** Validated in Task 0. If daemon-spawned dispatch fails, that's a known gap for `--background` (out of scope here).
- **`/bin/echo` as test fixture.** Behavior differs slightly across BSD/GNU echo. If CI runs Linux, adjust the test fixture to `/usr/bin/printf` or a tiny `sh -c 'cat'` wrapper.
