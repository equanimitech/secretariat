---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T130931Z-ncjl32.md
---
# Infra ask — `tags` as first-class envelope axis

**Date:** 2026-05-18
**Context:** /review session today decided envelope-kind classifiers (`pain`, `idea`, `pitch`, `spec`) move from `<channel>:<kind>` subchannels to YAML frontmatter `tags: [...]`. See [[feedback-themia-channel-routing]] and the 2 themia.pro migration envelopes (module:cassation, module:baux_commerciaux) shipped today.

## Gap

The decision is right but the infra doesn't support it cleanly:

1. **`capture` MCP has no `tags` param.** Tags must be hand-inlined into the body's YAML frontmatter. Workable but unenforced — easy to forget, easy to drift on shape (`tags: [pain]` vs `tag: pain` vs `kind: pain`).
2. **`read_channel` has no tag filter.** Cross-tag queries ("show all `pain` envelopes across modules") = filesystem grep. Defeats the whole composability argument for moving to tags.
3. **Envelope schema** — `$envelope` block doesn't declare `tags` as a field. Should it live inside `$envelope:` or at top-level YAML? Today's migration put it at top-level; needs spec ratification.

## Asks

- **`capture(queue, body, tags?: string[])`** — pass-through to envelope frontmatter; daemon owns the canonical shape.
- **`read_channel(handle, tags?: string[])`** — filter by tag set (AND or OR? lean OR for "any of these tags").
- **New tool `search_envelopes(tags, since?, until?, channels?)`** — cross-channel tag query. Optional; could defer if `read_channel`+tags is enough.
- **Envelope schema bump** — declare `tags: string[]` at top level (sibling of `$envelope`), or nest under `$envelope.tags`. Pick one.

## Composes with

- `dev:` parent reorg (today's decision, [[project-dev-channel-taxonomy]]) — same theme: tree topology should mirror domain, not envelope shape.
- Future: `streams` (already in channel CLAUDE.md — `stream=verification`, `stream=triage`, `stream=experiment`, etc.) feel orthogonal to `tags`. Stream = the agent's intent; tag = the envelope's quality. Worth resolving the relationship in the schema bump.
