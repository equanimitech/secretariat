---
name: review
description: Review outstanding captures across Secretariat queues — triage ideas and pains, classify by GTD outcome, shape pitches for "now" items. Scoped per org, channel, or thematic. Use when the user says "/review", "let's review", "review the backlog", "what should we work on next", or names a specific org/project ("review acme backlog", "review dev captures"). Companion to /idea and /pain.
user-invocable: true
allowed-tools: [mcp__secretariat__read, mcp__secretariat__read_channel, mcp__secretariat__list_channels, mcp__secretariat__list_orgs, mcp__secretariat__capture, Read, Bash, Agent]
---

# Review

Cross-queue triage for Secretariat captures. Aggregate, classify, decide, shape.

## Scope resolution

Parse the user's invocation for scope signals:

| Signal | Scope |
|--------|-------|
| `/review` (no args) | personal queues: `inbox:triage` + `inbox:pain` |
| `/review --org <alias>` | org's channel tree (all channels in that org) |
| `/review --scope <handle>` | specific channel handle (`channel:secretariat:dev`) |
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

End with: *"Confirm or edit before I act."*

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
> 1. Read surrounding code referenced in the capture.
> 2. Run `git log --oneline -50`; cross-reference in-flight commits.
> 3. Write pitch to `docs/pitches/<slug>.md` using four sections: `## Boundaries` (JBTD + Appetite) → `## Elements` → `## Risks` (Rabbit holes · Off-sides · Fat cut · Domain knowledge) → `## Pitch` (Problem + Bet + No-gos).
>
> Appetite scale: `tiny` (≤2h) · `small` (1 day) · `medium` (2-3 days) · `big` (1 week).
>
> Return: pitch path + 2-line summary.

Cap at **3 project items** dispatched in parallel. If >3, push back: "That's a lot. Cut to 3?"

## Rules

- Never re-classify without user input — first pass is a proposal.
- Never delete capture files. Trash → archive/. Pain → wontfix status.
- Do NOT create channels. Scope is resolved from existing channels only.
- Cross-scope dedupe: if two captures describe the same thing, flag and let user pick canonical.
- Expand `~` before all filesystem ops.
- `inbox:waiting` and `inbox:tickler` live in Secretariat queues, not local files.
