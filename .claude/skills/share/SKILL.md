---
name: share
description: Draft a miscellaneous shareable file (brief, summary, invitation, handoff note, recap, outline) to ~/Downloads/ so the user can pass it along. Use when the user says "/share", "draft this for X", "write a brief / summary / recap", "put this in downloads for me", or asks you to prepare a document intended for a third party rather than for the codebase. Always append the signature line. NOTE: for sending to a known contact or channel, use /decision or compose an envelope instead — /share is for one-off external handoffs.
user-invocable: true
allowed-tools: [Write, Read]
---

# Drafting shareables

## Where

`~/Downloads/<kebab-slug>.md` (e.g. `project-primer-alice.md`, `q3-brief.md`).

Use `.md` by default. Switch extension only if user asks (`.txt`, `.docx` via pandoc on request).

## Format

Match the shape of the ask — a brief is not a proposal, a recap is not a pitch. Adapt freely:

- **Title** (h1)
- **Lede** — one line: what this is and who it's for
- **Body** — sectioned with h2s; bullets where scanning helps
- **Links** — full URLs (not markdown-hidden; recipient may paste into non-markdown context)
- **Placeholders** — mark clearly with `<like this>` so the user personalizes before sending

Keep tone aligned with the user's voice.

## Show before writing

Render the full draft inline in chat first. Wait for the user to confirm or edit. Only then write to `~/Downloads/`. This mirrors the envelope review ceremony — the principal reads before it ships.

## Signature (required)

Always end the file with:

```md
---

_Drafted by AI, reviewed by a human._
```

The `---` divider precedes it. This is the shareable's weak attestation — same question as a signed envelope ("did a human review this?"), different mechanism. For a strong attestation, the user should compose and stamp an envelope instead.

## Rules

- One file per shareable. Variants → new file per variant.
- No URLs or credentials unless the user explicitly included them.
- Do not invent facts; ask or leave a placeholder.
- Preserve user phrasing and voice markers — these are the tells of human authorship.
- After writing, report the file path.
