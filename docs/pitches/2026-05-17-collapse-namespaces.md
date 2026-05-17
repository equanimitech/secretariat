# collapse namespaces — one primitive, one layout

Pitch — 2026-05-17. Filed against the v0.4.x vault layout after a
review session surfaced that `~/.secretariat/` has *three* namespaces
doing overlapping work:

```
channels/             ← top-level "personal" channels (journals, secretariat)
orgs/<alias>/channels/ ← channels inside an org
queues/               ← flat queues (inbox/, area/, project/, equanimitech/)
peers/                ← empty (dead)
did, key, profile.json, contacts.json, relay-state.json ← naked at root
```

There is one primitive — *a queue-root with channels under it* — and
the layout should make that primitive impossible to miss. Two kinds of
queue-root: the principal (`_self`) and an org (`orgs/<alias>`).
`inbox`, `area:articles`, `project:autonomous-enterprise` are not three
namespaces — they're just channel names under `_self`. The
`area:` / `project:` / `inbox:` GTD-vocabulary prefix is dead weight in
a channel-first world.

## Boundaries

### Job to be done

As a principal opening `~/.secretariat/`, I want **one consistent
organizing principle** — every envelope-bearing directory is a channel
under a queue-root; every queue-root has the same shape — so the
layout teaches the model instead of contradicting it, and so adding a
new conversational surface means picking a name, not picking a
namespace.

*When*: today's three-namespace layout is the substrate's surface area
for every load/save call site, every CLI verb, every MCP tool, every
agent prompt that walks the tree. The split-brain leaks. The
`CaptureRoots { flat_queues, channel_tree }` parameter in
`crates/core/src/application/capture_ops.rs` is the smell — the use
case is forced to know which of two roots a capture belongs to. With
the collapse, captures land under `<root>/channels/<channel-segments>/`
unconditionally.

### Appetite

`big`. This is a foundational refactor: every load/save site touched,
a one-shot migration script, lexicon updates, MCP tool description
updates, docs/AGENTS.md updates. Six weeks for someone working it
seriously. The justification is leverage — every subsequent
feature lands against a coherent layout instead of paying the
three-namespace tax forever.

### What's in scope

- New layout (see Elements §1).
- One-shot migration tool (`sec vault migrate` — idempotent, reads
  current shape, writes new shape, dry-run by default).
- `CaptureRoots` collapses to a single `vault_root` parameter; the use
  case resolves `root × handle → directory` via one rule.
- `QueueHandle` grammar simplifies (Elements §3).
- `.org` → `org.md`, `profile.json` → `identity.md`, `contacts.json`
  → `contacts.md`, `cognition.json` → `cognition.md` (markdown +
  frontmatter for principal-editable; raw bytes for keys, machine-only
  JSON for `relay-state.json`).
- `~/.secretariat/peers/` deleted (empty, dead).
- `~/.secretariat/.tauri-{daemon,mcp}-binary-path` moved under
  `~/.secretariat/.runtime/` (operational, not principal-facing).
- Migration of all existing user vaults (Rafa's own first, then
  Marcelo's, Christophe's) coordinated as part of the v0.5.0
  release — breaking change, gated on a clean upgrade path.

### What's out of scope

- Peer/Contact primitive collapse — the "DM is just a 2-roster
  channel" refactor is its own pitch ([[project_contracts_attach_to_queues]]).
  This pitch organizes *the existing primitives*; collapsing
  `Recipient::Peer` is a separate domain refactor.
- Channel manifest (`channel.md` vs `.channelDef`) — already in flight
  on a separate branch, will land first; this pitch picks up after.
- SQLite read-cache — still v0.4+ deferred per
  [[project_filesystem_authoritative]].
- Migrating to a single binary identity wrapper that includes the key
  (HSM, encrypted bundle). Keys stay loose binary at
  `_self/identity/key` for now; rotation/migration UX is its own wedge.

## Elements

### 1. New layout

