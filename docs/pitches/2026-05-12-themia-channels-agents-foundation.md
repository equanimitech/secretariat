# Themia channels + agents foundation

Pitch — 2026-05-12. Source: free-text shaping session (this conversation).

**Hard dependency:** v0.3 substrate (`docs/ideas/2026-05-12-secretariat-as-autonomous-enterprise-substrate.md`) — channels primitive, owner-as-sequencer, channel-dir-is-activation-surface. Must land alongside or after slice 1 of the end-state substrate monoslice (`docs/ideas/2026-05-12-end-state-substrate-monoslice.md`).

## Boundaries

### Job to be done

As the principal of Themia (a French legal-jurimetrics company), I want a curated channel tree with French handles + display names and one agent slot per channel, so that subsequent agent-implementation pitches (Vérificateur per module, Chercheur jurimétrique, Secrétariat digest, Rédacteur clients, Rédacteur com, Compta) each have a deterministic home directory, an inheritable skill scope, and a contract defining cadence + trust gate. Today's baseline: 36 speculative channels with mixed-language handles, no agent assignments, no per-channel contracts — channels created exploratively but never wired to anything that runs.

### Appetite

`medium` — couple of days. Channel restructure is mechanical (CLI exists); agent-slot scaffolding is per-channel `.claude/CLAUDE.md` + `contract.md`; one agent skeleton (Vérificateur baux_commerciaux) to validate the activation-surface principle end-to-end.

Appetite picked: `medium` — 14 channels × (create + 2 files each) + 1 agent dry-run is bounded but multi-step. Override with `--appetite=<size>`.

## Elements

Breadboard: places (channel dirs), affordances (CLI verbs + agent loop), connections (subscription + draft → outbox → stamp → publish).

- **Place — `~/.secretariat/orgs/themia.pro/channels/`** — root of the 14-channel tree, time-sharded sub-trees per existing layout (`docs/decisions/2026-05-12-substrate-layout-v03.md`).

- **Place — per-channel `.claude/`** — inherits up the tree (org-root → product-trunk → channel-leaf). Holds `CLAUDE.md` (agent role description), `skills/` (domain skills — e.g. `jurimetric-review-decision` symlinked into `module:baux_commerciaux/`), `agents/` (sub-agent definitions if needed).

- **Place — per-channel `contract.md`** — declares cadence floor, trust gate (signed-only | stamp-required), roster, preferred transports. Per `project_meta_channel_pattern` it lives in the channel directory; signed-envelope form lands in `_meta` when relay sync is real. Field names mirror `tech.equanimi.secretariat.channelContract` lexicon to minimize drift.

- **Affordance — `sec channels delete --org themia.pro <handle> --yes`** — prune the 23 channels being dropped. Exists today.

- **Affordance — `sec channels create --org themia.pro <handle> --name "<Display>" --description "..."`** — create the new channels (some renames of existing, some net-new). Exists today.

- **Affordance — agent role spec template** — one markdown shape reused per channel's `CLAUDE.md`: who am I, what do I read, what do I draft, what's my trust gate, when do I run (push-subscribe vs scheduled).

- **Affordance — one concrete agent dry-run** — `Vérificateur Veriguard pour baux_commerciaux` on `module:baux_commerciaux`. Smallest scope, existing skills (`jurimetria-lab:jurimetric-review-decision`, `jurimetria-lab:jurimetric-review-cohort`), existing Veriguard MCP tools (`obtenir_problemes_decision_baux_commerciaux`, `confirmer_annotation_baux_commerciaux`, etc.). Validates the activation surface: `cd <channel-dir> && claude` picks up role + skills + contract automatically.

