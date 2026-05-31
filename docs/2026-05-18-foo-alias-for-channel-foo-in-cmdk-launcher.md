---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T101819Z-qtbrf7.md
---
# `#foo` alias for `channel:foo` in cmdk launcher

Raw capture — 2026-05-17. Promoted to secretariat:dev from `_self/inbox/triage` 2026-05-18.

- Strip leading `#` on input, prepend `channel:` before resolving.
- Render rows as `#secretariat:dev` instead of `channel:secretariat:dev` — shorter, Slack-familiar.
- UI sugar only. Keeps `channel:` namespace canonical in domain (room for `dm:`, `_meta:`, etc.).
- Zero parser change.
- Questions:
  - Does `#` collide with anything in cmdk's filter ranking?
  - Should the alias also work in CLI (`sec launch #secretariat:dev`)? Different layer.
