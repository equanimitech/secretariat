---
name: pain
description: Capture a bug, friction, or improvement. Use when the user says "/pain", "save this bug", "this is annoying", "this is broken", "this could be better", or reports friction mid-conversation (often prefixed "bug:", "annoying:", "why does..."). Do NOT fix on the spot unless asked — capture and move on. Pain entries surface at the next /review session.
user-invocable: true
allowed-tools: [mcp__secretariat__capture]
---

# Capturing pain

## Decision tree

1. **Call `mcp__secretariat__capture`** with:
   - `queue: inbox:pain`
   - `body`: user's raw phrasing (see template below)
   - `source: pain-skill`
   - `org`: omit (personal queue)

   Done. Do not also write a file. Do not fix the pain unless the user asks.

2. **Fallback** (Secretariat MCP unavailable) → write `docs/pain/<kebab-slug>.md` with lifecycle frontmatter (see below).

## Body shape (MCP capture)

Raw. Bullets. Keep the user's phrasing — no polishing.

```
[severity: high]   # optional — only if user signaled urgency

- <what hurts / what's broken>
- <where observed / repro hint>
- Questions:
  - <open question for /review>
- Don't fix yet.
```

## Body shape (fallback)

```md
---
status: open
severity: low         # low | medium | high
created: <YYYY-MM-DD>
updated: <YYYY-MM-DD>
---

# <title>

- <what hurts>
- <repro hint>
- Don't fix yet.
```

## Rules

- One pain per capture. Second complaint → new capture.
- Keep user's exact phrasing. No user-story rewrites.
- `inbox:pain` is the default queue. GTD classification (next-action / someday / wontfix) happens at `/review` time.
- Never add root-cause / fix / acceptance-criteria sections — that's shaping work.
- Severity optional. Omit if user didn't signal urgency.
