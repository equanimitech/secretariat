---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T101826Z-qmbdsk.md
---
# Simplify channel scaffold — is 3 files the floor?

Raw capture — 2026-05-17. Promoted to secretariat:dev from `_self/inbox/triage` 2026-05-18.

- Today every channel has exactly: `.channelDef`, `contract.local.md`, `envelopes/`. That's it. The full design space (outbox, sent, _meta, _ciphertext, template.md) is roadmap, not today's reality.
- Conflating "design space" with "what users see today" makes the system feel more complex than it is. Audit doc + AGENTS.md for places we present design space as current state.
- Three feel non-collapsible because of visibility contracts:
  - `.channelDef` = governed identity, will be signed + shared.
  - `contract.local.md` = private prefs, never sent (the `.local` suffix is load-bearing).
  - `envelopes/` = filesystem-as-truth (invariant #8) — dir-move IS the state transition.
- Real candidates for later simplification when those slices ship:
  - `outbox/` + `sent/` → one dir with `stamped: true` frontmatter? Counter: rename-on-stamp makes `ls outbox/` answer "awaiting my attention" trivially. Probably keep split.
  - `_meta/` + `_ciphertext/` → colocate as `_meta/ciphertext/`. Cosmetic.
- Questions:
  - Could `.channelDef` move INTO `_meta/` once `_meta` ships? Then top-level dir contains only "your things" (contract.local.md, envelopes/) and `_meta/` holds "governance things" (channelDef, roster, template). Cleaner mental model — "private vs shared" → "top-level vs _meta/".
  - Is the `template.md` override worth a file, or just a `template:` YAML field inside `contract.local.md`? Same visibility (private), same purpose (composition pref).
  - Should the principal-facing onboarding show only the 3-file reality, never the design map? The map belongs in AGENTS.md, not in UI affordances.
