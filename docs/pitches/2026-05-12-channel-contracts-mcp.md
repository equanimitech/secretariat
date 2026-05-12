# Channel contracts + role specs via MCP — scaffold on create, accumulate semantics

Pitch — 2026-05-12 (revised). Source: free-text exchange during channel-restructure session — "we need MCP tools to set our contracts with each (at different nested levels)" + follow-up "shouldn't the scaffolding happen when we create / modify channel? these should be easily changeable no?" + "contract.mds (like Claude mds should) accumulate."

**Hard dependency:** v0.3.0 orgs + channels substrate (shipped 2026-05-12 in `9c38404 release(v0.3.0)`). Supersedes the narrower first draft of this pitch from earlier today.

## Boundaries

### Job to be done

As the principal of an org with a multi-level channel tree (e.g. `themia.pro` with `module:baux_commerciaux`, `module:dommage_corporel`, `assemblee_generale`, `com:*`), I want:

1. **Channel creation to provision artifacts automatically** — `create_channel` should write a sensible `contract.md` (and optionally `.claude/CLAUDE.md` for role specs) at the same moment the channel directory is created. No second-step scaffolding ritual.
2. **MCP tools to edit those artifacts** — `set_channel_contract` / `set_channel_role_spec` / `clear_channel_role_spec`, mirrored at the CLI.
3. **Contracts to ACCUMULATE up the tree like CLAUDE.md** — org-root → trunk → leaf stack. Roster unions; cadence + trust-gate take the most-restrictive value. Sovereignty flows top-down; children can only narrow, not widen.

Today's baseline: contracts are a *description in the channel-create call's `description` field*. Nothing is enforceable, nothing is inherited, nothing is editable as a structured artifact. The current 15-channel Themia tree has zero `contract.md` files, one hand-scaffolded `module:baux_commerciaux/contract.md` + `.claude/CLAUDE.md` from earlier this session (which now needs deletion + regeneration once the create-flow lands).

### Appetite

`big` — full week. Wider than the original draft because it now also covers (a) auto-scaffold on create, (b) role-spec MCP verbs, (c) accumulate-semantics resolver, (d) CLI parity for everything. The win: end-state primitives that don't need a second pitch.

Appetite picked: `big` — 1 domain object + storage + accumulate resolver + 3 new MCP tools + revised `create_channel` + matching CLI + tests covering the merge rules. Override with `--appetite=<size>`.

## Elements

Breadboard: places (where artifacts live), affordances (verbs), connections (accumulate walk).

- **Place — `<channel-dir>/contract.md`** — per-channel structured contract. YAML frontmatter holds typed fields mirroring `tech.equanimi.secretariat.channelContract` lexicon names. Free-form markdown body holds the prose ("why this cadence?", "house rules"). Auto-generated on `create_channel`.

  ```yaml
  ---
  $type: tech.equanimi.secretariat.channelContract
  channel: channel:module:baux_commerciaux
  org: themia.pro
  cadence_floor_minutes: 15
  trust_gate: signed-only          # | stamp-required
  roster:
    - did:web:rafa.equanimi.tech
  preferred_transports:
    - relay:themia.pro
  ---
  ```

- **Place — `<org-dir>/contract.md`** — org-root contract. Top of the accumulate chain. Same shape as channel contracts. Empty handle = org-root in MCP calls.

- **Place — `<channel-dir>/.claude/CLAUDE.md`** — optional role spec for channels with a designated home agent. Auto-generated *only when* `create_channel` is called with `role_spec` or when `set_channel_role_spec` is invoked. Otherwise absent; channel inherits `.claude/` walk from ancestors per Claude Code's existing convention.

- **Affordance — extended `mcp__secretariat__create_channel`**:

  ```
  create_channel(
    org?: string,
    handle: string,
    name: string,
    description: string,
    contract?: ContractDto,      // NEW: structured contract; if absent, write
                                  // a minimal contract.md with `inherit-only` shape
    role_spec?: string            // NEW: markdown body for .claude/CLAUDE.md;
                                  // if absent, no .claude/ dir is created
  ) → { handle, created_at, scaffolded: [list of artifact paths written] }
  ```

  Backwards-compat: existing callers pass only `name + description`, work unchanged. New callers opt into structured scaffold.

- **Affordance — `mcp__secretariat__set_channel_contract`**:

  ```
  set_channel_contract(
    org?: string,
    handle: string,                // "" for org-root contract
    contract: ContractDto,         // partial — empty fields = leave untouched
    replace: bool = false          // false = field-level merge; true = full overwrite
  ) → { handle, applied: ContractDto }
  ```

