---
name: question
description: Capture an open question addressed to a person or channel, and park it in the waiting queue. Use when the user says "/question", "I need to ask Alice about X", "open question for the team", "we need to know Y before we can proceed", or signals a blocker or dependency on an answer from someone. The reply IS the close event — no read receipts, no polling.
user-invocable: true
allowed-tools: [mcp__secretariat__compose, mcp__secretariat__capture]
---

# Capturing a question

Questions are open dependencies. They land in `inbox:waiting` until a reply arrives (the reply closes them). They can also flow as envelopes to a contact or channel.

## Scope resolution

| Signal                                 | Behavior                                                                              |
| -------------------------------------- | ------------------------------------------------------------------------------------- |
| No addressee ("open question about X") | Capture to `inbox:waiting` — self-reminder, address later                             |
| Named person ("ask Alice")             | Compose envelope to that contact; copy also lands in `inbox:waiting` as tracking stub |
| Named channel ("for the dev channel")  | Compose to that channel; copy in `inbox:waiting`                                      |

## Body shape

```md
# <Question — one line, ends with ?>

**To:** <contact alias or channel URI, or "TBD">
**Blocking:** <what decision/task this unblocks, if any>
**By:** <date needed by, if time-sensitive>

## Question

<The actual question. One paragraph max. Specific enough that the recipient can answer without a follow-up.>

## Context

<Optional. Background the recipient needs. Link to relevant envelopes by hash if useful.>
```

Show draft inline before writing. Wait for confirmation.

## Flow

1. **Draft** — fill the shape above with what the user provided. Ask if addressee or blocking context is missing and matters.
2. **Capture to `inbox:waiting`** always — this is the tracking stub. Use `mcp__secretariat__capture` with `queue: inbox:waiting`, body = the formatted question, `source: question-skill`.
3. **Compose to addressee** (if named) — `mcp__secretariat__compose` to the contact or channel. Questions are signed but NOT stamped by default — questions are informational. Stamp only if the question carries a formal commitment (e.g., a process-verbaux question that must be on record).
4. Confirm to user: _"Question parked in inbox:waiting. [Sent to <contact> via DM / posted to channel X.]"_

## At review time

The `/review` skill processes `inbox:waiting`. A question there is closed by:

- Reply arrived → link the reply envelope hash to the question stub; archive the stub
- No reply in >14d → nudge the addressee or kill the question
- Answer was found another way → archive with one-line resolution note

## Rules

- One question per capture. If there are three things to ask someone, three captures — not a list.
- Keep it specific. "What do you think about X?" is not a question worth capturing — too vague to close.
- Never send a question envelope without showing the body first.
- No auto-stamp. Questions flow signed-only unless the user explicitly requests a stamp.
- "Blocking" field is the hook for the review session — it surfaces urgency without urgency flags.
