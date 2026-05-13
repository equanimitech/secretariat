---
name: idea
description: Capture a raw idea — a product thought, a fleeting note, anything worth keeping. Use when the user says "/idea", "/ideas", "save this idea", "capture this", or floats a thought mid-conversation (often prefixed "idea:", "what if", or "I wonder"). Do NOT shape into a pitch — that's for /review.
user-invocable: true
allowed-tools: [mcp__secretariat__capture]
---

# Capturing ideas

## Decision tree

1. **Call `mcp__secretariat__capture`** with:
   - `queue: inbox:triage`
   - `body`: user's raw phrasing (see template below)
   - `source: idea-skill`
   - `org`: omit for personal captures; set org alias if user signals a specific project/org context (e.g. "idea for acme" → `org: acme.com`)

   Done. Do not also write a file. Do not shape the idea.

2. **Fallback** (Secretariat MCP unavailable) → write `docs/ideas/<kebab-slug>.md` with the body below.

## Body shape

Raw. Bullets. Keep user's phrasing — don't polish into marketing copy.

```md
# <title>

- <observation / hunch>
- <adjacent angle>
- Questions:
  - <thing to round-table>
- Don't shape yet.
```

For MCP captures, this goes into `body` verbatim. Frontmatter is added by the substrate.

## Rules

- One idea per capture. Second riff → new capture.
- Never add Problem / Solution / Acceptance Criteria sections.
- `inbox:triage` is the default queue. The async contextification pass may re-file it; GTD classification happens at `/review` time.
- Do NOT create channels. If the user names an org context, pass `org` to route to the right channel tree — never create channels on the fly.
- For fallback: if an idea file exists on that topic, append rather than creating a duplicate.
