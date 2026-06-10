# Search + reference envelopes in conversations

**Date:** 2026-05-31
**Status:** project (deferred)
**Source:** Things3 quick-capture, triaged 2026-05-31

> I want to easily search for envelopes and reference them in conversations.

Fast envelope search + an inline reference primitive so a stamped envelope can be cited inside a chat/doc. Pairs with the handle/popover navigation idea from semantic-zoom.

- **2026-06-10:** re-floated as an MCP `search` tool — Claude needs to find docs
  by type/content (ideas, pains, decisions) across registered repos without
  grepping blind. The `[[repos]]` registry gives the corpus; frontmatter `type`
  gives the facet. Read-side sibling of [[2026-06-10-mcp-compose-tool]].
- **2026-06-10, engine:** Tantivy is already live in `penceive-core` (v0.22) —
  `SearchIndex` port + `TantivySearchIndex` infra impl, `wake reindex`
  (incremental) + `wake install-hook` (git post-commit) keep it fresh. The
  penceive extraction explicitly unblocked a Secretariat crate-dep. A Tantivy
  index is the invariant-#5-shaped cache: regenerable, never authoritative.
  - Gap: the current schema is journal-shaped — two fields (`date`, `body`).
    Doc search needs path, repo, frontmatter `type`, title, body, stamp state.
  - Boundary question: Signet seals ↔ Saperene knows — does `search` live on
    sec-mcp, or in penceive's bounded context with Secretariat consuming it?
