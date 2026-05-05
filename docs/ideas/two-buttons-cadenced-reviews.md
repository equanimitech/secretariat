# Two big-red-buttons home screen + cadenced reviews

Raw capture — 2026-05-05.

- "This looks a bit too much like regular emails... I don't like the UI at all for us, way too complicated for now."
- "I'm thinking: 2 'big red buttons' — one launches the inbox review, the other launches the outbox review. The reviews should be cadenced."
- The default surface of the app is **not** an inbox dashboard. It's a near-empty surface with two buttons:
  - **Review inbox** — enters a focused session walking through received envelopes
  - **Review outbox** — enters a focused session walking through drafts awaiting stamp
- Strategic friction (equanimitech principle 7): the principal *enters* the review session deliberately. Email's always-on inbox is the compulsive path; the two-button home is the intentional one.
- "Cadenced" — the review session itself is paced:
  - One item at a time, full screen
  - Maybe a timer ("10 min sweep")
  - Or progress tied to count ("3 of 7 envelopes")
  - Or just "next / next / done" — natural endpoint when the queue is exhausted
  - End of session returns to the two-button home
- This is the right shape for the review-session model
  (`memory/feedback_review_session_model.md`) made literal: the
  surface IS the review session entry point, not a dashboard you can
  drift in.
- Adjacent: the bubble-up idea (`docs/ideas/bubble-up-like-hey.md`) means an envelope can re-enter the queue at a future review time. The two buttons are still the only home.
- Adjacent: the multi-granularity envelopes idea
  (`docs/ideas/multi-granularity-envelopes.md`) — a cadenced review can
  show envelopes at headline-granularity by default, with "go deeper"
  to expand. Walks at the speed of attention.
- "Big red buttons" is shorthand for *unmistakable, single-purpose,
  no-decoration*. Probably not literally red — uses the app's
  existing palette — but the visual weight is "this is the only thing
  you do here."
- Questions:
  - What's on the home surface besides the two buttons? Probably the
    principal's name + avatar + small counts ("3 in inbox, 2 in
    outbox" — but only as ambient signal, not a navigation surface).
  - Should the counts show numbers or color indicators (green = empty,
    amber = stuff, red = a lot)? Ambient color matches equanimitech
    "peripheral presence" better than numerals.
  - Does the cadenced session show one envelope at a time or a paged
    list? One-at-a-time forces engagement; paged list reverts to
    email. Lean one-at-a-time.
  - Keyboard? `J/K` for next/prev, `S` to stamp, `R` to reply,
    `D` to defer (bubble-up later).
  - What replaces the current Review queue + Inbox columns? Both
    become entry points to their respective review sessions. The
    column layout disappears.
  - Settings / status — is there a third button? Or a small gear icon
    in a corner?
- This is a UI rework of `<ReviewSurface>` + a new `<ReviewSession>`
  walker component. Doesn't touch the Tauri commands underneath; same
  primitives (`list_inbox`, `list_review_queue`, `read_envelope`,
  `stamp_envelope`).
- Don't shape yet.
