---
type: pain
---

# MCP `read` rejects plain git-native docs

- `mcp__secretariat__read` fails with `envelope frontmatter missing` on any doc
  without a `$envelope` block — which is every plain git-native doc (pitches,
  decisions, ideas). Only legacy envelopes pass.
- Breaks step 1 of the stamp ceremony as written: the scribe is told to `read`
  before `stamp`, but `read` errors on exactly the docs we now stamp. Workaround
  today: filesystem read.
- Source: `crates/cli/src/commands/read.rs:50` — `parsed.envelope.ok_or(...)`.
  A doc with editorial-only frontmatter (or none) should print its body; the
  `$envelope` decrypt branch should be the special case, not the gate.
- Hit live 2026-06-10 stamping `docs/pitches/2026-06-10-search-keystone-slice.md`.
