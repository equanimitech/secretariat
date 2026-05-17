---
name: idea
description: Capture a raw idea — a product thought, a fleeting note, anything worth keeping. Use when the user says "/idea", "/ideas", "save this idea", "capture this", or floats a thought mid-conversation (often prefixed "idea:", "what if", or "I wonder"). Do NOT shape into a pitch — that's for /review.
user-invocable: true
allowed-tools: [mcp__secretariat__capture, mcp__secretariat__list_channels, Read, Bash]
---

# Capturing ideas

## Routing — infer before defaulting

Before calling `capture`, decide WHERE the idea lands.

1. **Check for `.secretariat` in the repo.** From the current working dir, walk upward until you find a `.secretariat` file or hit the repo root. TOML format expected:

   ```toml
   org = "your-org-alias"

   [channels]
   idea = "channel:your-channel"
   pain = "channel:your-channel:pain"
   ```

   Read it with the `Read` tool.

2. **If `.secretariat` provides an `idea` channel that is NOT `inbox:triage`:** propose to the user —

   > "Found `.secretariat` → capture to **`<org>` / `<channel:handle>`**? (y · `inbox:triage` · other)"

   Wait for their answer.

3. **If the inferred channel IS `inbox:triage` (or there is no inference):** skip the prompt. Capture silently to `inbox:triage`. Defaults stay quiet — only divergences need a confirmation beat.

4. **Verify the channel exists** before capturing a non-default route — call `mcp__secretariat__list_channels` (scoped to the inferred org). If the handle isn't present, surface the mismatch and fall back to `inbox:triage`, asking whether to create the channel. Never create channels on the fly inside `/idea`.

5. **Apply the decision:**
   - User confirms (`y` / `ok`) → `capture` with `org: <alias>`, `queue: <channel:handle>`.
   - User overrides with another handle → use that.
   - User declines, or no `.secretariat` exists, or inferred channel IS the default → `capture` with `queue: "inbox:triage"`, no `org`.

## Body shape

Raw. Bullets. Keep the user's phrasing — don't polish into marketing copy.

```md
# <title>

- <observation / hunch>
- <adjacent angle>
- Questions:
  - <thing to round-table>
- Don't shape yet.
```

This goes into `body` verbatim. Frontmatter is added by the substrate.

## Fallback

If Secretariat MCP is unavailable, write `docs/ideas/<kebab-slug>.md` with the body above. Append to an existing file if one matches the topic.

## Rules

- One idea per capture. Second riff → new capture.
- Never add Problem / Solution / Acceptance Criteria sections.
- `inbox:triage` is the default queue when no `.secretariat` inference is available, the inference points at `inbox:triage` itself, or the user declines a non-default suggestion.
- Confirm inferred routing BEFORE capturing **only when the route is non-default**. Defaults are silent; non-defaults need a beat.
- Do NOT create channels on the fly. If the inferred channel doesn't exist, fall back + surface the mismatch.
- After capture, confirm with the principal in one line — the substrate-returned file path is enough.
