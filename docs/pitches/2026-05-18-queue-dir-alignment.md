# queue_dir alignment — one shape across self/org/peer

Pitch — 2026-05-18. Filed against the post-v0.6.0 layout, where slice 2
collapsed `_self/` and `orgs/<alias>/` into the same primitive
(queue-root with `channels/<segs>/` underneath) but left **peer DM
queues on the pre-collapse layout** because peer/contact primitive
collapse was explicitly out of slice 2's scope.

Today, three storage shapes for envelopes:

```
~/.secretariat/
├── _self/channels/<segs>/envelopes/...          ← slice 2 ✓
├── orgs/<alias>/channels/<segs>/envelopes/...   ← slice 2 ✓
└── <peer-alias>/<handle-segs>/envelopes/...     ← still pre-collapse
```

The first two read the same way. The third doesn't — no
`channels/` intermediary; handle segments map directly to depth.
`queue_dir.rs::queue_dir` returns the peer-shaped path; everything
else returns the channel-shaped path. The substrate has two storage
metaphors for one primitive.

Per [[project_contracts_attach_to_queues]]: there is no separate
"bilateral contract primitive" — a queue has a roster; **cardinality
1/2/N changes shape, not primitive**. A DM is a 2-roster channel. The
storage layout should reflect that.

## Boundaries

### Job to be done

As a developer or a principal walking the vault, I want **every
envelope-bearing directory to live at the same depth under the same
shape**: `<root>/<alias>/channels/<segs>/envelopes/YYYY/MM/DD/<ts>.md`,
where `<alias>` ∈ {`_self`, `<org>`, `<peer>`}. So that:

- One resolver works for any recipient (self, org, peer).
- Per-channel `template.md` / `contract.local.md` overrides can land
  at `<alias>/channels/<segs>/template.md` regardless of alias type
  — unblocks the deferred per-channel template slice.
- The conceptual model is "queue-root with channels under it" with
  no exceptions.
- Future relay sync for DMs (peer-to-peer publishing) reuses the
  same path layout the org-channel sync will use.

_When_: every time I open a peer's queue directory and notice
`~/.secretariat/marcelo/inbox/` doesn't have a `channels/` layer in
between, my mental model breaks. The asymmetry forces
`queue_dir.rs::queue_dir` to keep a separate code path from the
channel-dir resolver in `channel_def_store.rs::channel_dir`. Two
resolvers, two metaphors. One should win.

### Appetite

`small`. ~one focused day, similar profile to slice 3.

Touches:

- `queue_dir.rs::queue_dir` — collapse peer path to insert `channels/`.
- `sync.rs:236` (and any other callers of `queue_dir`) — unchanged
  semantics, just new path.
- Migration: peer-queue directories `mv`'d from `<peer-alias>/<segs>/`
  to `<peer-alias>/channels/<segs>/`.
- `invite_ops.rs` (or whatever sets up the initial peer queue) — write
  to new layout from day one for fresh installs.
- Tests in `queue_dir.rs` updated for new path expectations.
- AGENTS.md rule mentions of peer queue paths refreshed.

No domain logic moves. The `(owner_did, handle)` wire address is
unchanged. Only the on-disk storage location shifts.

### What's in scope

**New layout:**

```
~/.secretariat/
├── _self/channels/<segs>/envelopes/...           ← unchanged
├── orgs/<alias>/channels/<segs>/envelopes/...    ← unchanged
└── <peer-alias>/channels/<segs>/envelopes/...    ← was: <peer-alias>/<segs>/
```

Example mappings:

- `marcelo/inbox/envelopes/...` → `marcelo/channels/inbox/envelopes/...`
- `christophe/inbox/default/envelopes/...` → `christophe/channels/inbox/default/envelopes/...`
- `did_key_z6mk.../inbox/envelopes/...` → `did_key_z6mk.../channels/inbox/envelopes/...`

Same logic for `outbox/` and `_ciphertext/` — all three sit under
`<alias>/channels/<segs>/`.

- `queue_dir(aliases, recipient, root)` returns
  `<root>/<alias>/channels/<segs>/`.
- `envelopes_dir`, `outbox_dir`, `ciphertext_dir` compose unchanged.
- Migration: hand-script step that walks `<root>/*` and for each
  non-reserved alias dir (`!= _self`, `!= orgs`, `!= bin`, etc.)
  inserts a `channels/` layer. `mv`-only.
- Tests in `queue_dir.rs::tests` updated for new path shape.

### What's out of scope

- Roster-as-first-class primitive. The "DM = 2-roster channel"
  framing motivates the storage alignment but doesn't ship the
  governance refactor here — peer queues stay implicit-2-roster, no
  explicit `roster.md` or `channel.md` per-peer-queue. Adding those
  is a future wedge (parallels org channel governance).
- Per-peer `channel.md` for the DM queue. Could in principle write
  one at first-invite, but DM queues have no `name` / `description`
  fields the principal would author. Defer.
- Per-peer `contract.local.md` for DM cadence. Today's contract
  accumulate-resolver walks `<root>/<channels>/...` already; once
  the peer queue is at `<peer-alias>/channels/<segs>/` it
  participates automatically. A future slice could add per-peer
  contract scoping; this slice just unblocks the path.
