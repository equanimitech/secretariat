# Drop envelope `depth` and `urgency` fields

Pitch — 2026-05-21. Source: free-text request — *"author-declared attention hints are inflationary; routing should be recipient-side via `contract.local.md`."*

## Boundaries

### Job to be done
As Secretariat's substrate maintainer, I want envelopes to stop carrying author-declared attention hints (`depth`, `urgency`) so that routing authority lives entirely with the recipient's `contract.local.md` cadence and the wire format stops paying lexicon cost for a field whose own description admits it is "inflationary by nature."

Baseline today: every envelope written since v0.2 carries a required `depth ∈ {gross, subtle}` and `urgency ∈ {now, soon, whenever}` in its frontmatter (see `crates/core/src/domain/envelope.rs:117-120` and `lexicons/tech.equanimi.secretariat.envelope.json` `required: ["from", "depth", "urgency", "source"]`). The agent loop defaults both — almost nothing in the codebase reads them after write. The v0.4 attention-routing daemon was planned to read them (AGENTS.md "out of scope" §"Attention routing daemon"), but the design has since converged on `contract.local.md` cadence + `queue_handle` namespace as the routing inputs.

### Appetite
`medium`

*Picked because:* subtractive change touches the envelope record (lexicon + domain VO), 8+ application use cases that thread the fields through, two CLI commands, the MCP tool surface, four MCP prompt docs, the TS bindings + one UI frontmatter renderer, and a vault-migration script. No new surface. Override with `--appetite=small` if we accept a one-shot migration that ignores legacy envelopes.

## Elements

- **Place:** envelope record — drop `depth` and `urgency` from `lexicons/tech.equanimi.secretariat.envelope.json` `required` and `properties`; bump the `description` to note the v0.11 removal + legacy back-compat.
- **Place:** domain VO — delete `EnvelopeDepth` and `EnvelopeUrgency` enums from `crates/core/src/domain/envelope.rs`; remove fields from `Envelope`, `EnvelopeBuilder`, `WireEnvelope`, and `Defaults`.
- **Affordance:** AGENTS.md rewrite of the v0.4 routing-daemon line — replace *"composes from existing `depth`/`urgency` envelope fields + per-channel `contract.local.md` cadence"* with *"composes from per-channel `contract.local.md` cadence + envelope `queue_handle` + envelope `kind`."*
- **Connection:** legacy back-compat — readers tolerate envelopes that carry the fields (existing on-disk vaults) and silently drop them on parse, the same shape we use for legacy `to`/`handle` (envelope lexicon "Legacy back-compat" §). No vault rewrite required.
- **Connection:** CLI flag removal — `sec compose` and `sec capture` lose `--depth` / `--urgency`; help text + completions regenerate.
- **Connection:** MCP tool surface — `compose` and `capture` tool schemas drop the parameters; the four affected prompts (`compose.md`, `capture.md`, `idea.md`, `pain.md`, `stamp.md`) lose the hint-setting language.
- **Connection:** UI frontmatter view — `FrontmatterField.tsx` + `src-tauri/src/commands/explorer.rs` stop surfacing the two fields in the envelope detail card.

## Risks

### 🐇 Rabbit holes
- **Channel-contract filter language.** `channel_contract.rs:6,27` comment blocks anticipate `depth_filter` / `urgency_filter` consumption fields. They were never implemented but the comments need rewriting to reflect the new routing model (handle-tree + cadence only). Risk: discover an in-flight contract schema that already expects them — quick grep before deleting comments.
- **Snapshot tests in `envelope.rs`.** Twelve `assert_eq!` against rendered frontmatter (`depth: subtle\nurgency: whenever\n`) need updating. Trivial but tedious; rebuilding the golden strings from the parser is the safe path.
- **Migration script.** `scripts/migrate-vault-v0.7.0.sh` only mentions the fields once (in a comment per the count). New `migrate-vault-v0.11.0.sh` is **not** required if readers tolerate legacy fields silently — but we need to verify the YAML parser does not error on unknown keys (it shouldn't; we use `serde(deny_unknown_fields = false)` semantics).

### 🏴 Off-sides called
- **Replacement routing daemon.** Out of scope. This pitch only kills the inputs that aren't load-bearing. The v0.4 routing wedge stays out of scope per AGENTS.md.
- **Removing `cadenceHint`.** Also author-declared, also rarely read — but it differs in kind (it's a scheduling preference, not an attention claim) and the recipient-side cadence model can compose with it. Leave alone.
- **Removing `source`.** Stays. It's retrospective-traceability, not routing.

### 🥩 Fat cut
- No new vault migration tool. Legacy envelopes on disk keep their frontmatter; readers ignore unknown keys; new writes omit. Saves a slice of work.
- No "show this in UI as deprecated" affordance. Just remove from the renderer.

### 🧪 Domain knowledge
- **Lexicon-first rule (AGENTS.md hard rule #3).** Lexicon edit lands in the same commit as the Rust change. The pitch already pairs them; the executor must not split.
- **Receiver-side parser tolerance.** Verify before writing the slice: parse a legacy envelope with the depth/urgency fields against the updated `Envelope` struct, confirm no panic / error. If `serde` strict-mode is set anywhere, relax it.
- **No on-wire stamp coverage of these fields.** Confirmed by re-reading envelope lexicon — stamp covers body only, not envelope frontmatter. Removing the fields does not invalidate any existing stamp.

## Pitch

### Problem
Two fields on every envelope claim authority they don't have. `depth: gross|subtle` and `urgency: now|soon|whenever` are author-declared hints — the writer announces *"this is gross"* or *"this is now,"* and the recipient is expected to honour it. The lexicon itself flags urgency as *"inflationary by nature; the recipient's per-channel contract.local.md cadence governs whether it surfaces inline or queues for review."* So we ship a required field whose authoritative interpretation is "ignore in favour of recipient policy." That is wire-format weight for nothing.

The intent was to feed a v0.4 attention-routing daemon. Since v0.3, the design has converged elsewhere: the recipient's `contract.local.md` declares cadence per channel, the `queue_handle` namespace declares the bucket, and the envelope `kind` declares what record this is. Author-declared attention hints have no slot left. Worse, they encode a centralized-attention model that violates recipient sovereignty over their own focus (Equanimi-Tech "Awareness" principle). Cut them.

### The bet
Spend a medium-appetite slice — roughly a couple of focused days — removing both fields end-to-end: lexicon, domain VO, application use cases, CLI flags, MCP tool params + prompt language, TS bindings, UI renderer. Receiver-side parser stays tolerant (legacy envelopes parse cleanly, unknown fields ignored), so no vault migration ships. AGENTS.md updates the v0.4 routing-daemon line to make `contract.local.md` cadence + `queue_handle` + envelope `kind` the only routing inputs. After this lands, every envelope on the wire is two fields lighter and the substrate stops paying a tax for a deferred feature that won't use the tax.

### No-gos
- No replacement routing daemon in this slice.
- No vault rewrite — legacy envelopes stay as-is on disk; readers silently drop the deprecated keys.
- No removal of `cadenceHint`, `source`, `agSource`, or any other AG-shape field.
- No new `contract.local.md` cadence schema work (out of scope; covered by the existing contract-review-cadence pitch).
- No counter-stamp / multi-party stamp coupling.
