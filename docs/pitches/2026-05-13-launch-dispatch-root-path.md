# launch + dispatch + root_path — channel→repo binding and cross-channel subagents

Pitch — 2026-05-13. Source: `/Users/rafa/.secretariat/queues/inbox/triage/20260513T160556Z-vxbpxo.md`

## Boundaries

### Job to be done

As a principal working out of a channel that maps to a real working
directory (a git repo for `dev:*` channels, a plain folder for umbrella
channels like `themia`), I want my channel-dir on disk to *be* that
working directory — and I want to launch interactive Claude Code in it or
dispatch headless subagents against it from anywhere — so that the
filesystem-authoritative substrate stops being a parallel universe to the
code I actually edit.

*When*: I'm in a parent Claude session and want to delegate work into a
specific channel's context (its `.claude/` tree, its envelopes, its
contract). Today: there's no way — channel-dirs live at
`~/.secretariat/<alias>/<handle>/` and my repos live at `~/Developer/…`.
Baseline today: I `cd` manually, start a new Claude session, lose parent
context, or I copy/paste prompts across windows.

### Appetite

`medium`

Appetite picked: `medium` — domain field is one VO, resolver hook is a
short-circuit in the accumulate chain, CLI/MCP launch is a subprocess
exec, dispatch is `claude -p` subprocess with cwd. Five surfaces but each
small. The `--background` arm routes through the existing
`AgentSupervisor` design (daemon plan) rather than building new
machinery. Override with `--appetite=small` if we cut `--background` to a
later slice.

## Elements

Five primary elements. Resolver + binding underpin everything; the verbs
are the principal-facing surface.

- **Place:** `contract.local.md` frontmatter gains one field — `root_path:
  Option<PathBuf>`. Receiver-private, lives on the consumption contract.
  **Note:** does NOT compose via the accumulate merge — `root_path` is a
  leaf-only override, not a numeric floor or restrictive enum. Treat as a
  sibling concern to `ChannelContract` in the same frontmatter, parsed
  alongside but not part of `ChannelContract`'s merge algebra. New tiny
  VO `ChannelBinding { root_path: Option<PathBuf> }`.

- **Affordance:** resolver hook. At the point where today's code computes
  the channel-dir path (`queue_dir.rs:149` and the contract walk in
  `contract_ops.rs:237`), check the leaf's binding first; if `root_path`
  is set, return it. Else fall through to the default
  `~/.secretariat/<alias>/<handle>/`. One short-circuit.

- **Connection:** `sec bind <channel-uri> --path <dir>` (CLI) +
  `bind_channel` (MCP). Writes/updates `contract.local.md` in the bound
  dir, then symlinks back from the default location so existing readers
  (daemon, MCP tools that walk the substrate today) keep working
  unchanged. On bind: detect `.git/`; if present, append a Secretariat
  block to the repo's `.gitignore` (entries: `contract.local.md`,
  `envelopes/`, `outbox/`, `_ciphertext/`). If absent, skip the gitignore
  step. Idempotent.

- **Affordance:** `sec launch <channel-uri>` (CLI) + `launch_channel`
  (MCP). Resolves binding → cwd. Execs the configured cognition command
  (`preferences.cognition.launch_command`, default `claude`). For CLI,
  replaces the current process (`exec`). For MCP, returns the resolved
  path so the host (Claude Code in a parent session) can open a terminal
  there — or, more usefully, spawn the headless variant.

- **Affordance:** `sec dispatch <channel-uri> <task>` (CLI) + `dispatch`
  (MCP). Resolves binding → cwd. Spawns `claude -p "<task>"` as a
  subprocess with that cwd. Captures stdout, returns final result string.
  Subagent inherits the channel's `.claude/` tree (CLAUDE.md, skills/,
  agents/) for free — no glue, no plumbing. `--background` flag (deferred
  arm): instead of awaiting stdout, hands the task to `AgentSupervisor`
  for managed long-running execution; returns a handle.

## Risks

### 🐇 Rabbit holes

- **Symlinking the channel-dir into a repo (or vice versa).** Two-way
  sync is hell — we explicitly aren't doing that. But the symlink choice
  (default-path → bound-path, or bound-path → default-path) interacts
  with daemon FS watchers (`crates/daemon/src/serve.rs` — outbox watcher,
  inbox watcher). On macOS, `notify` follows symlinks by default but
  edge-cases exist (`fsevents` on the target vs source). Prototype with
  one channel before generalizing.
- **`claude -p` auth when daemon-spawned.** Interactive Claude Code
  inherits the principal's login from `~/.claude/`. Headless via
  subprocess from the daemon (no TTY, may run under launchd) — does the
  auth resolve cleanly? Open question in the capture. Verify by spiking
  `Command::new("claude").arg("-p").arg("hello").current_dir(...)` from
  the daemon context before committing to the MCP shape.
- **Recursive dispatch.** Parent Claude calls `dispatch` → subagent
  (`claude -p`) is itself a Claude Code session → subagent calls
  `dispatch` again. Works mechanically (subprocess isolation), but no
  cycle detection. Out of scope; acceptable.
