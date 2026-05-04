---
status: open
severity: high
created: 2026-05-04
updated: 2026-05-04
---

# First message not working — boilerplates don't exist yet

Raw capture — 2026-05-04.

- First message to dad didn't go through cleanly because **boilerplates don't currently exist**.
- The "first message" experience matters most — it's the T2FM (time to first message) demo for every new principal.
- Symptoms observed today (Marcelo onboarding):
  - Composing required hand-edited markdown body (template scaffolds, no boilerplates for common message types).
  - "Hello world" / "first contact" / "received-confirmation" / "reply" — none have a canonical shape to drop into.
  - Without boilerplates, every first message becomes a bespoke composition, which kills both T2FM and the natural-language "send a message saying X" UX.
- Where observed: composing dad's first envelope today, plus dad's expected reply path.
- Questions:
  - What boilerplates does v0.2 need? (first-contact, ack/received, status-update, reply-with-context?)
  - Should boilerplates live in `~/.secretariat/boilerplates/` (user-customizable, like `template.md`)?
  - Should compose accept `--boilerplate <name>` and the MCP `compose` tool surface a `boilerplate` enum?
  - Should the AG template be reframed as one of N boilerplates rather than the only one?
- Don't fix yet.
