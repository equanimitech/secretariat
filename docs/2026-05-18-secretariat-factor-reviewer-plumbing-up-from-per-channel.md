---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T140024Z-nkrwax.md
---
# Secretariat: factor reviewer plumbing up from per-channel bespoke to substrate runtime

Surfaced 2026-05-18 while discussing how to wire `journals:moments` after the pattern set by `journals:therapy`.

## Problem

The `journals:therapy` reviewer works, but it scales poorly. Each channel that wants local-cognition extraction has to ship:

- A bespoke `bin/review.py` hardcoded for that channel.
- A `.claude/agents/<name>-reviewer.md` wrapper subagent.
- A `CLAUDE.md` gate per channel.
- Bespoke `sync.sh` for cron/triggers.

Adding `journals:moments` means duplicating 80% of `journals:therapy`'s plumbing. The cognition policy ("local only") is prose in the channel description — not structurally enforced. Drift risk: each new channel could quietly diverge.

This works for 1–2 channels. It betrays the substrate ambition for 10+.

## Why now

About to scaffold `journals:moments` (daily zenborg-moments extraction via LM Studio). Resisting the bespoke route. Want the runtime to absorb the repeated plumbing so the next twenty channels are declarative.

## Seed solution

Promote the therapy plumbing one level up. Channels become declarative; runtime handles dispatch.

**Channel manifest extended** (`channel.md` frontmatter):

```yaml
handle: journals:moments
cognition: local              # local | cloud | mixed
synthesis: extraction-only    # extraction-only | composition | freeform
reviewer: moments-extractor   # references ~/.secretariat/lexicon/reviewers/
sources:
  - kind: zenborg-vault
    path: ~/.zenborg
trigger:
  kind: cron
  schedule: "0 6 * * *"
stamp: optional               # required | optional | none
```

**Runtime components**:

- `~/.secretariat/bin/run-reviewer <channel-handle>` — parses manifest, dispatches.
- `~/.secretariat/lexicon/reviewers/<name>/prompt.md` — prompt template per reviewer kind.
- `~/.secretariat/lexicon/source-handlers/<kind>.py` — pluggable: `zenborg-vault`, `supernote-journals`, future ones.
- `~/.claude/agents/secretariat-reviewer.md` — single generic dispatcher subagent.
- `~/.secretariat/CLAUDE.md` — root cognition-policy gate (replaces per-channel CLAUDE.md for the reviewer path).

**Cognition routing**:

- `cognition: local` → POST to LM Studio at `localhost:1234` (configurable).
- `cognition: cloud` → Anthropic API.
- `cognition: mixed` → refused without explicit per-call attestation (audit trail required).

**The data-bypass discipline stays the same**: source bytes go file → script → LM endpoint → file. The dispatcher subagent only sees the output filepath, never the source content. This invariant must be preserved by the runtime, not by per-channel discipline.

## Shape later

Roundtable decides:

- Manifest schema details — is `cognition: mixed` even a thing, or should mixed-cognition channels be forbidden by construction?
- Reviewer registry shape — flat directory or hierarchical? Versioned?
- Source-handler API — read-only enumeration + bytes, or richer (timestamps, metadata, structured)?
- Trigger types beyond cron — hook (SessionStart), manual-only, event-driven (file change)?
- Stamp policy — does `stamp: required` block envelope visibility until stamped, or just remind?

## No-gos

- No cloud reviewers on channels currently marked local — migration must be explicit, not inferred.
- No retro-converting therapy to the new runtime until the runtime is proven by at least one other channel.
- No hiding cognition policy. The manifest must make it visible at glance.
- No bypassing the data-bypass discipline. Subagent never sees source bytes, period.

## Open questions

- Where does the channel-specific prompt fragment live vs. the reviewer-kind prompt? E.g. `moments-extractor` reviewer might be reused across `journals:moments` and a future `journals:moments-themia` — does each channel get a prompt suffix slot?
- Does the runtime need a notion of "reviewer version" so channels can pin? Or upgrade is fine and breakage is the channel's problem?
- How does this interact with non-reviewer envelopes (peer letters, captures)? The substrate refactor is about *reviewer-produced* envelopes specifically — manual captures stay as-is.
- Does the runtime help with stamp prompting (Touch ID dialog), or is that always the principal's terminal?
- Migration path: when this lands, `journals:therapy` becomes a manifest. Who/when?

## Related

- 2026-05-18 weekly moments review session.
- `journals:moments` channel already created; intentionally not yet scaffolded with bespoke `bin/review.py`.
- Existing reference implementation: `~/.secretariat/_self/channels/journals/therapy/bin/review.py`.

## Appetite

Medium. Substrate refactor, not a feature. One-time investment that pays back on every future channel. Probably belongs as a Shape Up pitch in the secretariat repo, not zenborg.
