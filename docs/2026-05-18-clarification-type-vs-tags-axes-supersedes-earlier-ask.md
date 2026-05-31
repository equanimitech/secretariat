---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T131859Z-ndm6zi.md
type: decision
---

> **Note 2026-05-18 (post-edit):** body below was the in-flight snapshot. Final shape: type list narrowed to 7 (idea, pain, question, decision, pitch, plan, moment) — see `envelope-types.md` lexicon. Tag lexicon dropped; tags are open-set.

# Clarification — `type:` vs `tags:` axes (supersedes earlier ask)

**Continues:** the infra-ask envelope captured earlier today (`20260518T130931Z-ncjl32`). That envelope conflated two axes under "tags" — corrected here.

## The two axes

Envelopes carry two orthogonal categorical axes beyond `$envelope` identity:

1. **`type:`** **(singular, categorical, closed-set)** — the envelope's primary kind. Cardinality 1. Values: `pain | idea | pitch | spec | note | decision | verification | experiment | rapport | pv`. Lexicon: `~/.secretariat/_self/lexicon/envelope-types.md`.
2. **`tags:`** **(multi-valued, composable, open-ish)** — orthogonal qualities. Cardinality 0..n. Values: `urgent | asap | cross-cutting | blocked | revisit | draft | archived | external`. Lexicon: `~/.secretariat/_self/lexicon/tags.md`.

Both live as top-level YAML frontmatter, sibling to `$envelope:`.

## Why the split matters

`pain | idea | pitch | spec` are mutually-dominant categories — an envelope IS-A pain. Forcing these into `tags: [pain]` invites multi-typing (`tags: [pain, idea]`) where the envelope reader can't tell which is dominant. Splitting:

* `type:` enforces single primary kind.

* `tags:` reserved for actually-composable qualities (urgent + cross-cutting + blocked stack fine).

## Revised infra asks

Replace the earlier asks:

* **`capture(queue, body, type?: string, tags?: string[])`** — pass-through both axes; daemon validates `type` against envelope-types lexicon, warns on unregistered values.

* **`read_channel(handle, type?: string, tags?: string[])`** — filter by type (single) and tags (any/all).

* **Envelope schema bump** — declare `type: string` and `tags: string[]` at top level (sibling of `$envelope:`). Both optional but recommended.

* **Lexicon validation hook** — when daemon writes an envelope with a `type:` or `tags:` value not in the lexicon, emit a warning. Hard reject only if the lexicon is marked closed-set.

## Composes with

* `stream:` (channel-level field, declared in channel CLAUDE.md) is a THIRD axis — agent-intent. `stream=verification` is what the verifier-agent emits; orthogonal to envelope-type. May need its own lexicon eventually.

* \[\[feedback-always-update-lexicons]] meta-rule: any new categorical value (type, tag, stream) must register in matching lexicon BEFORE first envelope using it lands.

