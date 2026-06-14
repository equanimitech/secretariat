# Chronological timeline — design

A read-only view over the substrate that answers "what did I create today / over
the last days / last month," with stamped / signed / raw distinguished.
Chronological zooming, applied to the doc surface (the Attentional-Granularity
axis, on time).

## Shape

`timeline` lists docs across every registered repo, grouped by date, each badged
by state. Cheap by construction: dates come from the `<date>-<slug>.md` filename
prefix (no read to bucket); state is a tolerant frontmatter peek (no decryption,
no typed deserialization — schema drift never hides a doc).

- **stamped** — `$attestation` present (principal committed; authoritative)
- **signed** — `$signature` present, no stamp (scribe-composed; informational)
- **raw** — neither block (plain markdown)

## Surfaces (parallel, per AGENTS.md)

- **Core** — `application/timeline_ops.rs`: `build_timeline(prefs, today, range, filter)`.
  Pure orchestration; `today` injected so range logic is testable and core never
  calls `Utc::now()`.
- **CLI** — `sec timeline [--range] [--zoom] [--tag] [--state] [--bucket] [--json]`.
- **MCP** — `timeline` tool (read-only), returns `{from, to, zoom, total, by_day, entries}`.

## Parameters

| param | values | default |
|---|---|---|
| `range` | `today` · `Nd` · `YYYY-MM` · `YYYY-MM-DD` · `A..B` | `7d` |
| `zoom` | `day` · `week` · `month` | `day` |
| `tag` | repo manifest tag (`equanimitech`, `themia`) | all |
| `state` | `stamped` · `signed` · `raw` | all |
| `bucket` | top-level dir under `docs/` (e.g. `decisions`) | all |

The zoom is the feature: `month` returns the per-day histogram only (compact);
`week`/`day` include per-doc entries. `abs_path` on each entry feeds `read`.

## Boundaries

- Lexicon-first does not apply: no new persisted record shape, so no `lexicons/`
  diff. The output DTO is an ephemeral query view.
- Companion to `/review-repos` (the stamp-state review walker) — this is the
  chronological cut of the same git-native substrate.

## Status

Shipped 2026-06-14: core + tests (9 passing), CLI, MCP tool. Clippy clean
(`-D warnings`). MCP tool requires a `sec-mcp` rebuild + reinstall + session
restart to surface in the agent.
