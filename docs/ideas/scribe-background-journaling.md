# Scribe should journal in the background

Raw capture — 2026-05-05.

- "Scribe should journal in the background or autogenerate proposed communications (not necessarily addressed)."
- The scribe (Claude / whichever AI assistant the principal uses) keeps observing the principal's day — files touched, calendar events, conversations, ambient context — and continuously drafts:
  - **Journal entries** — internal-facing, never sent to anyone. Captured thoughts, summaries of the day, reflections.
  - **Unaddressed drafts** — _"this might be worth saying to someone"_ but not yet routed. Sit in a "could-be-sent" pile.
  - **Pre-addressed but unstamped drafts** — same as today's outbox queue, just produced continuously instead of on-demand.
- The principal's review session shifts: instead of "open queue, see drafts the assistant made on demand," it's "open queue, see what the scribe noticed today, decide what (if anything) to send."
- This makes Secretariat a _capture-and-curate_ surface, not a request-response surface. The scribe is always running; the principal is always selecting.
- Adjacent: feels like Hey-style "Imbox" but inverted — instead of curating incoming, you curate outgoing.
- Questions:
  - Where do unaddressed drafts live? Same outbox? New "proposals" folder? Their own collection in the review surface?
  - How does the scribe know what's worth journaling vs what's noise? User-configurable signal/instruction set, or learned from past acceptances/rejections?
  - Privacy: the scribe needs to read the principal's ambient context (files, calendar, etc.). What's in scope? Configurable per-source toggles.
  - Cost: continuous AI inference is expensive. Is this BYOK only? Local model? Batch overnight?
  - Does this make Secretariat into "a writing tool that sometimes sends things" rather than "a messaging tool"? Worth being deliberate about.
- Don't shape yet.