- **Connection — the 14 channels (handle → display → home agents → trust gate):**

  **Cross-product (2)**

  | Handle | Display | Home agent | Trust gate |
  |---|---|---|---|
  | `general` | Général | Secrétariat (digest) | signed-only |
  | `clients` | Clients | Rédacteur clients | signed-only |

  **Themia jurimétrie product (5)**

  | Handle | Display | Home agents | Trust gate |
  |---|---|---|---|
  | `themia` | Themia | — (principals) | mixed |
  | `module:baux_commerciaux` | Module — Baux commerciaux | Principals + Vérificateur BC + Chercheur(BC) | signed-only |
  | `module:dommage_corporel` | Module — Dommage corporel | Principals + Vérificateur DC + Chercheur(DC) | signed-only |
  | `module:travail` | Module — Travail | Principals + Vérificateur + Chercheur | signed-only |
  | `module:cassation` | Module — Cassation | Principals + Vérificateur + Chercheur | signed-only |

  **Encyclopédie (1) — untouched**

  | Handle | Display | Home agent | Trust gate |
  |---|---|---|---|
  | `encyclopedie-jurimetrie` | Encyclopédie jurimétrie | — (defer to encyclopedia-product pitch) | signed-only |

  **Gouvernance (1)**

  | Handle | Display | Home agent | Trust gate |
  |---|---|---|---|
  | `assemblee_generale` | Assemblée générale | — (principal-authored PVs) | **stamp-required** |

  **Com (Slack-shaped, 5)**

  | Handle | Display | Home agent | Trust gate |
  |---|---|---|---|
  | `com:blog` | Com — Blog | Rédacteur com | signed-only |
  | `com:newsletter` | Com — Newsletter | Rédacteur com | signed-only |
  | `com:linkedin` | Com — LinkedIn | Rédacteur com | signed-only |
  | `com:webinaire` | Com — Webinaire | Rédacteur com | signed-only |
  | `com:landing-page` | Com — Landing | Rédacteur com | signed-only |

  **Ops (1)**

  | Handle | Display | Home agent | Trust gate |
  |---|---|---|---|
  | `ops:compta` | Ops — Compta | Compta | signed-only |

- **Connection — `module:<name>` collapses `data:` + `labo:` from earlier drafts.** Single channel per legal module hosts everything: principal-driven discussion (corpus health, schema, prioritization, ideas), agent-driven workflow (verification findings, hypothesis experiments), published rapports. Differentiation moves from channel-handle to **envelope stream tag** — see [[project-envelope-streams]] memory. Streams (illustrative, not locked) — `data`, `idea`, `decision`, `verification`, `experiment`, `rapport`, `triage`. Stream-tag design is its own follow-on pitch; this pitch only commits to `module:<name>` as the channel shape and notes streams as the differentiator for later.

- **Connection — Veriguard sidestepped at handle level:** Veriguard is a Themia *product* (verification MCP server) but only one *workflow* inside the module channels (alongside Chercheur experiments, triage, rapports). Channel handle = `module:*`, not `veriguard:*` or `labo:*`. Product brand stays as "Veriguard" in agent role descriptions (e.g. `Vérificateur Veriguard pour baux_commerciaux`). No product rename needed.

- **Connection — agent identity policy:** all Vérificateurs + Chercheur share ONE `did:key` (`agent:module@themia.pro`), distinguished by channel scope. Single key per agent role across modules. Compta / Rédacteurs each get own `did:key`. Secrétariat gets own `did:key`.

- **Connection — module slug convention:** `baux_commerciaux`, `dommage_corporel`, `travail`, `cassation` — snake_case underscores, matching Veriguard MCP tool suffixes (`analyser_insights_baux_commerciaux`, `obtenir_problemes_decision_dommage_corporel`). Same identifier across substrate + lab tools = zero ambiguity, grep-symmetric. `queue_handle.rs:126` allows underscores in segments. Tradeoff: verbose vs short — pay verbosity for tool-symmetry.

