# Penceive ↔ repos — the graph layer, folded into Secretariat (All Aboard)

**Date:** 2026-05-31
**Status:** sketch — **fold DEFERRED.** Decision 2026-05-31: keep Penceive **separate**; the repos are the integration layer.
**Extends:** `docs/ideas/2026-05-31-git-native-substrate.md` · `docs/ideas/2026-05-27-signet-protocol.md`
**Driver:** EQTECH "All Aboard" — Penceive is the **graph layer** of the stack. Whether it merges into Secretariat is a *UX* question, not an architectural one.

> **Decision (2026-05-31): keep Penceive separate.** In git-native, the **repos are the
> integration point** — Secretariat (review/stamp/edit) and Penceive (extract/graph/query)
> are two consumers of the same corpus, no merged codebase required. Quick path = **point +
> BYOK** (Phase 0 below). The full fold into `crates/core` is the *eventual* end-state, only
> worth it when one shell is wanted — not the next move. Everything below describes that
> end-state for reference; read it as the destination, not the plan.

---

## ⚓ Premise

The git-native substrate gave us **documents** (repos) + **orchestration** (Claude + the
stamp gate). The missing layer is the **model** — the entities, types, and concepts that
say what an organization *is*. Penceive already builds exactly that. The two apps share the
**corpus** (the repos); they need not share a codebase. The end-state below (fold into
Secretariat) is the autonomous-enterprise substrate at company / org / personal scope — but
it is **deferred** in favor of the Phase-0 point-and-BYOK integration.

## The four layers (where this one slots)

```
4  ORCHESTRATION   Claude + the stamp gate            cognition, woken by launchd
      ▲ reasons over
3  GRAPH           entities · types · concepts        ◀ THIS — Penceive engine in Secretariat
      ▲ extracts from
2  DOCUMENTS       stamped markdown                   repos + git
1  SUBSTRATE       identity · convention              ~/.signet + org_roots
```

## Why it's unification, not a rewrite — the abstractions already match