- **Affordance — `mcp__secretariat__get_channel_contract`**:

  ```
  get_channel_contract(
    org?: string,
    handle: string,                // "" for org-root
    resolved: bool = true          // true = accumulate up the tree; false = just this level
  ) → {
    handle,
    own: ContractDto,                  // just this channel's overrides
    accumulated: ContractDto,          // merged view (only when resolved=true)
    chain: [string]                    // ["", "module", "module:baux_commerciaux"]
  }
  ```

- **Affordance — `mcp__secretariat__set_channel_role_spec`**:

  ```
  set_channel_role_spec(
    org?: string,
    handle: string,
    spec: string                      // markdown body for .claude/CLAUDE.md
  ) → { handle, file_path }
  ```

  Creates `.claude/` dir if missing; writes/overwrites `CLAUDE.md`. Channel must already exist.

- **Affordance — `mcp__secretariat__clear_channel_role_spec`**:

  ```
  clear_channel_role_spec(
    org?: string,
    handle: string,
    confirm: bool
  ) → { handle, removed }
  ```

  Removes `.claude/CLAUDE.md` and `.claude/` dir if now empty. Channel survives. (Tree-walk inheritance from ancestors resumes.)

- **Affordance — CLI parity:**

  ```
  sec channels create <handle> [--org <alias>] [--name ...] [--description ...]
                                [--contract-file <path>] [--role-spec-file <path>]
  sec channels contract set <handle> [--org <alias>] [--cadence-floor-minutes N]
                                     [--trust-gate signed-only|stamp-required]
                                     [--roster <did>...] [--preferred-transports <uri>...]
                                     [--replace]
  sec channels contract get <handle> [--org <alias>] [--resolved/--own-only]
  sec channels role-spec set <handle> [--org <alias>] --file <path>
  sec channels role-spec clear <handle> [--org <alias>] --yes
  ```

  Per AGENTS.md rule: every principal-facing primitive ships on both interfaces.

- **Connection — accumulate semantics (the key insight):**

  For `handle = module:baux_commerciaux` in org `themia.pro`, resolver walks:

  ```
  1. ~/.secretariat/orgs/themia.pro/contract.md           (org-root)
  2. ~/.secretariat/orgs/themia.pro/channels/module/contract.md   (trunk; may not exist — ok)
  3. ~/.secretariat/orgs/themia.pro/channels/module/baux_commerciaux/contract.md  (leaf)
  ```

  Each level contributes per field type per [[project-contracts-accumulate]] memory:

  | Field | Merge rule | Rationale |
  |---|---|---|
  | `cadence_floor_minutes` | MAX | Larger floor = more restrictive; can tighten down |
  | `trust_gate` | MAX-RESTRICTIVE (stamp-required > signed-only) | Sovereignty flows top-down |
  | `roster` | UNION | Adding members is monotonic |
  | `preferred_transports` | UNION | More transport options OK |

  No `inherit_from_parent` flag — accumulate is the only model. (`assemblee_generale: stamp-required` works naturally even with org-root at `signed-only`: max-restrictive wins.)

