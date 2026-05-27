# AG-adaptive depth rendering

Pitch — 2026-05-27. Source: conversation 2026-05-27 (no captured idea file; arose from Rafa's reading-side frustration with dense envelopes piling up in the Move 3c editor).

## Boundaries

### Job to be done

As the principal opening a dense envelope, journal entry, or capture, I want to control the *rendering depth* of what's on screen — a one-sentence summary first, then a thesis + three beats, then section-level paras, then full text — without the document being rewritten and without leaving the editor. I want to drill into the one section that matters and keep the rest shallow. The same attentional-granularity principle Claude already uses to *compose* envelopes (gross → subtle, deepening pathway) applied to *reading*. Baseline today: markdown is rendered at full depth, always; "skim" is `Cmd-F` and willpower; a 4-page stamped envelope and a one-line capture look identical at the viewport scale.

### Appetite

`medium`

> Appetite picked: `medium` — a 2-week bet. Cognition adapter (existing `CognitionPort`, first real consumer), depth-ladder cache, one editor UI surface, one CLI affordance. No new domain primitive, no lexicon edit, no transport change. Override with `--appetite=<size>`.

## Elements

Breadboard, four primary elements. Reading-side only — composition flow (the `attentional-granularity` skill writing envelopes) is untouched.

- **Place — depth ladder.** A document acquires an *AG ladder*: an ordered set of representations at increasing depth, sharing the taxonomy of the `attentional-granularity` skill. v1 levels: **L0** one-sentence headline · **L1** thesis + 3 beats · **L2** section-by-section paras · **L3** full text. Ladders are document-shaped (the section structure of L2 mirrors the markdown headings of L3 — same skeleton, more flesh). The ladder is data, not a re-render: each level is a string the editor swaps in.

- **Affordance — `CognitionPort::generate_ag_ladder`.** First real consumer of the port (architectural invariant #5). New method:

  ```
  generate_ag_ladder(
    doc_hash: DocHash,
    body: &str,
    levels: &[AgLevel],   // [L0, L1, L2] — L3 is the source, never generated
  ) → Result<AgLadder>
  ```

  Returns `{doc_hash, levels: Map<AgLevel, Representation>, generated_by: Did, generated_at}`. Validates the substrate choice (Claude Code, Anthropic API, local Ollama/MLX) without bolting in vendor-specific shape. Local-model adapter is the default for the high-frequency reading loop (latency tolerable); the Anthropic adapter is opt-in for long-form docs (>2000 words) where quality matters more than latency.

- **Place — ladder cache, two strategies.** Aggressively cached, keyed by `doc_hash`. Two storage shapes by document kind:

  | Document kind                                    | Strategy                                                                                                                                                                              |
  | ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
  | Stamped / long-form envelopes                    | **Inline in frontmatter** — `ag_depths: { l0: "…", l1: "…", l2: ["…", "…"] }`. Durable, travels with the envelope, signed-along on next stamp, principal can edit and re-sign.        |
  | Ephemeral docs (captures, journal entries, ideas) | **Sidecar on-demand** — `<doc-path>.ag.json` next to the source. Regenerable; not signed; quietly invalidated when `doc_hash` changes; never written to outbox or sent over transport. |

  Either way: edit the document → `doc_hash` changes → cache miss → regenerate. Ladder is never "stale" because mismatched hashes can't be served.

- **Affordance — editor controls.** Two layers of control in the Tauri Move 3c editor:

  - **Global depth slider** at the document level — four detents (L0/L1/L2/L3), keyboard `1`/`2`/`3`/`4`, default depth per channel optionally set in `contract.local.md` (`default_render_depth: L2`).
  - **Section-level drill on hover** — `[` deepens the section under the cursor, `]` shallows it. The most important interaction: readers expand the section they care about; the rest stays shallow. Section depth is independent of global depth (whichever is deeper wins).

  Sketch (fat-marker):

  ```
  ┌─────────────────────────────────────────────────────────┐
  │ envelope: ag-handoff to marcelo                  [⚙]    │
  │ ───────────────────────────────────────────────────────│
  │ depth:  ●─────○─────○─────○    L0 L1 L2 L3              │
  │                                                          │
  │ ## thesis (L1)                                  [ depth ▾│
  │   bounded autonomy is a contract, not a permission.      │
  │                                                          │
  │ ## section: agent contracts (L2 — drilled with [ )      │
  │   the agent contract sits between the principal and the │
  │   agent. it carries cadence, scope, and revocation… (L3 │
  │   prose here — section drilled independently of global) │
  │                                                          │
  │ ## section: trust model (L1 — shallow)                  │
  │   three layers, selectively stamped.                     │
  └─────────────────────────────────────────────────────────┘
  ```

- **Affordance — CLI parity.** `sec view <file> --depth L0|L1|L2|L3` renders the chosen depth to stdout (default L2). `--regen` forces a fresh ladder. Per AGENTS.md rule "every principal-facing primitive ships on both interfaces" — though here the *primary* surface is the editor; CLI is the headless escape hatch (also gives the dev loop a tight feedback path during the bet).

- **Connection — generation triggers.** Three points where the ladder fires:
  1. **On stamp** — stamped envelopes pre-generate the ladder synchronously, store in frontmatter, sign as part of the envelope. Worst-case latency; happens once; user already in a ceremony.
  2. **On save** (drafts, captures, journal entries) — background generation, sidecar write, no UI block.
  3. **On first read** (uncached) — synchronous, spinner inline, only the requested depth blocks (L0 first, others stream in if we want — see no-gos for v1).

## Risks

### 🐇 Rabbit holes

- **Cognition latency makes the slider feel sluggish.** A 4-detent slider with 1.5s-per-detent regenerations is unusable. Mitigation, three layers: (a) local-model adapter is default for editor interactions (sub-300ms for L0/L1 on a 1000-word doc on Apple Silicon, by our back-of-envelope); (b) all ladders pre-generated on stamp/save so 90%+ of reading hits warm cache; (c) when the cache is cold, **degrade gracefully** — render L3 immediately, slider position shows "generating L1…" with a spinner; never block the user from reading the source. Quality bar for the bet: cold-start L1 ≤ 800ms on local, slider transitions ≤ 50ms cached. **Figureoutable.**

- **Cache invalidation on edit.** The ladder is bound to `doc_hash`; any edit invalidates. For *stamped* envelopes this is fine (stamping is rare, immutable post-stamp by rule #4). For *drafts* and *journal entries* under active edit, regenerating on every keystroke is wasteful. Mitigation: debounce regeneration to 2s after last edit, and gate behind "render at L3 if no current ladder" — the user always has the source; ladders catch up async. **Figureoutable.**

- **Bad summaries at low depth.** L0 from a small local model on a Themia legal brief will sometimes be nonsense ("This document discusses legal matters"). The ladder is *a proposal*, not authoritative. Mitigation: the editor lets the principal edit any level inline — `Cmd-E` on the L1 view opens it for editing; saved edits persist to the cache (frontmatter for stamped docs, sidecar for ephemerals) and are *preferred* over regenerated content until the source body changes. For stamped envelopes, principal-edited summaries are signed as part of the envelope — the principal vouches for their own elevator pitch. **Figureoutable.**

- **AG taxonomy drift between composition and rendering.** The `attentional-granularity` skill (composition) and this renderer must share level definitions, or "L1 thesis + 3 beats" means different things in different places. Mitigation: extract the level taxonomy to a single source-of-truth doc (`docs/developer/ag-ladder-taxonomy.md`), referenced by both the skill prompt and the cognition adapter's system prompt. Both surfaces consume the same definitions. **Easy** — one doc, two readers.

- **Where the ladder gets sourced when cognition is offline.** Local model may be loading; network may be off; user opens an uncached long doc. Mitigation: render L3 (the source), show a small "summaries unavailable — cognition offline" affordance, never block reading. The renderer is *additive*; failure mode is "you see what you already had." **Easy.**

### 🏴 Off-sides called

- **Auto-depth from reading attention / scroll dwell.** The interesting future ("Claude notices you've dwelled on section 3 and shallows the others") needs telemetry-like signals that conflict with architectural invariant #2 (no telemetry). On-device dwell-tracking is *technically* fine — never leaves the box — but the UX research load is its own pitch. Out for v1.

- **Per-paragraph granularity.** Sections are the unit; paragraphs inside a section are not independently shrinkable. Section-level is enough for the dense-doc problem; paragraph-level would explode the cache and the UI. Defer.

- **Cross-doc digest views** ("show me L0 of every stamped envelope this week"). Real and valuable — but it's a different feature (digest assembly + cross-doc walker), not a depth renderer. Separate pitch.

- **Multi-language depth rendering.** Themia is French; some docs will be French-source. The cognition adapter needs to *preserve language* (not translate) at each level. v1: prompt-engineer the adapter to match source language; verify with one French doc in the pilot. Full multi-language affordances (language-aware level taxonomy, mixed-language docs) are deferred.

- **Streaming partial depth levels.** Generating L1 with chunks arriving over 800ms is nicer than a spinner. Out for v1 — synchronous returns, spinner inline. Streaming is a follow-on once the port shape is proven.

- **Cognition-port shape changes for *non*-rendering consumers.** The port grows one method here. Other consumers (review summarization, draft critique, etc.) will want their own methods. The port's eventual shape is a design problem for later; this pitch adds *one* method, in the spirit of the slice.

### 🥩 Fat cut

- **A separate "depth" domain primitive.** Tempting to add `Depth` to the envelope domain. Cut — depth is a *rendering* concern; the envelope's wire shape doesn't care. Frontmatter `ag_depths:` is a cache key, not a typed field. If we later want signed depth ladders as first-class artifacts, add the lexicon then.

- **A new MCP tool for ladder generation.** The cognition port already does it; exposing `generate_ag_ladder` to remote agents is unmotivated for v1 (the principal is the only reader). Defer.

- **Per-channel cognition adapter override.** "This channel uses Claude, that channel uses local" — interesting; out. v1: one cognition substrate per principal, the principal's existing choice. Channel-level cognition is its own pitch.

- **Editor history of depth choices.** "Restore my reading depth on this doc from last time." Cute, premature. v1: default depth from contract or L2, no persistence.

### 🧪 Domain knowledge

- **Verify with Rafa: which surface ships first — Tauri editor or CLI?** AGENTS.md says both, but timing matters. Editor probably first (the dense-doc problem is felt in the editor); CLI follows in the same bet but lower polish. Confirm.
- **Verify the local-model latency assumption.** "Sub-300ms on Apple Silicon for L0/L1 of a 1000-word doc" is hand-waved. Pilot on Rafa's M-series with a real Themia brief and a real journal entry before locking the default adapter to local.
- **Pilot doc selection.** First test: take one stamped envelope to Marcelo (~1500 words, dense) and one journal entry (~400 words, narrative) — do L0/L1/L2 actually feel useful, or is the renderer hiding the doc's voice? If voice gets flattened, the bet pivots toward L1-edited-by-principal as the default and L0/L2 become optional.
- **Confirm AG taxonomy mapping.** The `attentional-granularity` skill names its levels — match those names exactly in the taxonomy doc. Don't invent parallel vocabulary.

## Pitch

### Problem

Markdown documents in Secretariat get dense fast. A stamped envelope to Marcelo about agent contracts is 1500 words; a daily journal entry runs 600; a captured idea may be a paragraph or eight. The Move 3c editor renders all of them at full depth always. The principal opens a channel, sees a wall of body text, and either (a) reads everything (expensive), (b) skims with `Cmd-F` and willpower (lossy), or (c) defers ("I'll read it properly later" — and doesn't). The result is that dense docs go unread; the substrate's own promise of *intentional, paced review* (memory: `feedback_review_session_model`) is undermined by the rendering layer.

Composition already solves this on the *write* side: the `attentional-granularity` skill drafts envelopes that *deepen* — headline, then thesis, then beats, then prose. The reader-side analogue is missing. Either we have to write every envelope twice (once for skimmers, once for readers), or the renderer learns to *project* the deeper version onto the shallower ones on demand. The latter is what this pitch builds.

The leverage point: the cognition substrate is already a first-class concept (architectural invariant #5, `CognitionPort` in `crates/core`), but it has *no real consumer yet*. AG-adaptive rendering is a natural first user — it exercises the port's shape with a concrete, frequent, latency-sensitive workload, and validates the local-model adapter as a real path (not just a sovereignty checkbox). Building this proves the port *and* solves the reading-side density problem in the same bet.

### The bet

Medium appetite, 2 weeks. Ship:

1. **`CognitionPort::generate_ag_ladder`** with two adapters: local (Ollama/MLX) as default, Anthropic API as opt-in for long docs.
2. **AG-ladder cache** — frontmatter `ag_depths:` for stamped/long envelopes (signed-along on next stamp); sidecar `.ag.json` for ephemerals (captures, journal entries, drafts).
3. **AG taxonomy doc** (`docs/developer/ag-ladder-taxonomy.md`) — single source of truth shared by composition skill and rendering port.
4. **Tauri editor surface** — global depth slider (`1`/`2`/`3`/`4`), section-level drill (`[`/`]` on hover), `Cmd-E` to edit a level inline (principal-edited levels override the generated ones; persist + sign on stamp).
5. **CLI parity** — `sec view <file> --depth L0|L1|L2|L3 [--regen]`.
6. **Generation triggers** — synchronous on stamp (pre-fill frontmatter); debounced background on save; sync on first uncached read with graceful L3 fallback.
7. **Tests** — cache hit/miss by `doc_hash`; cold-start latency budget on local adapter; degraded mode when cognition is offline; principal-edited level survives regeneration; section-level depth independent of global.
8. **Pilot** — Rafa reads one stamped Marcelo envelope and one journal entry through the new renderer for one week; qualitative report.

This pitch pays off because *every* downstream reading surface — review walker, channel browser, digest views, eventually the Penceive layer — needs a depth-aware projection of dense docs. Front-loading the cognition-port shape + the cache layout means the next surface doesn't reshape the substrate; it just adds a viewer. And the substrate validates `CognitionPort` in a real workload before any heavier consumer (review summarization, draft critique, agent loops) commits to its shape.

### No-gos

- No domain change in `crates/core/src/domain/envelope.rs`. Depth is rendering, not envelope shape.
- No lexicon edit. `ag_depths:` is a cache key in frontmatter, not a typed lexicon field. (If we later want signed depth ladders as first-class artifacts, that's a separate lexicon pitch.)
- No new MCP tool exposing ladder generation to remote agents. Principal is the only reader in v1.
- No auto-depth from scroll/dwell signals. Reader picks depth explicitly.
- No per-paragraph granularity. Section is the unit.
- No cross-doc digest views (L0 of every recent envelope). Separate feature.
- No streaming partial depth levels during generation. Synchronous return + spinner; streaming is a follow-on.
- No multi-language depth rendering beyond "match source language." French docs stay French at every level; mixed-language is deferred.
- No protocol-layer claim of any kind. This is app-layer; if Signet (the proposed stamping-protocol split) materializes, AG rendering stays in the app, not the protocol.
- No telemetry on which depth users pick, how often regen happens, etc. (Rule: no telemetry, ever.)
