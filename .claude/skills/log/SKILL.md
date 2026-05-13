---
name: log
description: Append a timestamped log entry to a channel or personal queue. Use when the user says "/log", "log this", "record that I did X", "note for the channel", or when an agent wants to write an activity trace. Designed for high-cadence, low-ceremony entries — no stamp required, no review ceremony. The autonomous journaler's primitive.
user-invocable: true
allowed-tools: [mcp__secretariat__capture]
---

# Logging

Logs are the running tape — lightweight, high-cadence, signed but not stamped. Every developer journal entry, agent activity trace, session summary, or "what happened today" note lives here.

The key distinction from other capture types:
- `/idea` → bids for future action
- `/pain` → signals something broken
- `/decision` → records a commitment, gets stamped
- `/log` → records what happened, no action required

## Scope

| Signal | Queue |
|--------|-------|
| No context ("log this") | `area:journal` personal queue |
| Org/channel named ("log for dev channel") | `channel:dev` (or named channel) in the org |
| Agent writing a session trace | `area:journal` or channel's activity queue |

## Body shape

Intentionally minimal. The timestamp is in the filename (substrate adds it). The entry should be legible at a glance in the review walker.

```md
# <What happened — past tense, one line>

- <Detail / context bullet>
- <What changed / what was produced>
- <Next step if any — but don't over-engineer it>
```

For agent-written logs, add a `source:` tag in the body:

```md
# Secretariat agent: morning digest generated

source: secretariat-agent
- 3 new envelopes in channel:project-x since yesterday
- Digest draft written to inbox:digest:morning
- No anomalies
```

## Flow

1. Determine target queue from context.
2. If channel: check it exists (don't create it). If unsure, default to `area:journal`.
3. Call `mcp__secretariat__capture`:
   - `queue`: resolved target
   - `body`: formatted entry
   - `source`: `log-skill` (or `<agent-name>` if agent-authored)
   - `org`: set if targeting an org channel
4. Confirm: *"Logged to `<queue>`."* No further ceremony.

## Agent journaler pattern

This skill is designed to be called programmatically. An agent running a duty cycle should:

1. At session end, call this skill (or the MCP `capture` tool directly) with a summary of what it did.
2. Source tag = agent's identifier (e.g. `source: digest-agent`).
3. Target = a channel the principal subscribes to (so it surfaces at review) OR `area:journal` for personal agent runs.

The daemon's `AgentSupervisor` can inject the channel dir as cwd; the agent calls capture with `queue: area:journal` (personal run) or `queue: channel:dev` + `org: <org-alias>` (channel run).

**No stamping.** Logs are ambient context, not authoritative record. The principal stamps a *decision* derived from log entries — not the logs themselves.

## Rules

- Past tense. Active voice. One-line title.
- No shaping. No GTD classification at capture time. Logs surface at review if anything there bids for action — at that point they get classified.
- No stamp, no compose, no send. Logs stay local unless the principal explicitly forwards.
- High-volume is fine. Logs are designed for N-per-day frequency without friction.
- Agents: always include `source:` tag so the review session can group by origin.