- **Connection — five anchor contracts to set for Themia after pitch lands:**

  | Level | Override |
  |---|---|
  | `themia.pro` (org root) | `cadence_floor_minutes: 15`, `trust_gate: signed-only`, `roster: [rafa]` (christophe + agent DIDs added when minted) |
  | `assemblee_generale` | `trust_gate: stamp-required` (accumulate-max raises org's signed-only to stamp-required for this channel) |
  | `module:*` (trunk, only if needed) | `roster: [agent:module@themia.pro]` (agent gets write rights across all module:* leaves via union) |
  | `module:baux_commerciaux` | (none — pure accumulate from trunk) |
  | `clients` | `cadence_floor_minutes: 60` (max with org's 15 = 60; this channel polls slower) |

## Risks

### 🐇 Rabbit holes

- **`assemblee_generale` trust-gate accumulate edge case.** Org-root signed-only + leaf stamp-required → max-restrictive picks stamp-required. ✓ But what if a sibling channel wants signed-only and is *parented* under a trunk that says stamp-required? Children can't loosen — they're stuck at stamp-required even if it's wrong. Concrete decision: design says no — security property. If a channel genuinely needs laxer access than its parent, it should be reparented (different namespace), not bypass the inheritance rule. Document the constraint explicitly so principals don't get surprised.
- **Roster monotonicity on relay disconnect.** When a principal leaves a channel, "remove from roster" is the operation — but roster accumulates. Removing at the leaf doesn't propagate up. Concrete decision: roster removal happens at the level where the principal was *added*; lower levels of the chain don't shadow it. Loaders walk top-down + handle explicit `roster_remove: [<did>...]` field in v0.4+. v0.3 = additive only; deal with `did:key` revocation separately.
- **Auto-scaffold on `create_channel` with empty contract.** What does the `contract.md` look like if the caller passed no contract? Concrete decision: write an empty frontmatter (`---\n---\n`) + a short prose header explaining inheritance. Resolver treats empty frontmatter as "contribute nothing to merge."
- **Merge-rule consistency across field types.** Easy for current 4 fields; gets thorny as fields multiply (e.g. v0.4 adds `attention_envelope`, `notification_rules`). Concrete decision: every new field declares its merge rule in the lexicon at the moment it's added; resolver consults that.

### 🏴 Off-sides called

- **Signed contract envelopes.** Lexicon is drafted; signed-envelope shape is v0.4+. v0.3 = plain `contract.md` files. Out.
- **GUI for contract editing.** Per the recently-sharpened [[project-mcp-is-primary-interface]] memory: UI navigates, MCP does CRUD. Contract editing is CRUD → MCP only for v0.3.
- **Per-stream contract overrides.** "stream=verification has different cadence than stream=data within same channel." Defer until streams have real traffic.
- **Contract change history.** v0.3 = overwrite-in-place; principal can `git init ~/.secretariat/` if they want history. v0.4+ moves to signed envelopes which carry their own history.
- **Reparenting channels.** "Move `assemblee_generale` from `gouvernance:` trunk to bare root" — orthogonal feature, defer.
- **Multi-principal subscriber-side resolution.** v0.3 is single-principal-per-org; defer subscriber-side contract caching to v0.4.

### 🥩 Fat cut

- **`inherit_from_parent` flag.** Accumulate replaces the override-vs-inherit dichotomy. The flag becomes meaningless; remove from the design.
- **`replace: true` on `set_channel_contract`.** Tempting to support full-replace semantics; but accumulate-merge is the only design, so "replace this channel's contract" = "overwrite this channel's `contract.md`"; field-level merge across the chain still applies. Keep `replace: true` but document it only affects THIS channel's file, not the accumulated view.
- **Per-channel contract migration tooling.** "When org-root changes, replay against descendants." Useful eventually; out for v0.3.
- **Role-spec inheritance.** `.claude/CLAUDE.md` already inherits via Claude Code's tree-walk; substrate doesn't need its own resolver. Cut.

### 🧪 Domain knowledge

- **Verify with Christophe:**
  - Trust gate for `assemblee_generale` (stamp-required confirmed).
  - Roster shape — start with `[rafa]` only? Add christophe when his DID exists in substrate. Add `agent:module@themia.pro` to `module:*` trunk after key mint.
  - Cadence floor for `clients` — 60 minutes? Tighter for legal-deadline channels?
- **Confirm `cadence_floor_minutes` is the right grain.** Could be `cadence_floor: "PT15M"` (ISO 8601 duration). Pick before locking the lexicon.
- **Verify MAX-RESTRICTIVE for trust_gate isn't surprising.** Sovereignty-flows-top-down is a security property, but principals might expect "child overrides parent" intuitively. Document explicitly + flag in the principal-facing CLI docs.
- **Lexicon merge-rule annotation.** Should `tech.equanimi.secretariat.channelContract` carry per-field merge rule annotations? Concrete decision: yes — add `x-merge: max | union | max-restrictive | last-wins` to each field's JSON Schema so the resolver can read them generically. Future-proofs against new fields.

## Pitch

### Problem

After the Themia channels restructure landed 15 channels in v0.3.0, contracts are *promises in description fields*, not enforceable artifacts. Today, `assemblee_generale`'s "STAMP-REQUIRED at channel level" lives in the channel's description text — not in any structured form the daemon could enforce. The 14 other channels share defaults that ought to be inherited from an org-root contract, but no org-root contract exists either.

Worse: scaffolding contracts post-hoc is a maintenance trap. The one channel scaffolded earlier this session (`module:baux_commerciaux/contract.md` + `.claude/CLAUDE.md`) was hand-written — the other 13 stayed blank. As soon as a second principal (Christophe) joins, the discrepancy between "what we say the contract is" and "what's encoded" becomes a real risk.

The user-driven insight from the conversation: **(a) scaffolding should happen at `create_channel` time, not as a second ritual; (b) contracts should ACCUMULATE up the tree like CLAUDE.md files, not override-precedence.** Both reshape the design.

Concretely, after this lands: `create_channel(org=themia.pro, handle=channel:foo, name="Foo", contract={cadence_floor_minutes: 30})` writes the channel dir + `contract.md` + (if role-spec passed) `.claude/CLAUDE.md` atomically. `set_channel_contract` edits the artifact. `get_channel_contract(resolved=true)` walks org-root → trunk → leaf and returns the merged view per accumulate rules. No second-step rituals.

### The bet

For big appetite (a full week), ship:

1. `ContractDef` value object in `crates/core/src/domain/` mirroring the lexicon
2. `contract.md` read/write in `crates/core/src/infrastructure/contract_store.rs` with per-field merge rules
3. Accumulate resolver (`load_resolved_contract`) walking org-root → leaf
4. Extended `create_channel` accepting optional `contract` + `role_spec`; auto-scaffolds `contract.md` (always) + `.claude/CLAUDE.md` (when role-spec passed)
5. Four new MCP tools: `set_channel_contract`, `get_channel_contract`, `set_channel_role_spec`, `clear_channel_role_spec`
6. CLI parity: `sec channels contract set|get`, `sec channels role-spec set|clear`, extended `sec channels create`
7. Tests: accumulate walk with 0/1/2/3-level chains; max-restrictive trust-gate; roster union; auto-scaffold on create with/without explicit contract; role-spec opt-in
8. Migration: delete the one hand-scaffolded `module:baux_commerciaux/{contract.md, .claude/}`, recreate via the new path for parity

Pays off because every later agent-implementation pitch (Vérificateur per module, Chercheur jurimétrique, Éditeur jurimetria, Compta, Rédacteurs) needs contract enforcement (trust gate, roster, cadence) — front-loading this once beats wiring each agent to a different contract-fetch path. Accumulate semantics + auto-scaffold means principals stop hand-writing contracts; they describe overrides at the level that matters and resolution takes care of the rest. Field names mirror the v0.4 signed-envelope lexicon → zero migration churn when contracts become envelopes.

### No-gos

- No signed-envelope contracts — `channelContract` lexicon is drafted but unbacked at runtime in v0.3.
- No Tauri contract-editor GUI — UI navigates, MCP does CRUD ([[project-mcp-is-primary-interface]]).
- No per-stream contract overrides — defer until streams ship traffic.
- No contract change history — overwrite-in-place; principal's git is the escape hatch.
- No roster-as-signed-update — `rosterUpdate` lexicon for v0.4+.
- No multi-principal subscriber-side resolution — v0.3 single-principal-per-org; defer.
- No `inherit_from_parent` flag — accumulate is the only model; flag would re-invite override-vs-inherit confusion.
- No role-spec inheritance resolver — Claude Code's tree-walk already handles it; don't reinvent.
- No reparenting — orthogonal.

## Launch checklist

Pre-flight (need Christophe input):

- [ ] Confirm `cadence_floor_minutes` grain (minutes integer vs ISO 8601 duration)
- [ ] Confirm MAX-RESTRICTIVE trust-gate isn't surprising (vs "leaf-overrides")
- [ ] Confirm starter roster shape (`[rafa]` solo or `[rafa, christophe]` once his DID exists)
- [ ] Confirm `clients` cadence (60 min default OK?)

Execution (after lock):

- [ ] Domain: `ContractDef` value object + per-field merge-rule registry
- [ ] Infrastructure: `contract_store.rs` (read/write/resolve)
- [ ] Application: `set_channel_contract`, `get_channel_contract`, `set_channel_role_spec`, `clear_channel_role_spec` use cases
- [ ] MCP: 4 new tools + extended `create_channel`
- [ ] CLI: parity verbs
- [ ] Tests: accumulate semantics, auto-scaffold, role-spec opt-in
- [ ] Delete hand-scaffolded `module:baux_commerciaux/{contract.md, .claude/}` → recreate via new flow
- [ ] Set Themia anchor contracts:
  - `themia.pro` org-root: `cadence_floor_minutes: 15`, `trust_gate: signed-only`, `roster: [<rafa-did>]`
  - `assemblee_generale`: `trust_gate: stamp-required`
  - `clients`: `cadence_floor_minutes: 60`
- [ ] Verify accumulated contract for each of 15 channels via `get_channel_contract(resolved=true)`
- [ ] Document accumulate semantics in `AGENTS.md` rule (probably extending #6 or new #11)

Stop-conditions (circuit breaker):

- If max-restrictive trust-gate genuinely confuses principals → halt, reconsider (maybe explicit `override_chain: true` field for surgical exceptions).
- If lexicon-per-field merge-rule annotation gets messy → halt, hard-code rules in resolver for v0.3, plan for declarative migration in v0.4.