- `<peer-alias>/identity.md` mirroring `_self/identity.md` — pulling
  in the peer's resolved DID document. Future. Today, peer identity
  lives in `_self/contacts.md` (slice 4); peer-alias dir is queue
  storage only.
- Multi-handle DMs (`<peer>/<handle-1>` and `<peer>/<handle-2>` for
  the same peer). Already supported by the wire shape; the layout
  collapse doesn't change support.

## Elements

### 1. New `queue_dir` resolver

```rust
pub fn queue_dir(aliases: &AliasMap, recipient: &Recipient, root: &Path) -> PathBuf {
    let alias = aliases.alias_for(&recipient.owner);
    let mut dir = root.join(alias);
    dir.push("channels");                            // ← new
    for seg in recipient.handle.segments() {
        dir.push(seg);
    }
    dir
}
```

One line added (`dir.push("channels")`). The resolver becomes
indistinguishable from `channel_def_store::channel_dir(channels_root, handle)`
when `channels_root` is `<root>/<alias>/channels/`. Two resolvers
collapse into one in spirit (they remain separate functions because
the call sites have different shapes — one takes a channels root,
the other takes (aliases, recipient, root) — but they emit identical
shapes).

### 2. Optional: unify the resolvers

After this slice, `channel_def_store::channel_dir` and `queue_dir::queue_dir`
return the same shape. Could be unified into a single
`resolve_envelope_dir(vault_root, alias_or_root, handle)` API.

**Recommended:** don't unify in this slice. Two callers, two shapes
of input, two functions is fine. Premature consolidation. Land the
layout alignment first; consolidate the API only if a third caller
appears.

### 3. Migration

Append to the migration script:

```bash
# ---- queue_dir alignment — peer queues gain channels/ layer ----
# Walk top-level dirs. Skip reserved names (_self, orgs, bin, peers,
# logs, .archive, .runtime). For each remaining dir (= peer alias),
# if it has subdirs that look like queue handles (not `channels/`),
# move them under a new `channels/` layer.

for peer_dir in "$VAULT"/*/; do
  [[ -d "$peer_dir" ]] || continue
  name="$(basename "$peer_dir")"
  case "$name" in
    _self|orgs|bin|peers|logs|.archive|.runtime) continue ;;
  esac

  # Already migrated?
  [[ -d "$peer_dir/channels" ]] && continue

  # Are there any queue-looking subdirs to move?
  shopt -s nullglob
  to_move=("$peer_dir"*/)
  shopt -u nullglob
  [[ ${#to_move[@]} -eq 0 ]] && continue

  echo "[migrate] $name: collapsing peer queues into channels/"
  mkdir -p "$peer_dir/channels"
  for sub in "${to_move[@]}"; do
    sub_name="$(basename "$sub")"
    [[ "$sub_name" == "channels" ]] && continue
    mv "$sub" "$peer_dir/channels/$sub_name"
  done
done
```

`mv` only. Pre-flight `tar` snapshot already covers rollback (existing
script behavior). Post-move envelope count gate identical to slice 2.

### 4. Per-channel template + contract resolution

After this slice:

- `<peer-alias>/channels/<segs>/template.md` works as an outgoing
  envelope template override for DMs to that peer in that handle.
- `<peer-alias>/channels/<segs>/contract.local.md` works as a per-DM
  consumption contract (cadence floor for surfacing this peer's
  envelopes, trust filter, prose for the agent).
- `compose_envelope` checks `<channel-dir>/template.md` (computed via
  `queue_dir(aliases, recipient, root)`) before falling back to
  `<root>/template.md`. The per-channel template override slice
  (deferred from v0.6.0) ships in the same release.

This is the slice's main payoff — unblocks per-channel template
without inventing a separate resolver for "what counts as a channel".

### 5. AliasMap unchanged

`AliasMap` already resolves `recipient.owner DID → alias` for self
(`_self`), known peers (display-name slug), known orgs (`OrgAlias`),
and unknown DIDs (sanitized `did_key_...` fallback). All four cases
sit at the same depth under `<root>/<alias>/channels/<segs>/`. No
changes to AliasMap.

## Risks

### 🐇 Rabbit holes

- **Reserved-name list for the migration script.** The "skip these
  top-level dirs" list must enumerate every non-peer dir at the
  vault root. Missing one means the script tries to wrap it in
  `channels/` (bad). Mitigation: enumerate explicitly + abort if a
  top-level dir doesn't match peer-alias grammar (DNS-label-shaped
  OR `did_key_...` sanitized form). Don't silently wrap unknown
  dirs.

- **Peer alias collisions.** What if a peer's display-name slug
  collides with a reserved name (`bin`, `peers`)? Today the
  AliasMap doesn't validate; the principal's display-name choice
  picks the slug. Mitigation: existing `DisplayName` validation
  should already forbid reserved tokens; if not, tighten as part
  of this slice. Single-day cost.

