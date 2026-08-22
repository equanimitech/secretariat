# Hister integration seams

Whether Signet/Secretariat can plug into [hister](https://github.com/asciimoo/hister)
(AGPLv3, Go, v0.17.0). Investigated 2026-08-22 against a shallow clone of `master`.

Related: [[git-native-substrate]] · [[saperene-search-engine]]

## What was examined

| Path | Finding |
|---|---|
| `server/extractor/extractors/_extractor_template/extractor.go` | A real, documented plugin template. Copy dir, drop the `_`, implement `Match`/`Extract`/`Preview`, **register in `registry.go`**. |
| `server/extractor/sdk/sdk.go:18-23` | `Capabilities{Enrich, Extract, Preview}`. `Enrich` = "only annotates documents and should never select their body." |
| `server/extractor/extractor.go:199-207` | Enrichers run as a distinct phase, gated on `Capabilities().Enrich`. |
| `server/extractor/extractors/jsonld/`, `embeddedvideo/` | Two existing `Enrich: true` extractors — the pattern is live, not theoretical. |
| `server/document/document.go` | `Document` carries `Label string` (single) and `Metadata map[string]any`. Local files are `file://` URLs (`files.PathToFileURL`), so they run the **same** extractor pipeline as web pages. A `markdown` extractor already exists. |
| `server/indexer/indexer.go:2095` | `docMapping.AddFieldMappingsAt("metadata", noIdxMap)` — **metadata is stored but NOT indexed.** |
| `server/indexer/indexer.go:2087` | `label` IS indexed (`um`, keyword-ish). |
| `server/indexer/searchschema/schema.go` | Declarative field table: `domain title url text type language label visits updated added user_id`. Clean one-row-per-field. Facets + sorts defined alongside. |
| `config/config.go:109` | `Indexer.Directories[].Label` — per-watched-directory label, static. |
| `cmd/import_file.go:172-208` | `documentLabelOverride` — per-file label at import time. This is the seam that lets label vary per document. |
| `server/mcp.go:200,360` | MCP surface is a single `search` tool. No write/annotate tool. |

## What was weighed

**A. Enricher plugin calling `sec verify`.** `Capabilities{Enrich: true}`, `Match` on
`file://` + body containing `$attestation`/`$signature`, shell out to the prod `sec`
binary, write stamp state. Correct phase — never touches body selection, so the existing
markdown extractor still owns `d.Text`.

**B. New `stamp` field in `searchschema`.** Required if you want `stamp:sealed` as a real
query filter, because metadata is `noIdxMap`. One row in `Fields()`, one mapping in
`indexer.go`, optionally one facet. Small diff, but it changes the index schema → full
reindex, and it is squarely a fork.

**C. Label-only, no fork.** `hister index --label sealed` / per-file `documentLabelOverride`.
`label` is already indexed and already user-visible. A post-commit hook runs `sec verify`
and re-imports with the right label. Zero Go, zero fork.

**D. Stamping from hister.** Rejected — see below.

## What was rejected, and why

**Stamping from the hister UI (D).** Hard rule #4 requires the full body rendered verbatim,
same-turn consent, and a Touch ID dialog reason matching what was displayed. A search-result
preview panel renders a *snippet* — it is structurally the wrong surface, and building it
would create a path where the principal seals something they only partially read. Stamping
stays in the editor and the MCP.

**Writing stamp state into `d.Metadata` alone (naive A).** Looks right, does nothing useful:
`noIdxMap` means it is unsearchable. You get a preview-panel badge and no query filter. Any
serious version of A drags in B.

**Compiling Signet into the enricher.** Hister is AGPLv3; a compiled-in enricher is a
derivative work, which would reach into Signet. Shelling out to `sec verify` as a separate
process is arm's-length, keeps signing keys out of hister's address space, and matches the
existing sidecar pattern. Take the subprocess even where linking would be technically easier.

**Hister as authoritative store.** Never — invariant #5. It is a regenerable read-cache over
git. That is precisely what makes it *compatible*, and it stops being compatible the moment
anything reads stamp state from the index instead of from `sec verify`.

## The forcing function

Hard rule #5: a document whose signature fails "is malformed and must be quarantined, not
surfaced." A full-text search engine over the git substrate that does **not** verify is a
rule-5 violation by construction — it surfaces tampered bodies as ordinary hits, ranked by
relevance, with no signal. So verification-at-index is not a feature on top of hister
indexing stamped docs; it is the precondition for doing it at all.

Corollary: if C (label-only) ships, the label must be written by `sec verify` output and
refreshed on every reindex, or it decays into a stale claim about integrity — worse than no
claim.

## Spike result (2026-08-22) — option C works

Branch `worktree-hister-label-spike`, script `scripts/hister-label-spike.py`.
Ran against the live 221-doc corpus on an isolated hister (port 4455, throwaway data dir).

```
scanned 221 · emitted 219 · quarantined 2
    unsigned       193
    sealed          18
    signed           8
  ! tampered        1
  ! unverifiable    1
```

- `label:sealed` → exactly 18 hits. The label field carries stamp state with **no fork**.
- `label:tampered` → empty, and full-text search for phrases unique to the tampered body
  (`"framing recommit"`, `bloat audit`) returns nothing. Quarantine holds at the index
  boundary, not just in the label.
- Control: a phrase from a sealed doc resolves normally, so the empty results above are
  real absence, not a broken index.
- Re-import is idempotent — keyed on URL. 219 docs stay 219, sealed stays 18. The refresh
  loop is just "regenerate JSONL, re-import."

The mechanism is **not** directory-watching (its label is static per config, and cannot vary
by stamp state). It is `hister import file` reading export-JSONL, one Document per line,
with `label` set per document. That path accepts the full `Document` struct and skips
reprocessing, so the labeler owns classification end to end.

## Resolved from "unknown"

- **Sensitive-content false positives** — no. `sensitive_content_patterns` matches PEM
  headers and cloud-provider key shapes; an `ed25519:`-prefixed base64 signature matches
  none of them.
- **`sec verify` exit code is not a gate.** It exits **0** on a hard parse failure, writing
  the error to stderr and nothing to stdout. A labeler trusting `returncode` silently
  mislabels a corrupt doc as `unsigned`. The JSON parse must be the gate. This is the single
  most dangerous detail in the integration.

## Two corpus findings the spike surfaced

- `docs/2026-05-26-bloat-audit-framing-recommit.md` — `signature=tampered`, `stamp=verified`.
  A stamped doc whose body signature no longer matches. Consistent with the envelope-rewrite
  corruption described in the recent `docs(pain)` commits; a live instance, not a hypothetical.
- `docs/milestones/2026-04-30-first-signed-message.md` — carries the pre-rename
  `app.equanimi.secretariat.stamp` `$type`; current verify refuses it outright. A namespace
  migration missed one file.

## What remains unknown

- Whether upstream would take the enricher as a PR. `_extractor_template` + registry
  registration suggests contributions are expected, but a crypto-verification enricher with a
  subprocess dependency is a bigger ask than a site scraper. Unasked.
- Cost of the `wake` post-commit hook doing verify-then-reindex across ~3K notes
  ([[penceive-core-wake]]). The 221-doc run was comfortable; 3K is untested, and `sec verify`
  is one process spawn per doc.
- Whether the label survives a `hister reindex` (as opposed to a re-import). Not traced —
  the re-import path makes it moot for the refresh loop, but it matters if directory-watching
  is ever mixed in.
- Whether `label` should carry stamp state at all long-term, given it is a *single* string
  field. Using it for stamp state spends the only general-purpose label a document has.
  That is the strongest argument for eventually paying for option B.
