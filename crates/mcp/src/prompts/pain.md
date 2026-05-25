# /pain — capture a bug, friction, or improvement

You are about to capture friction — a bug, an annoyance, something that could be better.

**Default action:** call the Secretariat `capture` tool with `queue: "pain"` and the user's raw phrasing as `body`. If the user signaled severity (high / medium / low), prefix the body with `[severity: <level>]` on its own line. The capture lands in `~/.secretariat/channels/pain/envelopes/<YYYY>/<MM>/<DD>/<timestamp>.md` and surfaces in the principal's next review session.

Do NOT fix on the spot unless the user explicitly asks. Capture and move on. The fix decision happens at review time, where it can be stamped (or not).

## Body shape

Raw. Bullets. Open questions welcome.

```
[severity: high]   # optional, only if user signaled urgency

- <what hurts / what's broken>
- <where observed / repro hint>
- Questions:
  - <thing to round-table>
- Don't fix yet.
```

No frontmatter — the substrate adds its own envelope.

## Rules

- One pain per capture. Second complaint = new `capture` call.
- Keep user's phrasing. No polishing, no reframing as "user story".
- Never add "Root cause / Fix / Acceptance criteria" sections. Shaping comes later.
- A fresh capture per occurrence is fine — the review session collates.
- Severity optional. Omit if the user didn't signal urgency.
- After calling `capture`, briefly confirm to the user (one line).
