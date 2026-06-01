---
tag: pitch
appetite: medium
status: draft
source: docs/ideas/2026-05-31-background-sessions.md
hard_dependency: git-native substrate (repos.json registry) — docs/ideas/2026-05-31-git-native-substrate.md
slice_id: A
---

# Pitch — Background sessions: launchd fires `claude -p`, the stamp gates

**Bet:** Repoint the surviving launchagent from `daemon serve` to a scheduler that runs headless `claude -p` in registered repo roots, commits its output, and leaves it for review — no resident daemon comes back.

**Why it matters:** The substrate goes stale between launches. An always-on scribe keeps inbox/journal/enrichment current so the principal arrives to a ready review surface, not an empty repo.

---

## Boundaries

**JBTD:** When I'm away from my desk, I want the scribe to keep my repos current — process new inbound docs, journal what changed, enrich captures — so that my next review session opens on fresh work instead of a cold start. Baseline today: nothing runs unless I `sec launch` and drive it by hand; the launchagent still boots a `daemon serve` poll loop that git-native already deleted the federation guts of.

**Out:**
- No resident daemon, no long-lived process, no IPC socket. launchd is the only heartbeat.
- No auto-stamp. Background runs leave signed-or-unstamped docs; the principal stamps at review (hard rule #4).
- No new cognition adapter. Reuse the configured `launch_command` / `launch_env`; `-p` is an arg, not a fork.
- No routing/attention engine. Each run executes a fixed per-repo prompt, not a triage brain.

## Elements

- **Scheduled-run plist** (`crates/daemon/src/launchagent.rs:31`). Rewrite `render_plist` to invoke a new `sec dispatch` subcommand instead of `daemon serve`; add `StartCalendarInterval` (or `StartInterval`) so launchd wakes it on cadence. `RunAtLoad`/`KeepAlive` drop — a scheduler is one-shot-per-fire, not kept alive.
- **`HeadlessPlan` from the existing launcher** (`crates/core/src/infrastructure/cognition/launcher.rs:84`). `plan_launch` already returns `{command, args, cwd, env}`. Add the configured prompt as `claude -p "<prompt>"` and a non-interactive flag set; same `PrefsLauncher`, same per-target override merge.
- **`sec dispatch`** (`crates/cli/src/commands/dispatch.rs`, new — sibling to `launch.rs:48`). Walk `repos.json`, for each repo with a background config: resolve prompt + cwd, spawn-and-wait `claude -p` (not `exec` — dispatch loops over N repos), then `git add -A && git commit` the result under a `[scribe]` author trailer.
- **Per-repo run config** (`<repo>/.claude/background.md` or a block in `repos.json`). Declares the prompt (`process inbox` / `journal` / `enrich`) + cadence. Absent = repo opts out. Mirrors the `root_path` binding precedent.
- **Commit, don't surface** (git-native review walker). The run's only output is a commit. `/review-repos` already derives NEW/UNSTAMPED/REVISED from git + Signet verify — background commits land as UNSTAMPED rows the principal sees next review. No notification path.

## Risks

**🐇 Rabbit holes:**
- `claude -p` non-interactive flags + auth in a launchd context (no inherited shell, no keychain prompt). Pin the exact flag set and a smoke test before building the loop.
- Concurrent runs racing the same repo's git index. Serialize per-repo; a stale `.git/index.lock` must abort the run, not wedge it.
- Cost/runaway: a 15-min cadence × N repos × a model that loops. Cap with a per-run timeout and a cadence floor.

**🏴 Off-sides:**
- Routing/triage ("decide which repo needs attention"). That's the deferred attention engine — out. Dispatch runs a fixed prompt per repo.
- Auto-PR / auto-push. Background commits stay local; push is a principal act.

**🥩 Fat cut:**
- A `daemon_tick`-style MCP tool to trigger runs on demand. Tempting, but launchd + a manual `sec dispatch` cover it; no MCP surface this slice.
- Streaming run logs to a UI. The commit *is* the log; `git log --author=scribe` reads it.

**🧪 Domain knowledge:**
- Confirm `claude -p` runs headless under launchd with the user's Claude subscription (not just API-key/BYOK). If it needs an interactive login, the whole bet shifts to the Anthropic-API adapter — verify first.
- `repos.json` shape isn't built yet (git-native Open Question #6). Dispatch depends on it; if it slips, gate behind an absolute-path list in preferences.

## Acceptance

1. `sec daemon install` writes a plist that runs `sec dispatch` on a calendar/interval schedule, with `KeepAlive` removed; `launchctl list` shows it loaded.
2. `sec dispatch` walks `repos.json`, and for each repo with a background config runs `claude -p` with the resolved prompt + cwd + env from the existing launcher.
3. A scheduled run that produces output leaves exactly one git commit per repo, authored `[scribe]`, with no `$attestation` (unstamped).
4. `/review-repos` (or `sec review`) surfaces that commit as an UNSTAMPED/REVISED row at next review; nothing auto-surfaces or auto-stamps.
5. A repo with no background config is untouched — no commit, no run.
6. A run that hits a `.git/index.lock` or exceeds the per-run timeout aborts cleanly and logs to the daemon log dir; the next fire is unaffected.

---

_Drafted by Claude (scribe). Source: docs/ideas/2026-05-31-background-sessions.md._
