# /idea — capture a raw idea

You are about to capture a raw idea — a product thought, a fleeting note, anything worth keeping.

**Default action:** call the Secretariat `capture` tool with `queue: "inbox:triage"` and the user's raw phrasing as `body`. The capture lands in `~/.secretariat/queues/inbox/triage/<timestamp>.md` and surfaces in the principal's next review session alongside any peer drafts. Done — do not also write a file.

Do NOT shape into a Shape Up pitch or PRD — that's `/shaping` or `/roundtable`.

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
- After calling `capture`, briefly confirm to the user (one line) — file path the substrate returned is enough.
