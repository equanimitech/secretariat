---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T120136Z-dakgms.md
---
# Secretariat ingestor (Things3 → Secretariat captures)

Promoted from `_self/inbox/tickler` (tickle 2026-07-01, exercised early) 2026-05-18.

**Principal direction 2026-05-18:** two viable shapes — (a) update `/review` skill to traverse Things3 inbox natively (via `things-mcp.get_inbox`), or (b) build a channel-local sync script matching the existing `journals/therapy/bin/sync.sh` + `prompts/` pattern. Therapy precedent: `_self/channels/journals/therapy/bin/sync.sh` + `prompts/therapy-extraction.md`. Today's /review exercised the Things → capture flow 5× manually; substrate works, automation is the missing piece.

---

Raw capture — 2026-04-29. Reframed 2026-05-11: Secretariat is the substrate, not `docs/ideas/`.

- Originally framed as Things3 → `docs/ideas/` + `docs/pain/`. Now Secretariat is the canonical capture surface, so the ingestor's job is **Things3 → Secretariat `capture` (queue: `inbox:triage` or `inbox:pain`)**.
- Three-tier data flow:
  1. **Things3** (ergonomic capture) — existing habit, low friction, personal stuff stays here.
  2. **Secretariat** (typed substrate) — `inbox:triage`, `inbox:pain`, `inbox:waiting`, `inbox:tickler`. Single review surface.
  3. **Linear** (execution) — canonical execution record once a project lands.
- Ingestor logic:
  - Read Things3 via `things-mcp` (inbox + tagged items).
  - Filter: personal stays in Things3; work/dev/product candidates route in.
  - For each candidate, call Secretariat `capture` with queue `inbox:triage` (idea-shaped) or `inbox:pain` (broken/friction-shaped). Inferred from content.
  - Body = Things3 title + notes verbatim. Add a footer `Source: things3:<id>` for idempotency.
  - Tag the Things3 item `[ingested]` to skip on subsequent runs.
  - **Do NOT delete the Things3 todo.** One-way fan-out.
- Content-inferred routing (not tag-driven):
  - Is this work or personal? Personal → skip.
  - Idea (aspirational) vs pain (broken)? → queue selection.
  - Scope hint (zenborg / themia / leggia / ...) → mention in body so the principal can scope at /roundtable time.
  - Manual override: a Things3 tag like `route:queue:inbox-pain` forces the queue.
- When to run: on demand before /review. Optionally scheduled.
- Open questions:
  - Idempotency — Things3 ID in the envelope body / metadata for re-run upserts.
  - Should the ingestor handle Things3 *projects* (vs flat todos)? Default: flat only.
  - Two-way sync (Secretariat envelope → Things3 comment when pitched)? Default: one-way.
  - Other ingestors that could plug in here: Slack saved messages, Gmail starred, browser captures. **Same substrate, different source.**
- Subsumes:
  - `2026-04-29-things3-to-filesystem-sync.md` — fs sync isn't needed if we ingest straight into Secretariat. Archive that one.
  - `2026-04-29-global-shortcut-add-idea-to-repo.md` — partially: a global hotkey would still help for non-Things3 capture moments.
- When to build: trigger fulfilled 2026-05-18 (manual exercise during /review).
