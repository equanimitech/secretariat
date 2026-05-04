# Hide IDs entirely + borrow features from other communication media

Raw capture — 2026-05-05.

- "We are showing way too granular information about ids etc, it should never be shown."
- The DID, file path, doc hash, relay assigned id — none of these are useful to the principal in the review surface or the reader. They're protocol-level. The principal cares about *who*, *what*, and *what it says*.
- Move all of that to: tooltip on the avatar, or a "Provenance" disclosure inside settings/details, or a copy-button that fetches on demand. Never on the primary surface.
- Adjacent angle: the user's wondering whether to incorporate affordances from other communication media:
  - **Threads** (Slack/Discord/iMessage) — group related envelopes
  - **Tagging** (Linear/Notion) — non-hierarchical organization
  - **Replying to specific parts** (Google Docs comments / Hypothesis annotations) — quote-and-respond
  - **Commenting** (Github PR review comments) — inline marginalia, eventual resolve
- The vision is "AI-native professional messaging" (`docs/ideas/ai-native-professional-messaging.md`). What makes it AI-native is that the affordances above can all be *generated* by the scribe rather than wholly handcrafted by the principal:
  - Threads: scribe groups envelopes by topic/recipient/temporal context automatically; principal renames or merges
  - Tags: scribe proposes tags from envelope content; principal approves or rejects (BCT 4.1 Instruction; PDP Suggestion)
  - Reply-to-part: principal selects a passage in the reader; scribe drafts a response in context
  - Inline comments: future review-session ritual — annotate an envelope before stamping, or annotate a received envelope as you read it (private to you, or sent back)
- The dangerous trap: feature-list racing against email/Slack/iMessage. The whole positioning ("async, stamped, professional") loses if the app feels like Slack-with-extra-steps. Each borrowed feature must justify itself against the review-session model, not against feature parity.
- Questions:
  - Which features compose with stamps? Reply-to-part and inline comments do (each annotation is its own stamped artifact). Threads compose easily (a thread is just `in_reply_to` references walked). Tags are local-only metadata — don't need to be on the wire.
  - Which features fight the review-session model? Live commenting / real-time co-editing — both pull async into sync. Reject.
  - Where does each feature live in the surface? Threads = inbox grouping. Tags = secondary filter chip. Reply-to-part = button in the reader modal. Inline comments = sidebar in the reader.
  - Order of operations: which lands first? Probably "reply" (already an `/idea`) → "thread" (one `in_reply_to` field unlocks both reply and threading) → "tags" (local-only, lightweight) → "reply-to-part" (more invasive, requires selection state).
  - What gets dropped from current UI to accommodate? The DID + file path footer in the reader modal — that's the immediate target of the "hide ids" half of this idea. Replace with something useful (in_reply_to ref, tag chips, etc.)
- Don't shape yet.
