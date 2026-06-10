---
type: idea
tags: [mcp, trust-model]
---

# MCP `compose` tool — all docs written through the substrate

- Today Claude writes docs with the generic filesystem Write tool — nothing
  signs them. The signature layer (hard rule #4: every authored body carries a
  detached DID-keyed signature) is only exercised by manifests and stamping;
  agent-authored docs land unsigned.
- A `compose` MCP tool is the missing write-side of the trust model: write the
  doc AND sign it with the scribe's agent DID in one act. Signed-on-write makes
  `sec verify` meaningful for every doc, not just stamped ones.
- Same surface could own the conventions — frontmatter shape, `<date>-<slug>.md`
  naming, doc type (`idea` / `pain` / `decision` / `note`) — so Claude stops
  reinventing them per session.
- Per the parallel-surfaces practice this would ship as application use case +
  `sec compose` + MCP tool together.
- **Key question resolved (2026-06-10):** the scribe's signing key exists —
  `~/.secretariat/identity/agents/claude/key` (0600), provisioned by
  `agent_ops::provision` on `sec agent add`. Its DID matches the live
  `authorized_agents` entry. Compose-signs-as-scribe is unblocked.
- **Placement, not just signing.** The docs-worktree-pipeline spec
  (`docs/superpowers/specs/2026-06-04-docs-worktree-pipeline-design.md`)
  already defines placement-at-birth: narrative docs are written into the
  permanent docs worktree (`resolve_doc_target`), auto-committed + DID-signed
  there, flowing to main via standing draft PR, stamped at merge. `compose` is
  the natural carrier of that logic — the positive verb (resolve target →
  write → sign → commit) instead of hook-based prevention; the spec itself
  rejects the cleanup-hook approach. The spec's planned `docs_ensure` MCP tool
  becomes an internal step of `compose`.
- Questions:
  - Does `compose` cover edits too (re-sign on every save), or only creation?
  - Relation to [[2026-05-31-improve-quick-capture]] — is compose the substrate
    verb that quick-capture's dispatch eventually calls?
  - Carve-out handling: code-coupled docs (lexicons, design notes shipping in a
    feature PR) must NOT route to the docs worktree — does compose take a
    `coupled: bool` or infer from path?

Pairs with [[2026-05-31-search-reference-envelopes]] (the read-side: search).

Don't shape yet.
