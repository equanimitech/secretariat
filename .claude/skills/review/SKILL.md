---
name: review
description: Review outstanding captures across Secretariat queues — triage ideas and pains, classify by GTD outcome, shape pitches for "now" items. Scoped per org, channel, or thematic. Use when the user says "/review", "let's review", "review the backlog", "what should we work on next", or names a specific org/project ("review acme backlog", "review dev captures"). Companion to /idea and /pain.
user-invocable: true
allowed-tools:
  [
    mcp__secretariat__read,
    mcp__secretariat__read_channel,
    mcp__secretariat__list_channels,
    mcp__secretariat__list_orgs,
    mcp__secretariat__capture,
    Read,
    Bash,
    Agent,
  ]
---

# Review

Cross-queue triage for Secretariat captures. Aggregate, classify, decide, shape.

## Scope resolution

Parse the user's invocation for scope signals:

| Signal                       | Scope                                                                                                                  |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `/review` (no args)          | personal queues: `inbox:triage` + `inbox:pain`                                                                         |
| `/review --org <alias>`      | org's channel tree (all channels in that org)                                                                          |
| `/review --scope <handle>`   | specific channel handle (`channel:secretariat:dev`)                                                                    |
| `/review <natural language>` | match org alias or channel handle by name (`"themia"` → `themia.pro`, `"secretariat dev"` → `channel:secretariat:dev`) |

Multiple scopes can be combined: `/review --org themia.pro --org equanimi.tech`.

## Queue sources per scope

**Personal (default):**

- `inbox:triage` — uncategorized ideas
- `inbox:pain` — bugs and friction
- `inbox:waiting` — delegated, awaiting response
- `inbox:tickler` — time-deferred items

**Org scope:** Use `mcp__secretariat__list_channels` for the org, then read each channel's `outbox/` queue (principal's drafts awaiting review).

**Channel scope:** Read that channel's outbox + any captures addressed to it.

Read captured envelopes via `mcp__secretariat__read` or direct file walk under `~/.secretariat/`.

## Flow

### 1. Inventory

Walk queues per scope. Build a flat list:

```
[<queue>] <timestamp> — <title-or-first-line> — <one-line gist>
```

Show per-queue count summary first (e.g. `inbox:triage: 4 · inbox:pain: 2 · channel:secretariat:dev: 1`).

**Shipped check** — for `inbox:triage` items, run `git log --all --oneline --since="3 months ago"` and grep for slug keywords. Flag **potentially shipped** items; confirm with principal before archiving.

### 2. Classify (GTD)

Walk the decision tree per item:

```
ACTIONABLE?
├─ NO
│   ├─ trash      — duplicate / wrong itch / violates principles
│   ├─ reference  — useful info, not a task
│   ├─ someday    — soak; defer without date
│   └─ tickler    — defer with revisit date → capture to inbox:tickler
└─ YES
    ├─ <2min       — do it in-session, then archive
    ├─ delegate    — waiting → capture to inbox:waiting
    ├─ next-action — single step → Linear issue or concrete task
    └─ project     — multi-step → Shape Up pitch
```

Apply the **<2min rule** before classifying. Answer it in-session; don't bucket what you can resolve now.

### 3. Present outcomes

Group by scope → then outcome (`### next-action`, `### project`, `### someday`, `### tickler`, `### waiting`, `### trash`).

One line per item — Smart Brevity: `**<lead>.** <why ≤8 words>. <outcome reason ≤6 words>.`

Cross-scope dedupes in a final `## Flagged` section.

End with: _"Confirm or edit before I act."_

### 4. Wait for confirmation

Never archive, trash, or dispatch shaping subagents until the principal confirms. User may move items between outcomes.

### 5. Act on confirmed outcomes

- **trash** → move capture file to `<queue-dir>/archive/` (create if missing)
- **someday** → leave in `inbox:triage` with `gtd: someday` tag appended to body, OR capture to a `someday:` queue
- **tickler** → `mcp__secretariat__capture` to `inbox:tickler` with revisit date in body
- **waiting** → `mcp__secretariat__capture` to `inbox:waiting` with who/what/by-when
- **next-action** → suggest creating a Linear issue or concrete task; don't create without user say-so
- **project** → dispatch `Agent` subagent to shape a Shape Up pitch (see "Pitch subagent" below)

### 6. Weekly review checklist (closing step)

- [ ] **Inbox to zero** — every triage + pain capture has a GTD outcome
- [ ] **Next actions** — in-flight tasks still relevant?
- [ ] **Projects** — active pitches have ≥1 next-action ticket
- [ ] **Waiting** — process `inbox:waiting`; stale >14d → nudge or kill
- [ ] **Someday** — any candidate to activate this cycle?
- [ ] **Tickler** — items where revisit date ≤ today → back to inbox

## Pitch subagent

For `project`-classified items, dispatch a fresh `Agent` with this prompt (self-contained):

> Shape a Shape Up pitch for: `<body of the captured item>`.
>
> Project root: `<absolute path to secretariat repo or relevant org dir>`.
>
> Steps:
>
> 1. Read surrounding code referenced in the capture.
> 2. Run `git log --oneline -50`; cross-reference in-flight commits.
> 3. Write pitch to `docs/pitches/<slug>.md` using four sections: `## Boundaries` (JBTD + Appetite) → `## Elements` → `## Risks` (Rabbit holes · Off-sides · Fat cut · Domain knowledge) → `## Pitch` (Problem + Bet + No-gos).
>
> Appetite scale: `tiny` (≤2h) · `small` (1 day) · `medium` (2-3 days) · `big` (1 week).
>
> Return: pitch path + 2-line summary.

Cap at **3 project items** dispatched in parallel. If >3, push back: "That's a lot. Cut to 3?"

## Presentation cadence (walk-through review)

When walking a cluster of envelopes one batch at a time (rather than the bulk "present outcomes" step):

- **Batch size = max 3 envelopes per turn.** Larger batches saturate decision load.
- **Pyramid principle per envelope: lead claim + exactly 3 supporting bullets.** Top-down. Lead with the conclusion as one sentence; back with three bullets. No more, no less unless the envelope is genuinely empty.
- **Always propose a triage action per envelope.** Don't just describe — lean an opinion: `archive` / `route → <channel>` / `merge into <existing capture id>` / `capture as culture note in <channel>`. The principal yes/no/overrides.
- **Save a run log per session** as captures in `channel:journals:reviews` (personal channel, created on first use). **Append a new envelope per batch or sub-cluster** — envelopes are immutable on disk, so don't rewrite the previous log envelope; add a new one with the cumulative or batch-scoped state. Schema per row: envelope id, title, action taken, destination (channel handle + new envelope id if routed). Lets the session be audited and resumed. Do NOT use external file paths like `~/.secretariat/run-logs/` — run logs are first-class captures, not loose files.

When the user invokes the bulk "present outcomes" flow (step 3), the original Smart Brevity one-liner format applies — the Pyramid cadence is for envelope-by-envelope walks.

## Rules

- Never re-classify without user input — first pass is a proposal.
- Never delete capture files. Trash → archive/. Pain → wontfix status.
- Do NOT create channels unprompted. If routing requires a channel that doesn't exist, propose creation and wait for go-ahead before calling `create_channel`.
- Cross-scope dedupe: if two captures describe the same thing, flag and let user pick canonical.
- Expand `~` before all filesystem ops.
- `inbox:waiting` and `inbox:tickler` live in Secretariat queues, not local files.
- Queue archive lands in `<queue-dir>/archived/` (e.g. `~/.secretariat/queues/inbox/triage/archived/`), not in the `inbox/` flat dir.