```
~/.secretariat/
├── _self/                          ← principal-as-queue-root, same shape as an org
│   ├── identity.md                 ← DID + key path + display name + signing meta (frontmatter + body)
│   ├── identity/
│   │   └── key                     ← raw ed25519 PKCS#8 bytes, 0600
│   ├── contract.local.md           ← principal's global consumption preferences
│   ├── template.md                 ← global envelope template
│   ├── contacts.md                 ← markdown w/ frontmatter rows (one block per contact)
│   ├── cognition.md                ← cognition provider config (frontmatter)
│   └── channels/
│       ├── inbox/                  ← was queues/inbox/triage
│       │   ├── channel.md
│       │   ├── contract.local.md
│       │   └── envelopes/
│       ├── articles/               ← was queues/area/articles
│       ├── autonomous-enterprise/  ← was queues/project/autonomous-enterprise
│       ├── journals/               ← was top-level channels/journals
│       │   └── therapy/            ← nested channel; nesting via directory depth
│       └── secretariat-editor/     ← was top-level channels/secretariat/editor
├── orgs/
│   ├── themia.pro/
│   │   ├── org.md                  ← was .org JSON
│   │   ├── contract.local.md
│   │   └── channels/
│   │       └── dommage-corporel/
│   │           ├── channel.md
│   │           ├── contract.local.md
│   │           └── envelopes/
│   └── equanimi.tech/
│       └── …
├── .runtime/                       ← operational, never principal-facing
│   ├── daemon.sock
│   ├── relay-state.json
│   ├── daemon-binary-path
│   └── mcp-binary-path
├── .logs/                          ← was logs/, renamed for hidden-by-default
└── preferences.toml                ← app-level (window state, terminal picker, dev flags) — STAYS TOML
```

**Why preferences.toml stays.** App-level machine config (window
positions, terminal picker, dev-mode flags). Not principal-editable
in the same sense — the Tauri settings pane writes it. TOML has the
right ergonomics for that surface (typed, no parser surprises). The
markdown-everywhere rule applies to **principal-authored, agent-read
context**: identity, contracts, templates, contacts, cognition. Not
machine config.

### 2. `identity.md` shape

```markdown
---
did: did:web:rafa.equanimi.tech
display_name: "Rafael T. Ballestiero"
key_path: identity/key            # relative to this file
key_type: ed25519
key_created_at: 2026-05-12T05:55:00Z
key_rotations: []                 # append on rotation; old entries point at archived key paths
---

# Identity

This is the principal's identity record. The DID is the canonical
identifier; the key file referenced above is THE proof. Backup this
directory, not the JSON file that used to be here.
```

DID, key location, display name, rotation log all in one place. Agent
can read it; principal can hand-edit it. Key bytes stay binary in a
sub-directory.

### 3. `QueueHandle` grammar

Drop the `channel:` / `inbox:` / `area:` / `project:` top-level
prefix. A handle is just `<segment>(:<segment>)*` — colons separate
directory depth. The root context (`_self` vs `orgs/<alias>`) is
carried by the `Recipient` or the resolution call, not the handle.

Examples:
- Old `inbox:triage` → new handle `triage` under `_self` root.
- Old `area:articles:equanimitech` → new handle
  `articles:equanimitech` under `_self` root.
- Old `channel:dommage-corporel:paris-cohort` → new handle
  `dommage-corporel:paris-cohort` under `orgs/themia.pro` root.

Compatibility shim: the parser accepts the old `channel:` /
`inbox:` / `area:` / `project:` prefix and strips it during the
migration window (v0.5.0 → v0.6.0). Strips, doesn't error — lets old
captures still resolve while the model unlearns the prefix.

Wire URI grammar (per [[project_queue_uri_grammar]]):
`did:web:themia.pro#dommage-corporel:paris-cohort`. The `channel:`
token disappears here too; `#` separates DID from handle, handle is
just colon-segmented path.

### 4. Single resolution rule

```rust
fn channel_dir(vault_root: &Path,
               recipient_root: &Root,  // Self | Org(alias)
               handle: &QueueHandle)
               -> PathBuf {
    let root_dir = match recipient_root {
        Root::Self_ => vault_root.join("_self"),
        Root::Org(alias) => vault_root.join("orgs").join(alias.as_str()),
    };
    let mut dir = root_dir.join("channels");
    for seg in handle.segments() {
        dir.push(seg);
    }
    dir
}
```

`CaptureRoots { flat_queues, channel_tree }` → deleted. Everywhere it
appears, replaced with `vault_root` + the resolver.

### 5. `sec vault migrate`

CLI verb. Reads current shape, computes target layout, prints a
dry-run by default. With `--apply`:

1. Move `did`, `key`, `profile.json` into `_self/identity.md` +
   `_self/identity/key`. Merge fields into frontmatter.
2. Move `template.md`, `contacts.json`, `contracts/*` into `_self/`
   tree. JSON→markdown conversion via existing serde models.
