# /idea — capture a raw idea

You are about to capture a raw idea — a product thought, a fleeting note, anything worth keeping.

Do NOT shape into a Shape Up pitch or PRD — that's `/shaping` or `/roundtable`.

## Routing — infer before defaulting

Before calling `capture`, decide WHERE the idea lands. Sequence:

1. **Look for a repo-local `.secretariat` file** in the current working directory (walking upward to the repo root). If present, parse it. Expected format (TOML):

   ```toml
   org = "your-org-alias"

   [channels]
   idea = "channel:your-channel"
   pain = "channel:your-channel:pain"
   # optional further routing
   ```

2. **If `.secretariat` provides an `idea` channel that is NOT `inbox:triage`:** propose it as a one-line confirm prompt — *"Capture to `<org>` / `<channel:handle>`? (y · `inbox:triage` · other)"* — and WAIT for the principal's answer.

3. **If the inferred channel IS `inbox:triage` (or no inference at all):** skip the confirm prompt. Just capture silently to `inbox:triage`. Defaults stay quiet — only divergences from default need a confirmation.

4. **If the principal confirms (`y` / `ok`):** call `capture` with `org: <alias>`, `queue: <channel:handle>`.

5. **If the principal overrides with another handle:** use that. If they say `inbox:triage` or decline: fall back to default.

6. **Default (no `.secretariat`, no inference, or principal declines):** `capture` with `queue: "inbox:triage"`, no `org`. Lands in personal triage.

The confirm step only fires when a non-default route is proposed. Defaults are silent; divergences are intentional and need a beat of attention.

## Body shape

Raw. Bullets. Open questions welcome. Keep the user's phrasing — don't polish into marketing copy.

```md
# <title>

Raw capture — <YYYY-MM-DD>.

- <observation / hunch>
- <adjacent angle>
- Questions:
  - <thing to round-table>
- Don't shape yet.
```

This body goes into the `body` param of the `capture` tool verbatim. The substrate adds frontmatter — you don't.

## Rules

- One idea per capture. If the user riffs on a second thing, new `capture` call.
- Never add "Problem / Solution / Acceptance criteria" sections. That's shaping.
- If the user is clearly riffing on an already-captured topic, prefer a new capture (the review session collates) rather than trying to find + edit a prior one.
- Confirm inferred routing BEFORE capturing **only when the route is non-default**. Defaults are silent; non-defaults need a beat. A wrong-channel capture is friction the principal pays for later.
- After calling `capture`, briefly confirm to the user (one line) — file path the substrate returned is enough.
- Do NOT create channels on the fly. If the inferred channel doesn't exist, fall back to `inbox:triage` and surface that mismatch ("`.secretariat` points at `channel:jurimetria` but that doesn't exist yet — captured to `inbox:triage` instead. Create the channel?").
