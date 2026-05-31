---
migrated_from: equanimi.tech/project/secretariat/dev/20260525T130208Z-g3et5s.md
---

# Pitch — channel-local `docs/` slot for read-for-context material

Pitch — 2026-05-25. Tag: `pitch`. Source: live conversation w/ Rafa, 2026-05-25 (Themia compta provider-registry thread).

## Pitch

### Problem

Channels accumulate reference material that isn't envelope traffic and isn't consumption prefs: provider registries (vendor list driving an invoice sweep), runbooks (monthly close steps), account-portal indexes, glossaries, agent prompts scoped to the channel's bounded context.

Today this material has no home. It leaks into `channel.md` (bloats the channel description), scatters at the channel root next to `contract.local.md` (mixes governance with reference), or gets shoved into envelopes (fails the envelope test — no author, no recipient, no attestation moment).

The triggering case: a `providers.md` registry for Themia compta. Drives a Gmail-MCP-fed invoice download into `~/Documents/invoices/themia/`. Mutates freely as providers come and go. Not correspondence — it's a config artifact the channel's agents read.

### The bet

Sanction `<channel>/docs/` by **convention, not protocol**. Any channel that wants reference material gets a `docs/` folder. Free-form markdown. Daemon ignores it (only watches the `_drafts/` → `envelopes/` pipeline per the post-fe359eb substrate). Claude reads it for context by virtue of the channel dir being a Claude Code project (invariant #8).

Zero protocol surface: no lexicon record, no contract field, no CLI verb, no daemon change. One-line addition to AGENTS.md under the channel-dir-layout description, plus the rule-of-thumb:

> If it's read-for-context and mutates freely → `docs/`.
> If it's a moment-in-time claim by an author → envelope.
> If it's preferences for *you* consuming the channel → `contract.local.md`.

Practice-as-gate per the lexicon hard rule's spirit. Codify only if a second use case appears.

### No-gos

* No new lexicon record type for "doc".

* No CLI verb (`sec doc add` etc.). `mkdir` + write is the interface.

* No sync of `docs/` content as wire traffic. Strictly local-to-the-subscriber by default — same posture as `contract.local.md`.

* No schema. Doc files are free-form markdown; no frontmatter required.

* No retroactive migration sweep. Existing channels keep what they have; `docs/` is opt-in per channel.

## Boundaries

### Job to be done

When I'm working in a channel and need reference material the channel's agents (or future-me) will repeatedly read — provider list, runbook, account portals, glossary — I want a sanctioned slot so the file has a predictable home, my agents know where to look, and I'm not cluttering the channel root or bloating `channel.md`.

Baseline today: I either stuff it into `channel.md`, scatter `.md` files at the channel root next to protocol files, or wrongly shape it as an envelope.

### Appetite

`tiny`. One-paragraph AGENTS.md edit + seed one example (`ops/compta/docs/providers.md`). Appetite picked: tiny — convention adoption, no code, no protocol. Override with `--appetite=<size>` if shaping a richer slot semantics design.

## Elements

* **Place:** `<channel-dir>/docs/` — new sanctioned subdir, peer to `envelopes/` and `_drafts/`.

* **Affordance:** any markdown file, free-form, no schema. Author writes with `Write` tool or `mkdir + $EDITOR`.

* **Connection:** daemon ignores (verify against current `OutboxWatcher` scope — only watches `_drafts/` post-fe359eb). Claude reads via tree-walk inheritance when `cd <channel-dir> && claude` or when an agent is launched with channel dir as cwd.

* **Documentation:** AGENTS.md gets one block under the architectural-invariants section showing the channel-dir tree with `docs/` and the three-way rule.

## Risks

### 🐇 Rabbit holes

* **Tree-walk reach.** Invariant #8 says channel dir = Claude Code project with `.claude/{agents,skills,commands}/` tree-walk inheritance org→dept→leaf. Does that walk pick up arbitrary subdirs (`docs/`) or only `.claude/`? Verify in `src-tauri/` or wherever the launch wiring lives. If only `.claude/`, the convention needs `<channel>/.claude/docs/` instead — uglier but correct.

* **Tooling drift.** If future tools assume the channel dir contains only `{channel.md, contract*.md, envelopes/, _drafts/}`, adding `docs/` might break enumeration. Grep for any code that lists the channel dir.

### 🏴 Off-sides called

* Codifying `docs/` in the lexicon. Tempting because "make it official"; wrong because convention-first is the AGENTS.md hard rule for record shapes — same spirit applies to subdirs. Cut.

* Sanctioning more slots in the same pitch (`templates/`, `prompts/`, `scripts/`). Resist scope creep. One slot, one driver (compta/providers.md). Add more only when a real second driver appears.

* Building a `sec doc` CLI surface. Not principal-facing as a *primitive* — it's just a file. Skip the four-surface ceremony from AGENTS.md.

### 🥩 Fat cut

* Frontmatter schema for `docs/*.md` (`---\ntype: registry\nowner: rafa\n---`). Not needed. Free-form markdown reads fine for both humans and agents.

* Auto-index file (`docs/README.md`) listing contents. `ls` exists.

* Sync gate (mark some `docs/` files as shared with roster). Premature — same posture as `contract.local.md` covers it: local-only by default.

### 🧪 Domain knowledge

* Confirm `OutboxWatcher` (or whatever replaced it post-fe359eb) only watches `_drafts/` and never picks up arbitrary subdirs. Skim `crates/core/src/infrastructure/` for the watch scope. If it walks the whole channel dir, `docs/` might trigger spurious daemon work.

* Check whether `sec list` / channel-explorer UI enumerates arbitrary subdirs or hard-codes the known slots. If it enumerates, `docs/` will surface in the UI — possibly fine, possibly noisy.

* Verify the AGENTS.md edit doesn't conflict with the substrate shift described in invariant #8 — `docs/` should slot in naturally as "channel dir is a Claude Code project, here's another conventional subdir."