3. Move `queues/inbox/triage/*` → `_self/channels/inbox/envelopes/`.
   Same for `queues/area/<X>/` → `_self/channels/<X>/envelopes/`,
   `queues/project/<X>/` → `_self/channels/<X>/envelopes/`.
4. Move top-level `channels/<X>/` → `_self/channels/<X>/`.
5. Rename `<org>/.org` → `<org>/org.md` (JSON→frontmatter).
6. Delete `peers/`. Move `logs/` → `.logs/`, `daemon.sock` etc. → `.runtime/`.
7. Re-validate: walk the new tree, parse every `channel.md` and
   `contract.local.md`, confirm no orphans.

Idempotent — running twice does nothing on the second pass.
Reversible — emits a JSON log of every move; `sec vault migrate
--rollback <log>` undoes it.

### 6. MCP + CLI surface updates

Per AGENTS.md rule #6 (four-surface parallel): every changed
operation lands on application + CLI + MCP + tests. The verbs
themselves don't change much — what changes is what they receive:

- `capture` (tool) — `queue` parameter accepts the new bare-handle
  grammar; compatibility shim translates `inbox:triage` →
  `triage` for the migration window.
- `compose` (tool) — same compatibility shim on the recipient
  handle.
- `list_channels` (tool) — walks the new tree; output unchanged.
- `read_channel` (tool) — same.
- `secretariat://orgs` (resource) — walks new tree; output unchanged.
- `secretariat://contacts` (resource) — reads `contacts.md` instead
  of `contacts.json`.

### 7. Migration coordination

Three principals to migrate:
- **Rafa** — author, eats own dog food, migrates first.
- **Marcelo** — manual coordination; one walkthrough call.
- **Christophe** — DM with migration steps + offer to run remotely.

Gate the v0.5.0 release behind all three successful migrations. The
release notes are the migration guide; the `sec vault migrate
--dry-run` output is the trust-build (the principal sees every move
before approving).

## Risks

### 🐇 Rabbit holes

- **Conversion-time data loss in `profile.json` → `identity.md`.**
  Frontmatter is structurally weaker than typed JSON; round-trip via
  serde-yaml → struct → serde-yaml is lossy if unknown keys
  exist. Mitigation: migration tool preserves unknown frontmatter
  keys verbatim by reading raw YAML then re-emitting.
- **The compatibility shim becomes permanent.** Every migration
  window I've shipped has lived twice as long as planned. Hard
  deadline: v0.6.0 removes the prefix shim; CI fails if any
  handle in test fixtures still uses the prefix.
- **Idempotent migration that secretly isn't.** The `--apply`
  step has to be exactly reversible. Spike: run migration, run
  rollback, run migration again, diff the tree against the first
  migration. If it differs, the migration isn't idempotent. Block
  the slice on that test passing.
- **Nested channels collide with same-named flat queues during
  migration.** What if `queues/area/journals/` exists AND
  `channels/journals/` exists? Today they're separate; after the
  collapse they'd both want to be `_self/channels/journals/`.
  Mitigation: migration tool detects collisions in dry-run and
  refuses to apply until the principal chooses a rename.
- **`QueueHandle::parse` already validates the old grammar across
  hundreds of tests.** Test churn will be high. Mitigation: keep
  the old grammar working under the shim; new tests assert the new
  grammar; old tests stay green until the v0.6.0 cleanup slice.

### 🏴 Off-sides called

- Peer/contact primitive collapse. Out — separate pitch.
- Channel manifest refactor. Already in flight on another branch.
- A "vault.md" root-level index file describing the layout. The
  layout describes itself once collapsed; an index file is a
  symptom of incoherent layout, not a solution.
- Renaming `_self` to something more user-friendly (e.g. `me/`).
  The underscore signals "substrate-managed prefix" (consistent
  with leading-underscore namespace convention from
  [[project_namespace_symmetry]]); user-friendly aliasing is a UI
  concern, not a layout concern.
- Moving the entire vault under `~/Library/Application Support/`
  on macOS to follow platform conventions. The current `~/`
  location is intentional (principal-visible, easily `tar`-able)
  — see [[project_filesystem_authoritative]] and
  [[project_portability_already_inherent]]. Don't.

### 🥩 Fat cut

- Could ship the layout collapse WITHOUT renaming `.org` → `org.md`,
  `profile.json` → `identity.md`, `contacts.json` → `contacts.md`.
  Pure markdown conversion can be its own follow-up. Buys ~2 weeks.
  Risk: the inconsistency stays visible to the principal, eroding
  the "one principle" payoff.
