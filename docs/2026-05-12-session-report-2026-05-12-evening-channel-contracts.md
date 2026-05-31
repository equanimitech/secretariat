---
migrated_from: equanimi.tech/project/secretariat/dev/20260512T144832Z-7w7ejk.md
---
# Session report — 2026-05-12 evening — channel-contracts pitch landed end-to-end

Continuation of the earlier capture; the full arc through slices 1a → 5 closed in one session.

## Mid-session conceptual sharpenings (saved to memory)

Two pushbacks from the user reshaped the design mid-flight:

1. **Contracts attach to queues, not pair-relationships.** No separate "bilateral contract primitive." A queue has a roster; cardinality 1/2/N changes shape, not primitive. Saved to [[project-contracts-attach-to-queues]].
2. **Contracts are private consumption, not shared governance.** `<channel-dir>/contract.md` was conflating governance fields (roster, demanded trust) with consumption fields (my poll floor, my filters). Refactored: governance fields dropped; remaining fields explicitly private; renamed file to `contract.local.md` to mirror Claude Code's `CLAUDE.md` / `CLAUDE.local.md` visibility convention. The `.local` suffix is now load-bearing. Saved to [[project-consumption-vs-governance]].

## Code commits (in order)

- `390b11e feat(core): ChannelContract value object + contract.md storage; auto-scaffold on create_channel (slice 1a)`
- `7f0fb58 refactor(core): split contracts — <channel-dir>/contract.md is consumption-only; org-root auto-scaffold (slice 1b)`
- `ee92e32 feat(core): consumption-contract get/set use cases + rename to contract.local.md (slice 2 — application layer)`
- `fe1e350 feat(cli,mcp): contract get/set verbs for channels + orgs (slice 2 — surfaces)`
- `068f152 feat(core,cli,mcp): accumulate resolver — org-root → ancestors → leaf (slice 3)`

(Plus an unrelated `645ba1a feat(core): optional reply_to: DocHash on envelope for threading` that landed in the middle from earlier work in the tree — not part of this session.)

## Surfaces shipped

**CLI** (under `sec channels contract` + `sec orgs contract`):
- `sec channels contract get <handle> [--org A]`
- `sec channels contract set <handle> [--org A] [--cadence-floor-minutes N|--clear-cadence] [--min-trust signed-only|stamp-required|--clear-min-trust]`
- `sec channels contract resolve <handle> [--org A]`
- `sec orgs contract get <alias>`
- `sec orgs contract set <alias> [...]`

**MCP** (5 new tools):
- `get_channel_contract(handle, org?)`
- `set_channel_contract(handle, org?, cadence_floor_minutes?, min_trust?, clear?)`
- `resolve_channel_contract(handle, org?)`
- `get_org_contract(org)`
- `set_org_contract(org, cadence_floor_minutes?, min_trust?, clear?)`

## Themia + EquanimiTech anchor contracts (slice 4 — applied to live substrate)

| Scope | `cadence_floor_minutes` | `min_trust` |
|---|---|---|
| `themia.pro` org-root | 15 | `signed-only` |
| `channel:clients` (themia.pro) | 60 | (inherit signed-only) |
| `channel:assemblee_generale` (themia.pro) | (inherit 15) | `stamp-required` |
| `equanimi.tech` org-root | 30 | (none) |
| all other channels | inherit org-root | inherit org-root |

Verified accumulated views via `sec channels contract resolve`:
- `channel:clients` → cadence MAX(15, 60) = 60; min_trust signed-only
- `channel:assemblee_generale` → cadence 15; min_trust MAX-RESTRICTIVE(signed-only, stamp-required) = stamp-required
- `channel:dev:leggia` → pure org inheritance: 15 + signed-only

## Slice 5 — regenerated `module:baux_commerciaux/contract.local.md`

Hand-scaffolded version carried governance-shaped fields (`roster`, `trust_gate`, `inherit_from_parent`) that are now ignored on load. Replaced with the clean consumption stub; pure inheritance from org-root now applies (15 + signed-only). The role-spec `.claude/CLAUDE.md` (Vérificateur Veriguard agent instructions) was preserved untouched — that's a Claude Code activation artifact, orthogonal to the contract.

## Test + clippy state

- 299 tests pass workspace-wide (up from 280 pre-session)
- 18 new tests added for contract domain + store + ops + resolver
- `cargo clippy --workspace --all-targets -- -D warnings` clean

## v0.3 substrate state

- 25 channels, all with `contract.local.md`
- 2 orgs, both with `<org-dir>/contract.local.md`
- 27 contract files total on disk
- `bare contract.md` reserved for future channel-governance artifact (when `assemblee_generale`-style enforcement demands shared/signed policy beyond receiver-side filtering)

## What's now possible that wasn't this morning

- Principal can declare per-channel consumption preferences via MCP (Claude can read these tools and respect them when surfacing inbox traffic).
- The daemon's future routing engine ([[project-daemon-v03-subsystems]] RoutingEngine) has structured consumption rules to consult per channel — not just an unstructured prose hint in a description field.
- Accumulate semantics give the principal one declaration point per scope (org, trunk, leaf) instead of repeating preferences on every channel.

## Still pending (separate slices, future sessions)

- **Channel governance artifact** — `bare contract.md` (or `.channelDef` extension) carrying shared roster + channel-wide artifact policy. Needed when multi-principal channels go beyond Rafa-solo and when `assemblee_generale` needs server-side stamp-required enforcement, not just receiver-side filtering.
- **Role-spec MCP verbs** — `set_channel_role_spec` / `clear_channel_role_spec` for `<channel-dir>/.claude/CLAUDE.md`. Sketched in the pitch; descoped from this session's bite.
- **Repo → channel linking + daily reports** — `.secretariat-channel` marker + ScheduleTicker walk. Sketched in [[project-post-reset-todos]].
- **Per-reader review-state cursors** — daemon open-question doc shipped; design TBD.
- **~70 captures from 2026-05-06+ in `inbox/triage`** — never roundtable'd.

## Conceptual takeaway

The session's most valuable artifact isn't any one commit — it's the [[project-consumption-vs-governance]] separation. Two pushbacks ("invitations don't make my contract different from others'" → "but contracts are private approaches, no?") forced the correct decomposition before more code accreted on the wrong axis. `.local` suffix carries the invariant in the filename. The bare `contract.md` slot is now reserved for the right thing.