- **Connection — `assemblee_generale` channel + counter-stamp gap.** Stamp-required at channel level: every envelope is a stamped PV or formal resolution. Preparatory drafts (ordre du jour, convocations) live in `general`; only the voted+signed PV lands here. The channel is an archive, not a workspace. **Counter-stamp gap:** a real PV ideally carries multiple signatures (président + secrétaire de séance + scrutateurs); counter-stamp (m.3 process-verbaux model, AGENTS.md rule 4) is v0.4+. v0.3 single-stamp regime: président signs, body lists other attendees + roles inline. Themia's annual AG (June, SAS statutory) becomes the concrete v0.4 forcing function. See [[project-assemblee-generale-channel]].

- **Connection — channels dropped (23):** `random`, `competition`, `market`, `hiring`, `association`, `data-status`, `questions-clients`, `discussion:clients` (rename to bare `clients`), `discussion:acquereurs`, `analytics:*` (4 — fold into `general` / `ops:compta` as needed), `com:analytics` (fold into `general`), `ops:expenses|gpt-support|audit|finances` (4 — agent doesn't exist yet), `product:*` (9 — all replaced by `themia` trunk + `module:*`). Encyclopédie channel (`encyclopedie-jurimetrie`) survives untouched.

## Risks

### 🐇 Rabbit holes

- **Migrating the 2 envelopes in `product:data:baux-commerciaux` to `module:baux_commerciaux`.** Envelopes are signed — moving the file changes path but not signature. Need: a `sec channels move-envelopes <src> <dst>` verb, or a manual `mv` + re-index. Concrete decision: `mv` + re-index for the 2 envelopes; formalize the verb when ≥3 channels need it.
- **Per-channel agent identity provisioning.** Each agent needs a `did:key` keypair. `sec init` handles principal init but not per-agent. Need: a `sec agent init <role>` verb that mints keypair + writes `agents/<role>/key` + registers in org's roster. Concrete decision: defer the verb; for the dry-run, mint `agent:module@themia.pro` manually via `sec init --as agent:module` in a scratch dir, copy key into the org's `agents/module/key`. Productize the verb in the agent-runtime pitch.
- **Skill inheritance via `.claude/` tree walk.** Claude Code already walks `.claude/` up the directory tree. Need to verify it picks up skills from `org-root/.claude/skills/` when invoked inside `org-root/channels/module/baux_commerciaux/.claude/` — and that nested overrides work. Test with a stub skill before trusting it for Vérificateur BC.
- **Per-channel `contract.md` v0 shape.** Lexicon is `tech.equanimi.secretariat.channelContract` (signed envelope, v0.4+). For v0.3, plain markdown file with frontmatter (`cadence`, `trust_gate`, `roster`, `preferred_transports`) suffices. Risk: shape drifts before lexicon lands. Mitigate by mirroring lexicon field names exactly.
- **Stream tag set inevitably leaks into the dry-run.** Vérificateur drafts a finding to `module:baux_commerciaux` — what stream does it carry? Concrete decision: use `verification` for the dry-run's single finding; lock the full set in a separate stream-tag pitch. Stream metadata field on the envelope is fine to ship absent a finalized vocabulary — we already accept that envelopes can carry forward-compatible metadata.
- **`assemblee_generale` ships before counter-stamp.** v0.3 single-stamp regime means the first PV envelope will be signed by président only, with co-attendees listed inline in the body. When v0.4 counter-stamp lands, those same envelopes need to accept additional stamps without invalidating the original signature. Mitigation: design counter-stamp as additive (new signature records reference the original envelope hash; original envelope is immutable). Verify this is the intended v0.4 shape before drafting Themia's first AG PV under v0.3 (or risk having to re-stamp old PVs later).

### 🏴 Off-sides called

- **Implementing the 6 non-dry-run agents.** This pitch only builds slots + one dry-run (Vérificateur BC). Each remaining agent = own pitch (Secrétariat digest, Chercheur jurimétrique, Rédacteur clients, Rédacteur com, Compta, plus eventually Éditeur encyclopédie).
- **Encyclopedia product shaping.** `encyclopedie-jurimetrie` channel left untouched; no redaction/publie split; no Éditeur agent slot. Defer to encyclopedia-product pitch when wedge is real.
- **Stream-tag taxonomy.** This pitch defers the full stream vocabulary to follow-on work. Only commits to (a) one channel per module, (b) `stream` field will exist on envelopes, (c) `verification` is the dry-run stream value.
- **Relay sync for the new channels.** Channels work locally without relay; sync is orthogonal. Out.
- **`_meta` per product trunk** for skills/contracts/roster. Tempting but premature — start with `.claude/` walk; introduce `_meta` channels when version-controlled, federated skill sync is needed (v0.4+).
- **Veriguard product rename to "Vérigarde".** Brand decision orthogonal to channel structure. Sidestepped by using `module:*` handles. Defer to Christophe's brand call.

### 🥩 Fat cut

- **`themia` trunk subtree** (`themia:data`, `themia:produit`). Premature — single `themia` channel until cadence diverges.
- **`veriguard` / `labo` parent trunk channel** (above `module:*`). Cut — `module:` is the unit, no super-bucket needed.
- **Earlier `data:` + `labo:` split per module.** Cut — collapsed into single `module:<name>` channel; streams differentiate within.
- **Encyclopédie redaction/publie split.** Cut — encyclopedia channel survives as single existing handle.
- **`rapports` as own top-level trunk.** Cut — rapports are envelopes in module channels with `stream=rapport`.
- **Roster-as-signed-envelope.** v0.3 keeps roster as plain markdown frontmatter in `contract.md`. Signed `rosterUpdate` envelopes are v0.4+.
- **`com:analytics` as its own channel.** Folds into `general` — low traffic, no agent.

### 🧪 Domain knowledge

- **Verify with Christophe before locking handles:**
  - `module:` — anglicism but also French; alternatives `domaine:` (legal-domain), `matiere:` (legal-matter). `module` matches Themia's existing UI vocabulary for legal-area selectors.
  - Em-dash vs hyphen separator in display names (`Module — Baux commerciaux`). Themia copy norm check.
- **Verify with Christophe that one shared `agent:module` DID across modules + workflows is acceptable** — vs. distinct `agent:verificateur-bc`, `agent:chercheur`, etc. Signature provenance is per-envelope regardless; the question is whether downstream consumers want to attribute findings to "the module agent acting in BC scope" vs. "the BC verifier" specifically.
- **Confirm `sec channels list --org themia.pro --prefix module:` works as documented** — existing CLI grouping logic was built for the old tree; new prefix shape should just work but worth one test run.
- **`stream` metadata field on envelopes.** Verify envelope schema accepts forward-compatible metadata fields without breaking signature verification. Should already be the case (signatures cover canonical envelope JSON), but worth a verification round-trip in the dry-run.

## Pitch

### Problem

Themia has 36 channels created speculatively over the past weeks. Most have zero envelopes; the 2 that have any traffic (`product:data:baux-commerciaux`) are on the wrong handle for the future structure. None has a `.claude/` directory, a `contract.md`, or a named agent. Subsequent agent work — Vérificateur per module, Chercheur jurimétrique, Secrétariat digest — has nowhere to land: each agent needs a deterministic home directory (per `project_channel_dir_is_activation_surface`), and right now those homes are mixed-language, speculatively shaped, and unassigned. Building the agents on top of the current tree means rebuilding everything later. Building them on a curated foundation makes each agent a contained, single-pitch slice.

Concretely, the channel tree IS the agent-org chart. Right now it's a draft on a napkin. We need it to be a structure — with one channel per legal module (`module:<name>`, snake_case matching Veriguard MCP tool suffixes), envelope **streams** carrying the flow-within (data discussion, verification finding, experiment, rapport), Slack-shaped `com:`, the encyclopedia channel left alone, and `did:key` agent identities provisioned at the org level.

### The bet

For medium appetite (a couple of days), restructure Themia's channels to the 13-channel French tree above; create per-channel `.claude/CLAUDE.md` (agent role spec) + `contract.md` (cadence + trust gate + roster) for each; migrate the 2 BC envelopes to `module:baux_commerciaux`; mint the shared `agent:module@themia.pro` identity; and run one agent dry-run (Vérificateur Veriguard pour baux_commerciaux) end-to-end via `cd ~/.secretariat/orgs/themia.pro/channels/module/baux_commerciaux/ && claude` — proving the activation-surface principle holds: directory walk picks up role + skills + contract automatically, agent reads Veriguard MCP findings, drafts envelope (with `stream=verification`) to channel outbox, signs with `agent:module` key.

Pays off because every later agent-implementation pitch (5-6 remaining agents) becomes a small-or-medium contained bet rather than a tangled migration-plus-implementation knot. Front-loading the structural decisions once (one-channel-per-module, streams-for-flow, snake-case modules, shared agent DID, contract shape) beats re-litigating each one six times.

### No-gos

- No implementing the 5-6 non-dry-run agents — own pitches.
- No encyclopedia restructure or Éditeur agent — defer entirely.
- No stream-tag taxonomy locking — follow-on pitch.
- No relay sync work — orthogonal.
- No `_meta` signed-envelope contract migration — v0.4+.
- No `themia` / `module` / parent trunk channels until traffic demands them.
- No `sec agent init <role>` CLI verb — manual key provisioning for the BC dry-run; productize in the agent-runtime pitch.
- No envelope-migration CLI verb — `mv` + re-index for the 2 BC envelopes; formalize when ≥3 channels need it.
- No Veriguard product rename — orthogonal brand decision; sidestepped by `module:*` handle.

## Launch checklist

Pre-flight (require Christophe input):

- [ ] Lock `module:` vs alternatives (`domaine:`, `matiere:`) — handle prefix
- [ ] Confirm shared `agent:module` DID acceptable (vs per-module DIDs)
- [ ] Confirm display-name typography (em-dash separator)

Execution (after lock):

- [ ] Delete the 23 dropped channels via `sec channels delete --org themia.pro --yes`
- [ ] Create the 14 new channels via `sec channels create --org themia.pro <handle> --name "<Display>"`
- [ ] Migrate 2 envelopes: `mv` files from `product:data:baux-commerciaux/envelopes/` → `module:baux_commerciaux/envelopes/`; re-index
- [ ] Mint `agent:module@themia.pro` keypair in scratch; copy key to `~/.secretariat/orgs/themia.pro/agents/module/key`
- [ ] For each channel, write `.claude/CLAUDE.md` (role spec) + `contract.md` (cadence/trust/roster)
- [ ] Test `.claude/` inheritance: stub skill at org-root, verify reachable from `module/baux_commerciaux/`
- [ ] Symlink `jurimetria-lab:jurimetric-review-decision` + `jurimetric-review-cohort` into `module/baux_commerciaux/.claude/skills/`
- [ ] Dry-run: `cd module/baux_commerciaux && claude` → verify role spec loads, skills available, Veriguard MCP reachable
- [ ] Have the agent draft one verification finding (with `stream=verification`) to the channel outbox; principal stamps; verify signature + stamp chain
- [ ] Document outcome in `docs/milestones/2026-05-12-themia-channels-agents-foundation.md`

Stop-conditions (circuit breaker):

- If `.claude/` inheritance doesn't walk org-root → halt, re-pitch with explicit `skills/` symlink per channel.
- If shared `agent:module` DID causes downstream attribution confusion → halt, split into per-role DIDs, re-pitch.
- If channel-creation discovers handle-validation rejection (e.g. underscore + colon combination) → halt, fix `queue_handle.rs`, resume.
- If `stream` metadata field breaks signature verification → halt, design stream tag as separate signed sub-envelope, re-pitch.
