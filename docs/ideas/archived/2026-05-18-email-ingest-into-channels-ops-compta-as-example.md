---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T101321Z-tlqafq.md
---
# Email ingest into channels (ops:compta as example)

Raw capture — 2026-05-12. Promoted to secretariat:dev from `_self/inbox/triage` 2026-05-18.

- Would love to ingest from my email into ops:compta — e.g. invoices, receipts, bank statements arrive as Gmail attachments + plaintext, get pulled into the channel as envelopes (stream=invoice?).
- Same primitive could power other channels: client correspondence emails → channel:clients; vendor contracts → channel:assemblee_generale? (no, separate); newsletters/inbound press → general or com:newsletter (inbound flavor).
- Transport-adapter angle: Gmail is already the universal bootstrap adapter per AGENTS.md invariant #4. An ingest adapter watches a labeled Gmail folder, parses message + attachments, captures into a target channel keyed by label.
- Questions:
  - Is this a "transport-adapter for inbound" generalization vs ops-compta-specific?
  - Stream tagging: invoice / receipt / bank-statement / vendor-comm — start with one (invoice) or design taxonomy upfront?
  - Trust gate: invoices probably signed-only; only the AG-related ones eventually stamped (board meeting minutes referencing financial decisions)
  - OAuth scope — read-only on Gmail; never delete/move on Gmail side (anti-compulsion = source of truth stays in Secretariat)
  - Does this need MCP tooling or is it daemon-side (poll Gmail label → capture loop)?
