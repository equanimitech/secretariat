# Inputs / outputs / transports

Raw capture — 2026-05-05.

- Should Secretariat have **inputs** and **outputs** as a first-class concept (separate from the existing `transport` adapter idea)?
- Examples floated:
  - **Linear as input** — issues / comments arriving from Linear become envelopes in the inbox.
  - **Slack as input/output** — DMs/threads sync in; replies go back out via the same adapter.
  - Implicit: Gmail, IMAP, iMessage, SMS as transports already in AGENTS.md (rule 4 — "transports are adapters, not authorities").
- Questions:
  - Is "input" / "output" a different concept from "transport", or just two directions of the same adapter?
  - Does Linear-as-input fit the substrate cleanly? Linear issue ≠ envelope-shape (no `from` DID, no recipient queue, no stamp). Wrap in a synthetic envelope `(linear_did, channel:project-X)` on import?
  - Does Slack-as-output collapse into "send to a queue whose owner is a Slack channel adapter"? I.e. `Recipient { owner: slack-workspace-did, handle: channel:eng }`. Adapter on relay side handles fan-out to actual Slack message.
  - What's the unit of trust when an inbound came from Linear/Slack? They're not stamped — the principal didn't sign them. Mark as "unsigned, source-attested" envelopes? Different review track?
  - Conversely, replies _from_ the principal back into Linear/Slack — those ARE stamped (principal signed them locally) but the recipient platform can't verify the stamp. The signature is for the audit log, not for Slack.
  - Is there a generic adapter contract (read N envelopes, write N envelopes) that any input/output plugs into?
  - Inbound rate limits / cadence — Slack DMs are high-frequency, Linear comments medium, IMAP low. Does the principal's `attention-envelope.md` apply to inbound from these too, or only to peer principals?
- Don't shape yet.
