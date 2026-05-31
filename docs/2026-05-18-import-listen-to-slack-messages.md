---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T101305Z-hbf7r3.md
---
# Import / listen to Slack messages

Raw capture — 2026-05-12. Promoted to secretariat:dev from `_self/inbox/triage` 2026-05-18.

- Any chance we can import / listen to Slack messages?
- Adjacent angles:
  - One-time import of historical Slack channels into matching Secretariat channels (would mirror the 36-channel Themia taxonomy we just bootstrapped — slack/#analytics-pipeline ↔ channel:analytics:pipeline naturally)
  - Live bridge: new Slack messages stream into the corresponding Secretariat channel as envelopes signed by an adapter agent DID (per the idea doc's "agent-proxied external service" pattern)
  - Slack-as-transport (AGENTS.md invariant #4 sense — Slack pipe carries encrypted Secretariat envelopes between two users) is a *separate* feature from import
- Questions:
  - One-shot import or live tail or both?
  - Map Slack channels 1:1 to Secretariat handles by name, or let the user remap?
  - Whose DID signs imported envelopes? Adapter agent with its own DID? The user's key with origin metadata? Mark them unsigned but with provenance?
  - Two-way (post from Secretariat → Slack) or read-only?
  - Slack OAuth required — central-authority risk against sovereignty invariants? Or scoped tokens fine?
  - Historical import: include user mentions, threads, reactions, attachments?
