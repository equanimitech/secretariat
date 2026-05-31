---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T084220Z-rssugf.md
---
# Outbox dir vs filter-by-stamp

- Every queue has `outbox/` (drafts) + `envelopes/` (attested). For self-addressed journals "outbox" reads wrong — nothing's going out.
- Two dirs encode "staged-for-stamp" vs "attested," but the three-layer trust model (AGENTS.md #4) already says stamped-subset = authoritative, signed-only = ambient. That's a filter, not a directory boundary.
- Wondering: do we need the two-dir split at all? Alt — single `envelopes/` stream, every envelope signed at compose time, stamped subset queried by reading frontmatter or sidecar stamp record.
- Selective-stamp rule already implies "most envelopes flow unstamped" — filtering is the access pattern. Directory split forces stamp = mutation (mv between dirs); filter approach makes stamp = append (write stamp record). More consistent with append-only invariant.
- Cost of current: outbox name misleading for self queues; two paths for daemon (outbox watcher + envelopes reader); rename churn if we keep dirs but rebrand to `drafts/`.
- Cost of unifying: stamp ceremony loses its dir-boundary tell; need disciplined frontmatter or sidecar for "is this stamped"; daemon needs to filter not walk.
- Hierarchy rule reference: AGENTS.md #4 — signature mandatory, stamp selective. If trust is layered at the record, why does the filesystem split by stamp status?

- Don't fix yet.
