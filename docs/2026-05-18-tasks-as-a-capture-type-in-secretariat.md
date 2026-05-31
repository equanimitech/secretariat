---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T101129Z-5zrwe7.md
---
# Tasks as a capture type in Secretariat

Raw capture — 2026-05-12. Promoted to secretariat:dev from `_self/inbox/triage` 2026-05-18.

- Envelope with `state` field (open / scheduled / done) + optional `when`. Same primitive as `/idea` and `/pain` captures, one new field.
- AI agents can write tasks too — their own todos, drafted tasks for principal review. Things-MCP can't carry signed agent provenance; Secretariat can.
- Scope discipline: stay substrate (storage + sync + AI-readable + signed). Don't compete with Things/Linear on UI, scheduling, reminders, recurring rules — scope creep.
- Things-as-adapter: let Things stay as a front-end that reads from `<self>:tasks` queue if user wants polished UX. Same pattern as Slack-as-transport — dumb pipe in front, signed envelopes underneath.
- Self-channel tasks queue: `did:web:rafa.equanimi.tech#tasks` (or `<self>:tasks`). Channel tasks: `<channel>:tasks` — team-visible work items per channel context, drafted by agents, stamped by humans when committing to deliver.
- Target: v0.4 wedge. Driver: tasks as ambient agent traffic (most signed-only) with selective stamps marking commitments. Mirrors v0.3 channel pattern at a different granularity.
- Questions:
  - Does `state` belong on the envelope or in a meta record (state-transition envelope referencing the original)? Latter is more pure (envelopes immutable), former is more ergonomic.
  - Does `when` collapse into the existing `urgency` / `depth` fields, or is it orthogonal?
  - Channel `tasks` queue vs. a sub-handle convention (`<channel>:work:tasks`) — when does the latter become useful?
