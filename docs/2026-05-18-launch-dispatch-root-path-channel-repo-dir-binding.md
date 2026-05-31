---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T101613Z-ir5k6y.md
---
# launch + dispatch + root_path — channel→repo/dir binding & cross-channel subagents

Raw capture — 2026-05-13. Promoted to secretariat:dev from `_self/inbox/triage` 2026-05-18.

**Status check 2026-05-18:** `launch_channel` shipped (`crates/core/src/application/launch_channel.rs`); `root_path` shipped via `channel_binding` domain entity (`crates/core/src/domain/channel_binding.rs` + `binding_store.rs`). **`dispatch` NOT shipped** (the IPC `dispatch` in `daemon/src/ipc/server.rs:132` is request routing, not cross-channel agent dispatch).

Remaining work: `sec dispatch <channel> <task>` + MCP `dispatch` (headless `claude -p` with cwd = root_path), the persistence-matrix integration via AgentSupervisor, and `dispatch --background` as sugar over the supervisor.

---

- Single field on consumption contract: `root_path: Option<PathBuf>` on `contract.local.md` (receiver-private). Drop the earlier `repo_path` distinction — `.git/` presence is a runtime check, not a schema split.
- Resolver: leaf-level `root_path` short-circuits default `~/.secretariat/<alias>/<handle>/`. Lives in `accumulate_resolver` (slice 3 shape).
- `sec init` (or `sec bind <channel> --path <dir>`) detects `.git/` → writes `.gitignore` block. No `.git` → skip. Same path either way.
- Gitignore split: committed = `CLAUDE.md`, `skills/`, `agents/`, `_meta/envelopes/` (roster/governance), eventual `contract.md`. Ignored = `contract.local.md`, `envelopes/` (decrypted cache), `outbox/`, `_ciphertext/`.
- `sec launch <channel>` + MCP `launch_channel` — resolves root_path, execs configured cognition in that cwd. Profile field `cognition.launch_command` defaults `claude`.
- `sec dispatch <channel> <task>` + MCP `dispatch` — headless via `claude -p`. Subprocess with cwd = root_path. Subagent inherits `.claude/` tree automatically. Returns final stdout.
- Persistence matrix lands clean:
  - same channel / one-shot → Task (free, exists)
  - same channel / session-persistent → Task background (free, exists)
  - same channel / forever → AgentSupervisor (daemon)
  - cross channel / one-shot → dispatch
  - cross channel / session-persistent → dispatch --background (sugar over AgentSupervisor)
  - cross channel / forever → AgentSupervisor
- `dispatch --background` doesn't grow a parallel persistent mechanism — routes through AgentSupervisor.
- Wider scope (e.g. `themia` org channel) can point root_path at an umbrella dir hosting multiple repos as siblings; sub-channels independently bind their own repo roots. No nested-symlink hell.
- Nesting Claude-in-Claude via `claude -p` works (subprocess isolation), bills same account — fine for v0.3.

Questions:
- Auth surface for `claude -p` headless when daemon-launched (no terminal) — does it inherit principal's login cleanly?
