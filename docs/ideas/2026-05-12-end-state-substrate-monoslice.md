# End-state substrate in one slice

**Date:** 2026-05-12
**Tags:** `equanimitech/secretariat`, v0.3, substrate, shipping
**Status:** idea / alternative shipping path — captured during slice 1 shaping

---

## The pitch in one paragraph

Instead of rolling out the v0.3 substrate as a sequence of additive
slices (passport-home now, workspaces later, agents later, sync later),
ship the full end-state substrate in one larger slice — passport home +
workspace registry + repo-local `.secretariat/` discovery + channel-tree
merging + initial Secretariat-agent loop. Higher blast radius, slower to
ship, but no intermediate "the substrate is half-done" period and
every later slice (sync, contracts, attention-routing) builds on a
single coherent foundation rather than a series of compromises.

---

## What's in the monoslice

1. **Passport-home substrate** (slice 1 today's scope) — `<handle>/key` +
   `.identity` cross-check, `channel/`, `queues/`, capture routing.
2. **Multi-passport detection** — scan `*/key`, `current` pointer for
   default-persona selection, `--as <handle>` CLI override.
3. **Workspace registry** — `.secretariat/` as a repo-local marker,
   `sec workspace register/list/unregister`, upward-walk discovery,
   `workspace.json` for owner-DID + name + registered channels.
4. **Channel-tree merging** — `sec review` and MCP review resource walk
   passport home + every registered workspace, conflict rule (workspace
   wins for same-handle).
5. **Skill/agent inheritance** — Claude Code's existing `.claude/` walk
   gets the merged tree (workspace `.claude/` overrides passport-home
   `.claude/` for the same channel).
6. **Migration of existing v0.2.x installs** — `sec migrate` script that
   moves `key`/`did`/`profile.json`/`inbox/`/`outbox/`/`queues/` from
   root into `<handle>/` and updates references.
7. **Initial Secretariat agent loop** — daemon-launched per-channel
   agent (Claude Agent SDK) that reads subscribed channels, drafts
   digests, signs with own DID, drops in passport-home digest queue.

Anything past that (sync, relay subscriptions, contracts as runtime
constructs, attention-routing) stays out — those are real future
slices.

---

## Tradeoff vs the slice-by-slice path (the recommended A path)

| Dimension | Monoslice | Slice-by-slice |
|---|---|---|
| Time to demoable capture | ~3-5 days | ~3 hours |
| Time to multi-principal use | ~5 days | ~3-4 weeks |
| Refactor risk if substrate shape needs to change | Lower (changes happen in one breath, before anyone depends on it) | Higher (each slice adds dependents on the shape; revisiting hurts) |
| Bug surface in slice 1 | Wider — workspace + agent + merging logic ships together | Tight — single concept per slice |
| Confidence we've designed the right end-state | Lower until shipped (can't course-correct mid-slice) | Higher (each slice has a real demo and feedback before the next) |
| Recursive validation with the book | Better (one coherent system to build the book against) | Slightly worse (the book co-authored on a half-built substrate for a few weeks) |

---

## Why this is interesting

The slice-by-slice path is the recommended-A discipline — small bites,
clear demos, each slice ships a measurable thing. But the cost is that
slices 2-N rest on the slice-1 shape; if slice 4 reveals the shape was
wrong, slice 1's choices become technical debt that infects every later
slice.

The monoslice trades up-front time for compositional cleanliness.
Everything is designed once against the full target end-state. The
substrate's invariants (one passport per machine until multi-passport
is intentional, one workspace = one publishable identity, channel-tree
merges by handle) all get exercised together in one ship, and any
mis-design surfaces immediately rather than 6 weeks later.

---

## When this becomes the right call

Not yet. The slice-by-slice path is correct *because*:

- Slice 1 today validates the bare passport-home layout against a real
  capture loop. We don't yet know if `<handle>/key` detection feels
  right in practice.
- Workspaces (idea B) introduce repo-local `.secretariat/` discovery —
  a substantial UX surface that deserves its own pitch with risks +
  no-gos, not absorption into slice 1.
- Agents (Secretariat-agent loop) need a CognitionPort impl that's
  actually well-shaped (current adapter is BYOK-only, sketch quality)
  and a daemon-cron mechanism that's not yet there.

Revisit if:

- Slice 1 + 2 ship cleanly and the per-slice cadence feels like
  bureaucratic over-shaping rather than discipline.
- Workspaces and agent-loop end up tightly coupled in design (e.g. the
  agent needs to know about workspaces from day one) — bundling becomes
  cheaper than separation.
- An external deadline (book release, Themia milestone) demands one
  big-bang substrate ship instead of a 6-week trickle.

---

## No-gos

Even in monoslice form, the following stay out:

- Sync daemon + relay subscriptions (real network protocol — separate
  ship).
- Contracts as runtime constructs (Marcelo-Rafa contract evaluation).
- Attention-routing daemon (proposal × bounds × per-channel contract
  evaluation).
- Multi-passport switching UX (`current` pointer ships, but the actual
  cross-passport correspondence is a future story).
- Cross-channel global ordering (explicit substrate non-goal per
  AGENTS.md).
