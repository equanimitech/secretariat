---
migrated_from: equanimi.tech/project/secretariat/navigator/ideas/20260517T200239Z-lc76sk.md
---
# Wildcard channel handles for review walks

Raw capture — 2026-05-17.

- Can we easily search for ideas across channels using something like `**:ideas`?
- Wildcards to walk with reviews — `/review channel:**:ideas` matches `torchbearer:ideas`, `secretariat:editor:ideas`, etc.
- Good thing about filesystem-backed substrate: glob is free. `~/.secretariat/orgs/*/channels/**/ideas/envelopes/**/*.md` is one shell glob.
- Adjacent angles:
  - `channel:**:pain` to walk all pain channels across orgs.
  - `channel:module:*:pain` to walk per-module pain queues only.
  - `channel:journals:*` to walk every journal type.
  - Glob in `list_channels` (filter param) + `read_channel` (handle param).
- Questions:
  - Glob syntax: full shell `**` semantics, or simpler `*`-only segment matching?
  - Cross-org: does `channel:**` traverse personal + every org, or default scope to current?
  - Does this also apply to `capture` (route to first match) or only read paths? Lean read-only — writes need an explicit destination.
- Don't shape yet.