- **Sync.rs path computation.** `sync.rs:236` uses
  `envelopes_dir(aliases, &r, &paths.root)` — composes on top of
  `queue_dir`. Should follow the new shape automatically since
  `envelopes_dir` is `queue_dir(...).join("envelopes")`. Verify in
  tests; no manual surgery expected.

- **Outbox watcher / daemon.** The daemon's outbox watcher recurses
  into peer dirs looking for `outbox/*.md`. The path changes from
  `<peer>/<handle>/outbox/` to `<peer>/channels/<handle>/outbox/`.
  The recursive walker (`crates/daemon/src/outbox/watcher.rs` or
  wherever) should find both — recursive find on `*.md` under
  `outbox/` doesn't care about depth. Verify with the existing
  outbox-watcher integration test.

- **Existing draft envelopes in peer outboxes.** The migration `mv`s
  them along with the queue tree, so they end up at the new path.
  Daemon picks them up after the principal restarts the app.
  Confirm in smoke test.

### 🏴 Off-sides called

- Roster + governance for peer queues. Out — they stay implicit-2-roster.
  Future wedge.
- Resolver unification (`channel_dir` + `queue_dir` into one
  function). Out — premature. Two callers, two shapes, two functions.
- `<peer>/identity.md` per-peer identity mirror. Out — peer identity
  lives in `_self/contacts.md` (slice 4).

### 🥩 Fat cut

- Could ship the resolver change WITHOUT the migration script — fresh
  installs use the new layout; existing peer dirs stay on old layout
  forever, broken. Buys ~3 hours. Cost: principal's existing peer
  history doesn't surface. **Don't cut** — this is the same
  envelopes-never-lost discipline applied to retroactive access, not
  just storage durability.

- Could defer until per-channel template is concretely needed. Cost:
  the asymmetric `queue_dir` shape stays a wart in the substrate.
  Recommend bundling with v0.7.0 (slice 3 + slice 4) — three small
  slices, one cutover, one migration.

### 🧪 Domain knowledge

- Confirm: how many peer queue directories exist in your vault today?
  (Likely zero or near-zero — Marcelo + Christophe are the only
  peers and DM traffic is sparse.) If near-zero, migration cost
  is trivial.
- Confirm: does any test fixture write directly to
  `<peer-alias>/<handle>/envelopes/` paths? Grep + bulk-update if
  so.

## Pitch

### Problem

v0.6.0 collapsed self + org queue layouts into one shape:
`<alias>/channels/<segs>/envelopes/...`. Peer DM queues did NOT get
the same treatment because peer/contact primitive collapse was
explicitly deferred. Today peer queues sit at
`<peer-alias>/<handle-segs>/envelopes/...` — no `channels/` layer.

This blocks per-channel `template.md` / `contract.local.md`
overrides for DMs (the resolver doesn't know where to look), keeps
two storage metaphors in the codebase
(`channel_def_store::channel_dir` vs `queue_dir::queue_dir`), and
breaks the conceptual rule "every queue-root has the same shape."

### The bet

Insert one `channels/` layer in `queue_dir.rs::queue_dir`. Migration
script wraps existing peer queue dirs into a new `channels/`
parent. Per-channel template + contract resolution naturally
follow.

Ship together with slice 3 + slice 4 as v0.7.0 "layout-complete."
Migration extends the v0.6.0 hand-script. Same discipline: `mv`-only,
pre-flight tar snapshot, post-move count check.

The bet pays off when:

- One resolver shape across self/org/peer. Two callers, identical
  output structure.
- Per-channel template / contract overrides work for DMs without
  inventing a special case.
- The vault layout teaches one rule: `<root>/<alias>/channels/<segs>/`.
- Future federation (peer-to-peer relay sync) lands against the
  same path shape that org-channel sync uses.

### No-gos

- **No peer/contact primitive refactor in this slice.** Roster
  governance, multi-roster channels, DM-as-conversation aggregate —
  all separate wedges. This slice is storage layout only.
- **No data destruction.** `mv` only on queue trees; pre-flight tar
  snapshot is rollback. Envelopes inside peer queues are sovereign
  sediment by the same rule.
- **No resolver unification.** Two callers, two shapes of input,
  two functions. Don't consolidate prematurely.

## Reference

- v0.6.0 release: 2213f6d (merge), d1237a3 (release commit)
- Slice 2 pitch: `docs/pitches/2026-05-17-collapse-namespaces.md`
- Slice 3 pitch: `docs/pitches/2026-05-18-slice-3-identity-consolidation.md`
- Slice 4 pitch: `docs/pitches/2026-05-18-slice-4-principal-context-md.md`
- `crates/core/src/infrastructure/queue_dir.rs` — primary edit target
- `crates/core/src/infrastructure/channel_def_store.rs::channel_dir`
  — shape to mirror (without unifying)
- `crates/core/src/application/sync.rs:236` — `envelopes_dir` caller;
  composes on top of `queue_dir`, gets new shape automatically
- Memory: `project_contracts_attach_to_queues` — "no bilateral
  primitive; cardinality changes shape, not primitive" rationale
- AGENTS.md rule #7 (drafts in queue's local outbox) — peer queue
  outbox path mention refreshes after migration
