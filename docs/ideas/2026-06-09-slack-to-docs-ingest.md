---
type: idea
tags: [themia, transport]
---

# Slack → docs ingest (the inbound transport)

We now emit **out** to Slack on stamp (`announce-decision` → #general). The
mirror image: pull messages **in** from Slack and materialize them as docs in
the git substrate.

**Why it completes the picture.** Secretariat's transport becomes bidirectional
— the Signet↔Secretariat boundary in both directions. A decision made *in* Slack
(not in the editor) should still be capturable as a `decision` doc; a discussion
thread worth keeping becomes a doc for the record. Today those conversations are
stranded in Slack; the substrate never sees them.

**Open questions (why it's parked, not built):**
- **Trigger.** What pulls a message in — a Slack reaction emoji (📌 → ingest), a
  slash command, or a manual `sec ingest slack <thread-url>`? Reaction-as-trigger
  is the most ambient (no app-switch), mirrors the stamp-as-trigger ethos.
- **Type inference.** A pulled thread → which doc type? Probably `note` by
  default, `decision` if the human marks it. Don't auto-classify.
- **Provenance.** An ingested doc is *signature-level at best* — it's a Slack
  message, not principal-attested. It's a draft until the principal stamps it.
  Keep the trust tiers honest (signed ≠ stamped).

**Precedent:** supersedes/relates to the lapsed
`docs/2026-05-18-import-listen-to-slack-messages.md` — transports-as-adapters
returning as an inbound adapter that never weakens the trust model.

Pairs with [[2026-06-09-stamp-workflow-trigger-design]] (the outbound half).