| Concern | Penceive has | Secretariat has | Move |
| --- | --- | --- | --- |
| Cognition | `InferenceProvider` trait (`OllamaProvider`) | `CognitionPort` (anthropic/**BYOK**/ollama) | **unify** → extraction runs on the CognitionPort; gets Claude-BYOK + subscription + ollama for free |
| Ontology | `domain/blueprint.rs` (`TypeDefinition`, `types:` section) | — | move into `core` |
| Extraction | `agentic_extract.rs` · `extract.rs` · `derive_graph_edges.rs` · `extraction_worker.rs` | — | move into `core` + `infrastructure` |
| Graph store | `fs_knowledge_graph.rs` + `tantivy_graph.rs` (filesystem + Tantivy index) | (filesystem-authoritative ethos, invariant #8) | move — already the right shape |
| Query | `graph_search.rs` · `query.rs` · `mention.rs` · TS `mcp-server` | `crates/mcp` | fold graph tools into Secretariat's MCP |
| Identity/seal | — | Signet `stamp`/`verify`/`read` | the stamp gate annotates graph nodes |

Penceive's `InferenceProvider` ≈ Secretariat's `CognitionPort`. Collapsing them is the whole
BYOK story: **the same port that launches Claude for review now powers entity extraction.**
Sovereign choice per invariant #5 — local `ollama` (cheap, private) for the bulk seed pass,
Claude-BYOK (higher quality) for hard extractions, principal's call.

## The integration — corpus = repos

Today Penceive extracts from its own Tauri `entry`/`content` store. Git-native repoints it:

```
the CORPUS is the repo docs (no separate store)

  <org-repo>/
    blueprint.yaml          ← the org's CONSTITUTION (declared entity types)  "constitution first"
    docs/<date>-<slug>.md    ← the corpus  (extraction reads these)            "corpus later"

  extraction (CognitionPort: ollama | Claude-BYOK), guided by blueprint.yaml
     → typed entities + relations
     → graph  (fs_knowledge_graph + tantivy index — DERIVED, regenerable, encrypted)
     → query via MCP / editor cards / review
```

- **Per-org blueprint** = `blueprint.yaml` at the org root — the declared ontology (person,
  place, decision, concept, role, …). Travels with the org. Optional inheritance org → repo
  (like contracts once accumulated), TBD.
- **Graph = derived read-cache** over the authoritative docs (invariant #8). Rebuildable from a
  filesystem walk; never the source of truth. Penceive's `tantivy_graph` is exactly this.
- **Scope-invariant:** company blueprint, `themia`/`equanimitech` org blueprints, and the
  `saperene` personal blueprint are the *same primitive*. Penceive's engine **supersedes the
  earlier deferred Graphiti pick** — home-grown, blueprint-driven, local, filesystem-backed.

## 🔌 Penceive is multi-source — and Claude Code is a source

The corpus isn't just repo docs. Penceive should read from a **`Source` port** with
pluggable adapters — generalize today's `fs_extraction` into:

```
trait Source → yields entries (text + provenance + trust) into extraction

  RepoDocsSource     repo docs/*.md              stamped corpus       → authoritative-capable
  ClaudeCodeSource   ~/.claude/projects/<repo>/*.jsonl   session transcripts  → reasoning/context
  (future)           Things · Slack · email · web clips
```

**Claude Code as a knowledge source is the key move.** The per-repo session transcripts
(145 in secretariat, 176 in leggia, …) are the org's **cognition record** — what was
reasoned, decided, tried, rejected. Extracting entities/decisions from them means the graph
captures not just *what was written* (docs) but *what was thought* (sessions). That is the
company's working memory — the substrate the autonomous enterprise reasons over.

Each extracted node keeps **provenance** (doc path, or session id + timestamp) so any claim
traces to its source — and **trust** by source: a stamped doc is authoritative; a Claude Code
session is contextual/candidate unless its decision was sealed. This is the bi-temporal /
ground-truth-reachable discipline applied to a heterogeneous corpus.

Design fit: `Source` is a clean SOLID port; extraction consumes `Source`-yielded entries;
adapters are independent. `ClaudeCodeSource` parses the JSONL (per session / per exchange)
into entries. Lands as a Penceive refactor — **after** the BYOK provider work, to avoid
touching the same files.

## The stamp gate feeds the graph

The layer-4 gate becomes a **trust attribute on graph nodes**:

```
doc verify state          → graph node trust
  stamped (verified)      → AUTHORITATIVE node/edge   (a sealed decision the org vouches for)
  unstamped (signed-only) → CANDIDATE                  (deliberation; lower trust)
  tampered                → QUARANTINE                 (stale seal — flag, don't reason on it)
```

So `secretariat-as-company-os.md`'s *"stamps gate progression"* becomes literal: agents reason
over the graph, but only **stamped** nodes are authoritative. The org models itself; humans seal
what's real.

## Where it lives in Secretariat

```
crates/core
  domain/         + blueprint · entity · graph (node/edge) · mention      (from penceive/domain)
  ports/          + InferenceProvider folded into CognitionPort
  application/    + extract_ops · derive_graph_edges · graph_query_ops
  infrastructure/ + fs_knowledge_graph · tantivy_graph · extraction_worker
crates/mcp        + graph query tools (replacing penceive's TS mcp-server)
src-tauri         + graph view / cards / chronos navigation               (from penceive/components)
```

Lands as an **app-layer** concern (Secretariat), not protocol (Signet has no opinion on graphs).

## Migration path (All Aboard, incremental)

1. Lift Penceive's `InferenceProvider` calls onto Secretariat's `CognitionPort` (BYOK/Claude online for extraction).
2. Port `blueprint` + graph domain into `crates/core` (pure, no IO).
3. Port `fs_knowledge_graph` + `tantivy_graph` into `infrastructure`; point the corpus reader at repo `docs/`.
4. Author a first `blueprint.yaml` per org (themia, equanimitech) + saperene.
5. Seed-extract each org's repo docs → graph (ollama bulk, Claude-BYOK for hard cases).
6. Expose graph query as Secretariat MCP tools; wire the stamp-state → node-trust annotation.
7. Graph view in the Tauri shell (Penceive's cards/navigation).

## 🚧 Open questions

1. **Graph store** — keep Tantivy index, or move to embedded graph DB (Kuzu)? Tantivy already filesystem-backed + invariant-clean; lean keep.
2. **Blueprint inheritance** — org-root → repo-leaf accumulation (the one survivor of the dead contract machinery)?
3. **Extraction cadence** — launchd `claude -p` incremental on commit, vs on-demand. (Ties to the orchestration layer's heartbeat.)
4. **Encryption** — Penceive's per-entry encryption vs git-native "repo is the trust boundary." The graph holds extracted facts; keep it local + encrypted, never pushed (like the keychain).
5. **What of Penceive-the-standalone-app** — fully absorbed, or kept as a thin graph-view shell pointing at the same engine?
