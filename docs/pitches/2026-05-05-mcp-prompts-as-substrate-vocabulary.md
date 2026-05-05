# MCP prompts as substrate vocabulary

Pitch — 2026-05-05. Source: free-text shape (extends `docs/ideas/secretariat-as-company-os.md` and `docs/pitches/2026-05-05-event-sourced-envelope-substrate.md`).

**Hard dependency:** the `capture` tool in `crates/mcp/src/server.rs:398` (slice 1c, shipped in v0.2.4) — every prompt that captures or shapes is a thin wrapper over it.

## Boundaries

### Job to be done

When a non-Rafa principal — Marcelo, Christophe, dad — installs Secretariat for the first time and connects it to Claude, they have access to the `capture` tool but no *verbs* for when to use it. Today's baseline: they wouldn't use it at all, or Rafa would have to manually copy `~/.claude/skills/{idea,pain,shaping,share,roundtable}/SKILL.md` into their `~/.claude/skills/` for each principal, on each machine. Tomorrow: install DMG, the vocabulary appears.

### Appetite

`small` — one focused day. Mostly text porting + glue.

## Elements

Breadboarding the surface:

- **Place:** `crates/mcp/src/prompts/` (new directory). One markdown body per prompt — `idea.md`, `pain.md`, `shaping.md`, `share.md`, `roundtable.md`. Each is the SKILL.md body, lightly edited to drop the "is the MCP available?" decision tree (it always is, by definition — the prompt only fires *because* the MCP is wired).
- **Affordance:** rmcp 0.8 `#[prompt]` macro in `crates/mcp/src/server.rs`, five handlers, each returning the body via `include_str!`. Same module that already exposes `compose`, `stamp`, `capture`, etc. as `#[tool]`.
- **Connection:** principal types `/idea …` (or whatever Claude Code's MCP-prompt prefix turns out to be) → Claude Code requests the prompt from sec-mcp → MCP returns the body → Claude treats it as instructions for the current turn → calls `capture` tool with `queue: inbox:triage` and the user's phrasing as `body`. The substrate writes the file. Done.

That's the whole surface. Five static markdown files, five handler stubs, one dispatcher.

## Risks

### 🐇 Rabbit holes

- **rmcp 0.8 prompt-macro ergonomics.** We've used `#[tool]` heavily but never `#[prompt]`. Need to confirm: does it support `$ARGUMENTS`-style param substitution? Does it accept dynamic args at all, or only return static text? If args don't work natively, the prompt body would have to instruct Claude to read the user's *next* message as input — workable but uglier UX. Mitigation: spike `idea.md` first with one arg, prove the path.
- **Slash-command surfacing.** MCP spec says prompts surface as slash commands. In practice, Claude Code might prefix them — `/mcp__secretariat__idea` instead of `/idea`. That collides with the existing user-skill `/idea` and creates a UX rough patch. Mitigation: verify at spike time. If unavoidable, we ship under the prefixed name and document the alias; `~/.claude/skills/idea/` users get to keep the bare name.
- **Two `/idea` skills competing.** Once the MCP prompt ships, Rafa has *two* `/idea` paths: his local skill and the MCP prompt. Both route to the same `capture` tool, so the *outcome* is identical, but Claude has to pick one when the user types `/idea`. Mitigation: once verified the MCP version works, delete the local skill from `~/.claude/skills/`. One canonical path per principal.

### 🏴 Off-sides called

- **Porting Rafa-personal skills** (`/runway`, `/page`, `/multi-mind`, `/teach-rust`, `/themia-copywriting`, etc.). These are *Rafa's* config, not substrate vocabulary. They don't ship.
- **Per-principal prompt customization.** Letting Marcelo override `/idea`'s body with his own template. Future pitch. v0.3 ships static.
- **A prompt-registry / marketplace.** Hard no — collapses sovereignty (rule 1: no central server). Vocabulary is shipped *in the binary*.
- **Rich prompt args / MCP-prompt UI affordances.** Prompts take a string. Anything richer is shaping — out of scope.

### 🥩 Fat cut

- **`/page` and `/multi-mind` style meta-skills.** These manipulate Claude's session state, not Secretariat state. Don't belong in sec-mcp.
- **`/check`, `/cleanup`, dev workflow skills.** Engineering hygiene, not principal vocabulary.
- **Versioning the prompts independently.** They version with the binary. v0.2.5 ships a new `/intent` prompt → bump and re-DMG. Same flow as new tools.

### 🧪 Domain knowledge

- **Verify Claude Code surfaces MCP prompts as slash commands at all.** This is the load-bearing assumption. Quick test before committing: register one trivial prompt in the spike, install via `claude mcp`, see if `/<name>` appears in the picker. If it doesn't, the whole pitch dies and we keep skills local.
- **Verify Claude Desktop has parity.** Some principals (likely Marcelo) will run Claude Desktop, not Claude Code. If Desktop doesn't surface MCP prompts, the substrate-OS framing only works for half the install base. Test both clients during spike.
- **Claude.ai web behaviour.** If Marcelo uses claude.ai with the Secretariat MCP wired, do prompts appear there too? Lower priority — the steady-state install is desktop/CLI — but worth noting.

## Pitch

### Problem

Secretariat's value to a non-Rafa principal collapses when they install it and don't know what to *say*. The `capture` tool exists, but a tool without a verb is opaque — Marcelo wouldn't know to type "capture this idea about the book" any more than he'd type "execute the postsynaptic-receptor protocol." He needs `/idea`, the way Rafa has `/idea`. Today that verb lives in `~/.claude/skills/idea/SKILL.md` on Rafa's machine. Same machine. Same user.

This breaks the framing that Secretariat is "the operating system of a company" (per `docs/ideas/secretariat-as-company-os.md`). An OS without standard verbs isn't an OS — it's a kernel without a shell. Every new principal becoming a manual port of Rafa's skill directory is the inverse of substrate-shaped distribution.

The natural place for the vocabulary is the binary that ships the substrate. MCP servers can expose `prompts` (per the protocol), and Claude Code surfaces them as slash commands. So sec-mcp ships with `/idea`, `/pain`, `/shaping`, `/share`, `/roundtable` baked in. Install DMG, vocabulary appears, no skill-distribution problem.

### The bet

One day. Five universal prompts ported into `crates/mcp/src/prompts/` as static markdown, registered via rmcp `#[prompt]` macros, shipped in the v0.2.5 DMG. Each prompt body is a lightly-edited copy of the corresponding `~/.claude/skills/<name>/SKILL.md`, with the "is MCP available?" decision tree dropped — by definition, if the prompt fires, the MCP is wired.

This pays off because:
- **One install, full vocabulary.** Marcelo's onboarding goes from "Rafa walks me through capturing an idea" to "I type `/idea …` and it works."
- **Vocabulary versions with the binary.** New verb in v0.3.0? Bump and re-DMG. Every principal gets it on next update. No skill-sync problem.
- **Substrate-OS framing earns its name.** Decisions, drafts, captures, conventions all live in (or are shipped with) the same binary that holds the keys and the queues. The OS metaphor stops being aspirational.

Spike day-1: register one prompt (`idea.md`), install via the dev MCP, verify slash-command surfacing in both Claude Code and Claude Desktop. If verified, port the other four. If not, kill the pitch and keep skills local — the rest of the day reverts to other slice 1c work.

### No-gos

- Not porting Rafa-personal skills (`/runway`, `/page`, `/multi-mind`, etc.) — those stay in `~/.claude/skills/`.
- Not building per-principal prompt customization. Static bodies in v0.3.
- Not building a prompt-registry or marketplace. Vocabulary ships in the binary, full stop.
- Not deduplicating with the local user-skill `/idea` until the MCP version is proven — once proven, delete the local skill in a follow-up commit.
- Not handling claude.ai web parity. Desktop + Code only.
