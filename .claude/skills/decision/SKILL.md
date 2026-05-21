---
name: decision
description: Record a decision with its rationale and guide it through the stamp ceremony. Use when the user says "/decision", "record this decision", "we decided X", "the call is Y", or signals a commitment being made. This is the canonical use of the stamp — decisions are what the principal attests to. DO guide the stamp ceremony. DO ask for rationale if missing.
user-invocable: true
allowed-tools:
  [
    mcp__secretariat__compose,
    mcp__secretariat__stamp,
    mcp__secretariat__capture,
    Read,
  ]
---

# Recording a decision

Decisions are the authoritative ledger of the org. A decision envelope is what gets stamped — the principal's Touch ID is the record that a human vouched for it.

## When to use

- A call was made: architectural, strategic, product, operational
- The principal says "we decided", "the call is", "I'm going with", "record that we chose"
- End of a shaping session — the selected pitch is a decision
- A direction was rejected — that's also a decision worth recording

## Scope

Two modes depending on context:

| Signal                                           | Where it lands                      |
| ------------------------------------------------ | ----------------------------------- |
| Personal / no org context                        | `area:decisions` local queue        |
| Org/channel named ("for acme", "in dev channel") | compose as envelope to that channel |

Ask if unclear: _"Is this a personal record or should it go to a specific channel?"_

## Flow

### 1. Collect the decision

If the user gave the full context, proceed. If not, ask for:

- **The decision itself** (one sentence, active voice)
- **Rationale** (why this over alternatives — at minimum one line)
- **Alternatives considered** (optional but valuable — what was rejected)
- **Who's affected** (optional — relevant principals)

Do NOT proceed to compose without at least the decision sentence + one line of rationale. A decision with no rationale is just noise.

### 2. Draft the body

Use this shape:

```md
# <Decision title — imperative, one line>

**Date:** <YYYY-MM-DD>
**Context:** <channel or area, e.g. `secretariat#channel:dev` or `personal`>

## Decision

<One paragraph, active voice. What was decided and by whom if multi-party.>

## Rationale

<Why this call. What problem it solves. What alternative was rejected and why.>

## Consequences

<Optional. What changes as a result. What's now off the table. Future triggers.>
```

Show the full draft inline before writing anything. Wait for the user to confirm or edit.

### 3. Write and stamp

**Personal decision** → `mcp__secretariat__capture` to `area:decisions`, then offer to stamp:

> "Decision recorded. Stamp it to make it authoritative? (Touch ID)"

**Channel decision** → `mcp__secretariat__compose` to the channel. Then stamp ceremony:

1. Show the full body verbatim (already done above — confirm nothing changed).
2. Ask: _"Stamp this decision? (Touch ID will be required)"_
3. On yes → `mcp__secretariat__stamp`.

**Multi-party decisions** are counter-stamped (v0.4 feature). For now, note in the body who else should attest; they'll stamp when the envelope reaches them.

## Rules

- Never stamp without explicit consent in this same turn.
- Never fabricate rationale. If the user doesn't provide it, ask.
- Decisions are immutable once stamped — don't compose a "correction", compose a new decision that supersedes it (reference the old envelope hash in the body).
- Keep the title imperative and specific: "Adopt owner-as-sequencer model" not "Decision about sequencing".
- Stamp = the principal vouches. If the decision is still tentative ("we're leaning toward X"), capture as idea instead — not a decision yet.
