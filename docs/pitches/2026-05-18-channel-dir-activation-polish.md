# Channel-dir activation polish

Pitch — 2026-05-18. Source: free-text request (Claude Code large-codebases article take-aways, items 1+2+5)

## Boundaries

### Job to be done
As a principal `cd`-ing into a channel-dir to work with Claude Code, I want the directory to be a *legible Claude Code project on arrival* — ignore-rules baked in, three artifacts (governance / navigation / consumption) each with a clear audience, agent roster named — so that no one has to relearn what each file is for or hand-author `.claudeignore` per channel.

Baseline today: `create_channel` writes `channel.md` + `contract.local.md` stub + `envelopes/`. No `.claudeignore`. No `CLAUDE.md`. Subagent intent (read-only digest vs write-mode compose) is implicit in `project_secretariat_agent` memory but not in code or scaffold.

### Appetite
`small`

## Elements

- **Place:** `<channel-dir>/.claudeignore` — written at `create_channel` time. Static content: ignore `_ciphertext/`, `.archive/`, `outbox/.tmp/`, `*.tmp`. Same idempotency rule as `contract.local.md` (never clobber hand edits).
- **Place:** `<channel-dir>/CLAUDE.md` — written at `create_channel` time. Stub content: 6–10 line pointer file naming the channel, the three sibling artifacts and their audience, and the agent roster. Editable; never clobbered on re-scaffold.
- **Affordance:** principal-overrideable stubs at `<self_root>/claudeignore-stub.md` and `<self_root>/channel-claude-stub.md` (mirrors the existing `<self_root>/contract-stub.md` pattern — `KeyPaths` already has the precedent).
- **Connection:** rename the in-flight agent intents to `secretariat-digest` (read-only, channel-mapping) and `secretariat-compose` (writes to outbox). Document in `CLAUDE.md` stub. Code rename if/when agent code lands; today it's a memory-level naming commitment.
- **Affordance:** AGENTS.md hard-rule addendum naming the three-artifact split — `channel.md` (governance, signed envelopes-to-be, roster + artifact policy), `CLAUDE.md` (AI navigation, principal-editable, never on wire), `contract.local.md` (subscriber consumption, private, never on wire). Each line says *who the audience is* and *whether it travels*.

## Risks

### 🐇 Rabbit holes
- Tempting to fold the `CLAUDE.md` stub into the `channel.md` manifest "since both are channel metadata." Don't — the article's core finding is *lean + layered, separate audiences*. Conflating them collapses the split we're trying to make bright.
- `.claudeignore` precedence with parent CLAUDE.md walk: verify Claude Code respects per-dir `.claudeignore` even when the dir is reached via `cd <channel-dir>` rather than as a subtree of a parent project root. Likely fine (Claude Code treats the cwd as project root), but cite the doc when implementing.

### 🏴 Off-sides called
- Out: editing the `CLAUDE.md` walker — already shipped in v0.6 accumulator.
- Out: renaming `channel.md` / `contract.local.md` files. The semantic split was already there; this pitch just *documents* it brightly and adds the missing third artifact.
- Out: building digest / compose subagent code. This pitch only commits to the names + roles in docs.

### 🥩 Fat cut
- A `skills-stub.md` per channel — tempting (mirror `.claude/skills/<channel>.md` scaffold) but skill seeding belongs in the skillDrop pitch, not here. JBTD resolves without it.
- Per-channel `.gitignore` — channel-dirs are not git repos by default. `.claudeignore` is sufficient.

### 🧪 Domain knowledge
- Confirm Claude Code reads `.claudeignore` at the cwd-as-project-root level (article implies yes; cite the doc paragraph in code comments).
- Confirm AGENTS.md is the right home for the three-artifact split (vs `docs/developer/secretariat-architecture.md`) — AGENTS.md is read every session, architecture doc is reference. The split is a behavioral rule → AGENTS.md.

## Pitch

### Problem
The channel-dir bet — that `cd <channel-dir> && claude` activates everything — only pays off if a fresh arriver can see at a glance which file is for what. Today four artifacts share a directory (`channel.md`, `contract.local.md`, `envelopes/`, optional `template.md`) but only two are stubbed on creation, and the AI-navigation layer (`CLAUDE.md`) is absent. The article's *"keep root files focused on pointers and critical gotchas only; everything else drifts into noise"* applies channel-by-channel: each channel-dir IS a project root.

Symptom: when an agent is dispatched into a channel, it has no pointer telling it what the channel does, where the governance lives, what the consumption preferences are, or which sibling artifacts to read first. It reverse-engineers from filenames. That's noise, exactly what the article warns against.

### The bet
Ship a small scaffold pass: `create_channel` writes a `CLAUDE.md` stub + `.claudeignore` alongside the existing `channel.md` and `contract.local.md`. AGENTS.md gains a short addendum naming the three-artifact split (governance / navigation / consumption) with audience + on-wire bit per artifact. Subagent intent gets named (`secretariat-digest` / `secretariat-compose`) in docs only — code wiring waits for the agent slice.

Pays off because: the legibility cost is paid once at scaffold time; every future agent dispatch starts with the right pointer. And because the three stubs are all idempotent (`channel.md` exists-gate, `contract.local.md` no-clobber, same rule extended), hand-edits survive.

### No-gos
- No code rename of subagent roles in this slice.
- No `skills/` directory scaffolding (deferred to skillDrop pitch).
- No edit to existing channels' missing `.claudeignore` / `CLAUDE.md` — only new `create_channel` calls write them. Hand-add or `sec channel rescaffold` is a follow-up if demanded; not bet here.
- No change to the `channel.md` / `contract.local.md` / `envelopes/` shapes already shipped.