- Could ship the layout collapse WITHOUT dropping the `channel:` /
  `inbox:` prefix. Just unify directories. Buys ~1 week. Risk: the
  handle vocabulary still lies, and every future surface that reads
  handles inherits the dead weight.
- Could defer the migration tool to a separate slice and ship the
  new layout + tell users "fresh vault required." Buys ~2 weeks.
  Risk: existing principals can't upgrade; v0.5.0 effectively forks.

### 🧪 Domain knowledge

- Confirm what `queues/equanimitech/secretariat/` represents — looks
  like a flat queue using GTD `equanimitech` namespace, but the
  bare `equanimitech` namespace isn't documented anywhere. Spike
  before migration: does the principal think of this as
  `area:equanimitech:secretariat` or as a project root?
- Confirm whether `~/.secretariat/peers/` is truly dead or held by
  some experimental code path. Grep + git log say dead; verify.
- Confirm Marcelo + Christophe will actually run the migration
  rather than nuke and restart. The `--dry-run` UX is load-bearing.

## Pitch

### Problem

`~/.secretariat/` has three places a thing-with-envelopes can live
(`channels/`, `orgs/*/channels/`, `queues/`), no first principle saying
which goes where, naked `did`/`key` files at root, half the config in
JSON and half in markdown, and an empty `peers/` directory left over
from a deleted feature. The three-namespace tax is everywhere — the
`CaptureRoots` parameter has to know which root to pick, the channel
walker walks two different trees, every prompt that orients the agent
has to explain the split.

This was incremental honest sediment — flat queues came first
(v0.2), channel-tree came second (v0.3), org-rooted came third
(v0.3 pivot), and they accumulated as siblings instead of one of them
absorbing the others. The substrate is ready to converge.

### The bet

Collapse to **one primitive: a queue-root with channels under it.**
Two kinds of queue-root (principal + org), same shape. Drop the
GTD-vocabulary handle prefixes. Wrap loose `did`/`key`/`profile.json`
into one `identity.md` (binary key stays in a sub-dir). Convert
principal-authored JSON to markdown with frontmatter; keep
`preferences.toml` (app-level config) and `relay-state.json`
(machine-only) as-is.

Ship behind a one-shot, idempotent, reversible `sec vault migrate`
tool. Land in v0.5.0 with a compatibility shim for old handle
grammars; remove the shim in v0.6.0.

The bet pays off when:
- A new contributor opens `~/.secretariat/` and the layout is
  self-explanatory in 60 seconds.
- Adding a new conversational surface means picking a name, not
  picking a namespace.
- The agent reads `_self/contract.local.md` and any channel's
  `contract.local.md` via one resolver, not two.
- The `CaptureRoots` parameter and its `flat_queues`/`channel_tree`
  split are gone from every call site.

### No-gos

- **No vault server.** Layout collapse doesn't touch sovereignty —
  filesystem-authoritative invariant holds.
- **No format proliferation.** Markdown + frontmatter for principal
  context; binary for keys; TOML for app config; JSON for
  daemon-managed runtime state. Four formats, each with a clear
  reason to exist.
- **No hidden-by-default principal context.** `_self/` is visible;
  identity, contracts, templates are visible. Only `.runtime/`,
  `.logs/`, and `.<file>` operational dotfiles are hidden.
- **No breaking change without a migration tool.** `sec vault
  migrate --dry-run` ships first; `--apply` ships only after the
  dry-run has been audited by all three pilot principals.

## Reference

- AGENTS.md rule #2 (no IO in domain), #6 (four-surface parallel)
- [[project_namespace_symmetry]] — sibling-namespace primitive
  identified 2026-05-12 but never followed through to layout collapse
- [[project_contracts_attach_to_queues]] — "no bilateral contract
  primitive; cardinality changes shape, not primitive" — same
  philosophy applied to queues
- [[project_queue_uri_grammar]] — `did:web:themia.pro#channel:…`
  becomes `did:web:themia.pro#…` after handle-prefix drop
- [[project_filesystem_authoritative]] — why we get to do this at
  all without a DB migration
- [[project_portability_already_inherent]] — what the collapse
  preserves: `tar`-able, fork-able, grep-able
- `crates/core/src/application/capture_ops.rs` — `CaptureRoots`,
  the smell that motivated this pitch
- `crates/core/src/domain/queue_handle.rs` — grammar lives here;
  the prefix-drop is a one-file change
- `docs/decisions/2026-05-12-substrate-layout-v03.md` — what
  v0.3 thought the layout would be; this pitch supersedes
