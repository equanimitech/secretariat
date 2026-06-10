---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:110a2323d4354698a723f461732ab48c1eea54fc6d0288c970fe0b8eb6aec7f7
  docFilename: 2026-06-10-search-keystone-slice.md
  stampedAt: 2026-06-10T10:57:40.282079Z
  signature: ed25519:CDmVQy4Ju3KCx9mySwc/J3RZ8WrgaUywE5I3pSHheefBPLtyHnjoCeP8c1m2XZpqetU93fQ8ZKo4S8IURDcWDA==
---

# Pitch — `sec search`: cross-repo doc search on penceive's index

**Bet:** Ship `sec search` + an MCP `search` tool over penceive-core's existing
per-repo Tantivy index — one day, zero penceive schema changes.

**Why it matters:** The scribe finds prior thought (ideas, pains, decisions)
across every registered repo instead of grepping blind — and it's the first
live wire between the boat's seal layer and its know layer.

---

## Boundaries

**JBTD:** As the scribe, when the principal references prior thinking ("the
idea we had about X"), I want ranked doc search across registered repos so I
find the doc instead of re-deriving it. Baseline today: blind `grep -ril` per
repo — works only when repo and phrasing are already known.

**Out:**
- No Tantivy schema change in penceive (indexed type/title/stamp fields = next slice, penceive's context).
- No editor/UI search — CLI + MCP only.
- No semantic/embedding search.
- No `compose` tool (sibling idea, own pitch).

## Elements

- **Dependency wire** (`Cargo.toml` workspace deps). Add `penceive-core` as a
  git dep pinned to a rev; path override for local dev. Release build must
  still pass — verify CI auth first (see 🧪).
- **`search_ops` use case** (`crates/core/src/application/search_ops.rs`, new).
  Per `[[repos]]` entry: `reindex_repo(repo, false)` (incremental, cursor at
  `.penceive/HEAD`), then `TantivySearchIndex::search`; merge hits by score.
  Resolve hit id `repo_docs:<rel-path>` → absolute path; read frontmatter for
  the `--type` filter; `$attestation` presence → `stamped` flag (reuse
  `verify_document` parsing).
- **Parallel surfaces** — `sec search <query> [--type] [--repo] [--limit]`
  (`crates/cli/src/commands/search.rs`) + MCP `search` tool
  (`crates/mcp/src/server.rs`). Compact hits: path, title, type, stamped,
  score, preview — never full bodies.
- **Tests** — use-case test over two temp git repos with typed docs: assert
  cross-repo hits, type filter, stamped flag. Gates: `cargo test --workspace`,
  `cargo clippy -- -D warnings`.

## Risks

**🐇 Rabbit holes:**
- CI access to the penceive repo for the git dep (private repo → release.yml
  needs a deploy key). Resolve before coding; don't improvise vendoring.
- First search on an uncursored repo triggers a full rebuild — accept the
  one-time cost, no progress UI.
- `--type` post-filters by reading hit files — cap `--limit` (default 10)
  instead of optimizing.

**🏴 Off-sides:** Doc-shaped index schema (typed fields, facet queries, global
ranking) — next slice, lands in penceive.

**🥩 Fat cut:** Cross-repo score normalization (shared IDF). Naive concat is
fine at this corpus size.

**🧪 Domain knowledge:**
- Hit id format is `repo_docs:<rel>` (`repo_docs_source.rs:55`) — strip the
  prefix to resolve paths.
- `reindex_repo` writes `.penceive/` *inside* the target repo — confirm it's
  gitignored in every registered repo, else every search dirties trees.

## Acceptance

1. `sec search "tantivy"` from any cwd returns ranked hits from ≥2 registered
   repos, each with path, title, type, stamped, preview.
2. `sec search --type idea compose` returns only docs with frontmatter
   `type: idea`.
3. MCP `search` returns the same hits via `sec-mcp`.
4. Second search reuses the incremental index (sub-second on this repo).
5. `cargo test --workspace` + `cargo clippy -- -D warnings` green; release
   pipeline builds with the new dep.

---

_Drafted by Claude (scribe)._