- **`.gitignore` writer in a dirty repo.** First bind in a repo with
  uncommitted changes or an existing `.gitignore` Secretariat block.
  Append idempotently with a sentinel comment block (`# === secretariat
  ===` … `# === /secretariat ===`) and a fixed-shape entry list; if the
  block exists, replace its contents in place. Never rewrite outside the
  fenced region.

### 🏴 Off-sides called

- Multi-device same-principal sync of bindings. Each device has its own
  `root_path` for the same channel (Rafa's laptop vs desktop point at
  different repo clones). That's fine — `contract.local.md` is
  receiver-private and per-device. No federation of paths.
- A "workspace manifest" listing all bindings. Tempting (one file to view
  every channel→path mapping), but `contract.local.md` is the source of
  truth; a manifest is a read-cache. Defer.
- GUI surface for binding. The principal binds via CLI or MCP. The Tauri
  navigator [[project_mcp_is_primary_interface]] can deep-link to
  `bind_channel`, but no settings pane.

### 🥩 Fat cut

- `sec launch` could grow `--prompt "<initial message>"` to seed the
  interactive session. Cut — `dispatch` already covers headless-with-
  prompt; launch is the bare interactive opener.
- `--background` arm of `dispatch`. Split into a follow-up slice if
  appetite tightens — the one-shot path is enough to unblock the JBTD
  (parent agent fans out across channels, awaits results). Persistent
  cross-channel agents already have a home (`AgentSupervisor` per the
  daemon plan) and don't *need* a `dispatch` entry point on day one.
- Profile-side `launch_command` config. Default `claude` hardcoded for
  the first slice; promote to `preferences.cognition.launch_command` only
  if a second cognition substrate (e.g. local CLI wrapper) actually
  shows up.

### 🧪 Domain knowledge

- Confirm `claude -p` is the right invocation for headless mode in
  current Claude Code (vs Claude Agent SDK). Capture noted this; verify
  before committing to the subprocess approach. Context7 lookup for
  `claude-code` CLI docs is the safest single-step check.
- Confirm daemon-spawned `claude -p` inherits OAuth credentials via the
  ambient filesystem (no TTY, no keychain prompt). Spike before locking
  the MCP `dispatch` contract.
- `notify` crate behavior on macOS when watched directory is a symlink
  target whose source moved. Probably fine for our case (we don't move
  bound paths), but worth a five-minute test.

## Pitch

### Problem

Secretariat's filesystem-authoritative principle [[project_filesystem_authoritative]]
makes channel-dirs first-class Claude Code projects
[[project_channel_dir_is_activation_surface]] — but only for channels
whose canonical home is under `~/.secretariat/`. For `dev:*` channels
that wrap a real git repo, the substrate currently forces a choice: keep
the channel-dir under `~/.secretariat/` and live with skills/CLAUDE.md
divorced from the code they describe, or hand-mirror files between two
locations and watch them drift. Neither preserves the AI feedback loop
the architecture explicitly protects.

The cross-channel orchestration story has the same gap from the other
side. A parent Claude session in some root context can't delegate "go do
this in the Themia book channel" or "review PR #42 in the Secretariat
dev channel" without losing its own context. Task tool stays in the
current cwd; new sessions lose parent state. There's no primitive for
*cross-channel* delegation that respects each channel's `.claude/` tree.

Both problems collapse to the same missing piece: a way to tell the
resolver "this channel lives at *that* directory," plus two verbs that
consume that binding (launch for humans, dispatch for parent agents).

### The bet

Add `root_path` to `contract.local.md` (as a tiny sibling VO, not
mangled into `ChannelContract`'s merge algebra — different semantics).
Hook the resolver to honor it. Ship `bind` + `launch` + `dispatch` as a
medium-appetite slice across CLI and MCP. `dispatch` headless uses
`claude -p` as a subprocess — zero new dependencies, principal's
existing Claude Code login carries through, channel `.claude/` tree
loads automatically via cwd. The persistent-background arm of
`dispatch` defers to the existing `AgentSupervisor` design instead of
growing a parallel mechanism.

The bet pays off when the principal can, from one parent Claude session,
fire `dispatch(themia:dev:secretariat, "review PR #42")` and
`dispatch(marcelo:book, "summarize ch.5 feedback")` in parallel, get
results back, and stitch them — without window-juggling, without losing
context, and without any channel's skills/agents leaking into another's
session. Same primitive serves human use (`launch` → terminal in the
right cwd) and agent use (`dispatch` → headless in the right cwd).
Filesystem stays authoritative; bindings are receiver-private; nothing
flies on wire.

### No-gos

- No two-way sync between bound dir and `~/.secretariat/`. Single
  inode via symlink, or single canonical bound path with the default
  location pointing to it. Pick one, document, hold the line.
- No federated path bindings. `root_path` is local-machine, never
  shared with the roster.
- No new persistence mechanism for `dispatch --background`. Routes
  through `AgentSupervisor` (daemon's design) or doesn't ship in this
  pitch.
- No GUI binding flow in v0.3. MCP + CLI only.
- No cross-channel global ordering of dispatched work
  [[project_owner_as_sequencer]]. Each channel's sequencer is its
  owner's relay; dispatched subagents read/write through normal
  channel discipline.
