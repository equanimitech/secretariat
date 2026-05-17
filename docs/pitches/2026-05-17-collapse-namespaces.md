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
- `CaptureRoots` collapses to a single `vault_root` parameter; the use
  case resolves `root × handle → directory` via one rule.
- `QueueHandle` grammar simplifies (Elements §3). **No compatibility
  shim** — three principals, one cutover, hard break.
- `.org` → `org.md`, `profile.json` → `identity.md`, `contacts.json`
  → `contacts.md`, `cognition.json` → `cognition.md` (markdown +
  frontmatter for principal-editable; raw bytes for keys, machine-only
  JSON for `relay-state.json`).
- `~/.secretariat/peers/` deleted (empty, dead).
- `~/.secretariat/.tauri-{daemon,mcp}-binary-path` moved under
  `~/.secretariat/.runtime/` (operational, not principal-facing).
- **Rafa's vault migration via one-shot hand-script.** A `migrate.sh`
  in the repo root (or `scripts/migrate-vault-v0.5.0.sh`) — read the
  mapping, run `mv` (NEVER `rm`) on every envelope, delete the script
  after the cutover. No CLI verb, no reversibility, no dry-run UX.
  Three principals: Marcelo + Christophe have **zero envelopes today**
  → fresh install on v0.5.0 is safe for them specifically because
  there's nothing to lose. The third wrote the code.

  **Invariant: envelopes are never destroyed.** The script moves
  envelopes; format/layout changes around them. This holds for every
  future migration too — when a principal has envelopes, the migration
  is `mv`-only on the envelope bodies, never `rm`, never nuke-and-
  reinstall. The substrate's promise is sovereignty over
  correspondence; breaking that promise once corrodes it forever.
- **Pre-v0.3 surface cleanups** carried by the same slice (sediment
  the new layout has no answer for; better to clear with the rename):
  - `sec list peers` walks the now-deleted `peers/` dir — drop the
    `Peers` target. `sec list {inbox,outbox}` overlaps the
    channels surface — fold into `sec channels` or move under
    `sec debug` (diagnostic only).
  - MCP `compose` default `handle = "inbox:default"` is bilateral-
    era. Make `handle` required, or default to `inbox` under `_self`
    per the new grammar.
  - MCP `capture` plumbs `legacy_cognition_config` +
    `legacy_cadence` paths into `load_or_migrate_preferences`
    (`server.rs:803-811`). Migration completes with v0.5.0; the
    cleanup slice drops the shim.
  - `commands/paths.rs::load_did` back-compat fallback for installs
    that pre-date the `did` file. Drops in v0.6.0 cleanup window.
  - MCP historic comments at `server.rs:995/1000/1075/1080/1135`
    documenting removed tools — move to `CHANGELOG.md`, out of
    the source file.
- **Lexicons as source of truth — by practice** (Elements §7).
  Today `lexicons/*.json` mirrors the wire shape but no one's been
  required to update it alongside Rust changes. The collapse
  renames half the surface; if the lexicon doesn't follow, drift
  compounds. Resolution: a workflow rule in `AGENTS.md` + memory
  ("edit the lexicon in the same commit as the record-shape
  change"), not codegen or runtime validation. Cheap to enforce
  while there's one author; promote to CI gate when contributor
  count grows.

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
- **`_meta` sibling queue** — explicitly dropped, not deferred. The
  earlier "every channel auto-spawns a `<channel>:_meta`" pattern
  ([[project_meta_channel_pattern]] — superseded 2026-05-17) is gone.
  Structural artifacts (`channel.md` / `contract.md` /
  `contract.local.md` / `template.md` / `CLAUDE.md` / `.claude/skills/`)
  live as **files** in the channel-dir. Mutations to them flow as
  `$type`-tagged envelopes (`tech.equanimi.secretariat.rosterUpdate`,
  `…channelDef`, `…skillDrop`) in the channel's main `envelopes/`
  stream. No sibling queue, no sub-queue, no resolved-cache directory.
  See [[project_namespace_collapse_drops_meta]].

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

**No compatibility shim.** Three principals; Marcelo + Christophe
have nothing yet; my own vault gets the hand-script. The parser
hard-rejects the old `channel:` / `inbox:` / `area:` / `project:`
prefix from v0.5.0 onward. The simpler grammar pays off
immediately; the shim would have lived too long anyway.

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

### 5. Hand-script `scripts/migrate-vault-v0.5.0.sh`

Bash. Runs once against my own vault. Steps:

1. `mv` `did`, `key`, `profile.json` data into `_self/identity.md` +
   `_self/identity/key`. JSON→frontmatter via a small inline Python
   one-liner (or `jq` + `printf`).
2. `mv` `template.md`, `contacts.json`, `contracts/*` into `_self/`
   tree. JSON→markdown for `contacts.json` → `contacts.md`.
3. `mv` `queues/inbox/triage/*` → `_self/channels/inbox/envelopes/`.
   Same for `queues/area/<X>/` → `_self/channels/<X>/envelopes/`,
   `queues/project/<X>/` → `_self/channels/<X>/envelopes/`.
   **Every envelope is `mv`'d, never `rm`'d. Never `cp` + delete-
   original either — `mv` keeps the file the same inode.**
4. `mv` top-level `channels/<X>/` → `_self/channels/<X>/`.
5. `mv` `<org>/.org` → `<org>/org.md` (JSON→frontmatter).
6. `rmdir peers/` (verify empty first). `mv logs/` → `.logs/`,
   `daemon.sock` etc. → `.runtime/`.
7. Walk the new tree, confirm envelope count before == count after.
   If mismatch, abort + restore from snapshot.

Pre-flight: `tar -czf ~/Documents/secretariat-snapshots/<date>-pre-collapse.tgz ~/.secretariat/`.
That's the rollback. Delete the snapshot a week after cutover when
the new vault has accumulated enough traffic to be trusted.

After cutover: `rm scripts/migrate-vault-v0.5.0.sh`. One-shot, no
maintenance.

### 6. MCP + CLI surface updates

Per AGENTS.md rule #6 (four-surface parallel): every changed
operation lands on application + CLI + MCP + tests. The verbs
themselves don't change much — what changes is what they receive:

- `capture` (tool) — `queue` parameter accepts the new bare-handle
  grammar; no shim, old prefix is a hard parse error. Drop
  `legacy_cognition_config` + `legacy_cadence` paths from the
  preferences-migration call; the hand-script already converted
  everything.
- `compose` (tool) — same bare-handle grammar on the recipient.
  Make `handle` REQUIRED (no `inbox:default` fallback); the
  channel-first world has no sensible default recipient queue.
- `list_channels` (tool) — walks the new tree; output unchanged.
- `read_channel` (tool) — same.
- `secretariat://orgs` (resource) — walks new tree; output unchanged.
- `secretariat://contacts` (resource) — reads `contacts.md` instead
  of `contacts.json`.
- `sec list` (CLI) — drop the `Peers` target (dir is gone). Fold
  `inbox`/`outbox` under `sec channels` or `sec debug`; the bare
  `sec list` verb retires.
- Historic `// Note: …was a tool in 0.2.x…` blocks in `server.rs`
  move to `CHANGELOG.md`. Source file carries current surface only.

### 7. Lexicons as source of truth (practice, not codegen)

Today `lexicons/*.json` is decorative — it mirrors the on-wire shape
but no code path validates against it. The Rust types in
`crates/core/src/domain/` are the de facto authority; lexicon drift
is silent. The collapse renames half the surface (handle grammar,
record paths, frontmatter fields); shipping that without locking
lexicons as SoT compounds drift.

Resolved as a **workflow rule**, not a technical mechanism. No
codegen, no runtime validator — those are appetite traps the
substrate doesn't need yet. Instead, the rule lands in `AGENTS.md`
and Claude's memory:

> When changing any record shape — adding a field, renaming a
> field, changing a grammar — the lexicon under `lexicons/` is
> edited in the **same commit** as the Rust change. Lexicon
> first if you can; Rust-then-lexicon-in-same-commit if you can't.
> Reviewing a record-shape PR that lacks a lexicon diff is a
> stop-the-line event.

Why a practice rule is enough right now:

- One author, two pilot principals. Drift is detectable by eye on
  every PR; we don't need a CI gate to catch it.
- The lexicons aren't published yet (`AGENTS.md` "Out of scope")
  so external consumers can't be broken by drift. The cost of a
  miss is internal confusion, not a wire incompatibility.
- Codegen + runtime validation are real options *later* — when
  publishing the lexicon or when a second implementation (mobile,
  web) appears. Today they'd be cost without payoff.

Scope inside this pitch:

- Every record-shape rename this slice does (`org.md`,
  `identity.md`, `contacts.md`, new `QueueHandle` grammar, the
  `_self` + `orgs/<alias>` envelope `to` shape) ships its lexicon
  edit in the same PR.
- Add the rule as a numbered Hard Rule in `AGENTS.md` so Claude
  reads it on every session start.
- Add a memory entry so the rule survives outside the repo too.

Out of scope: build.rs codegen, runtime JSON-schema validation,
CI conformance test, lexicon publication. All revisitable when
the constraints change.

### 8. Migration coordination

Three principals; only one carries data.

- **Rafa** — author, runs `scripts/migrate-vault-v0.5.0.sh` against
  own vault after pre-flight snapshot. Verifies envelope count
  matches before/after. Deletes script after one week of clean
  operation on new layout.
- **Marcelo** — zero envelopes today. Fresh install on v0.5.0:
  `brew upgrade secretariat`, then `sec init` re-derives identity
  from existing key (key file is untouched), nothing to migrate.
- **Christophe** — same. Possibly hasn't installed yet, in which
  case v0.5.0 is just "the install."

No release gate. The hand-script proves itself on my own vault;
Marcelo and Christophe never run it because there's nothing to
migrate.

## Risks

### 🐇 Rabbit holes

- **Conversion-time data loss in `profile.json` → `identity.md`.**
  Frontmatter is structurally weaker than typed JSON; round-trip via
  serde-yaml → struct → serde-yaml is lossy if unknown keys
  exist. Mitigation: hand-script preserves unknown keys verbatim by
  reading raw JSON, emitting raw YAML, and never round-tripping
  through a typed struct.
- **Envelope loss during `mv`.** The substrate's prime directive
  (`AGENTS.md` invariants; sovereignty rules) is that the principal
  never loses correspondence. Mitigation: pre-flight `tar` snapshot
  to `~/Documents/secretariat-snapshots/`; post-`mv` count match
  check; abort + restore if mismatch. Hand-script never `rm`s an
  envelope file — only `mv`. Same inode preserved.
- **Nested channels collide with same-named flat queues during
  migration.** What if `queues/area/journals/` exists AND
  `channels/journals/` exists? Today they're separate; after the
  collapse they'd both want to be `_self/channels/journals/`.
  Mitigation: hand-script checks for collisions before any move
  and bails to the principal (me) for manual rename.
- **`QueueHandle::parse` test churn.** The grammar test suite is
  ~30 cases; all must flip in one PR since there's no shim. Plan
  for it; it's not a risk, it's a known cost.
- **Lexicon practice silently rots.** A rule that only Claude reads
  decays the moment a contributor lands a record-shape change
  without reading `AGENTS.md`. Mitigation: until there's a second
  human contributor, drift is recoverable by-eye. Re-evaluate when
  someone other than Rafa lands a record-shape PR — that's the
  moment to escalate practice → CI gate.
- **CHANGELOG migration loses context.** The historic `// Note:`
  comments at `server.rs:995/1000/1075/1080/1135` document *why* a
  tool was removed, not just *that* it was. Mitigation: move
  verbatim, link from `CHANGELOG.md` back to the commit + the
  superseding tool's section in the file.

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
  Pure markdown conversion can be its own follow-up. Buys ~3 days.
  Risk: the inconsistency stays visible to the principal, eroding
  the "one principle" payoff.
- Could defer the `_self/identity.md` consolidation. Loose `did` +
  `key` + `profile.json` at root stays. Buys ~3 days. Risk: same
  inconsistency erosion; the `_self/` surface lands incomplete.

(Pre-revision "defer migration tool" fat cut deleted — there is no
migration tool, and the script-only path *is* the chosen approach.
Pre-revision "no shim" fat cut deleted — already accepted into the
main scope.)

### 🧪 Domain knowledge

- Confirm what `queues/equanimitech/secretariat/` represents — looks
  like a flat queue using GTD `equanimitech` namespace, but the
  bare `equanimitech` namespace isn't documented anywhere. Spike
  before the hand-script: does the principal (me) think of this as
  `area:equanimitech:secretariat` or as a project root? Decide
  manually before writing the move rule.
- Confirm whether `~/.secretariat/peers/` is truly dead or held by
  some experimental code path. Grep + git log say dead; verify.

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

Ship in v0.5.0 as a hard cutover. No CLI migration verb, no
compatibility shim. My own vault gets a one-shot bash script
(`scripts/migrate-vault-v0.5.0.sh`) that `mv`s every envelope into
the new layout under a pre-flight `tar` snapshot. Marcelo and
Christophe have zero envelopes today — they just install v0.5.0
fresh. Envelopes are never deleted, only moved; that invariant
holds for this migration and every future one.

The bet pays off when:
- A new contributor opens `~/.secretariat/` and the layout is
  self-explanatory in 60 seconds.
- Adding a new conversational surface means picking a name, not
  picking a namespace.
- The agent reads `_self/contract.local.md` and any channel's
  `contract.local.md` via one resolver, not two.
- The `CaptureRoots` parameter and its `flat_queues`/`channel_tree`
  split are gone from every call site.
- `lexicons/*.json` edits land in the same PR as every record-shape
  Rust change. Drift gets caught by the reviewer, not after.
- `sec list peers` / MCP `compose handle: inbox:default` /
  preferences migration shim / `did.json` fallback in
  `load_did` — all gone. The pre-v0.3 sediment is cleared in the
  same slice that earns the right to do so.

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
- **Envelopes are never destroyed.** The hand-script `mv`s; never
  `rm`s, never `cp` + delete-original. Pre-flight `tar` snapshot is
  the rollback. Count match before/after is the gate. This holds
  for this migration and every future migration — the substrate's
  promise of sovereignty over correspondence is non-negotiable.

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
- `lexicons/` — current decorative source; becomes runtime SoT per
  Elements §7
- `crates/mcp/src/server.rs:803-811` — `legacy_cognition_config` +
  `legacy_cadence` preferences-migration shim, retiring this slice
- `crates/mcp/src/server.rs:709` — `inbox:default` compose fallback,
  retiring this slice
- `crates/cli/src/commands/paths.rs::load_did` — `did.json`
  back-compat fallback, retiring with the v0.6.0 cleanup
- `crates/cli/src/commands/list.rs` — `Peers`/`Inbox`/`Outbox`
  targets, retiring or moving under `sec debug`
- AGENTS.md "Out of scope" list — counter-stamp + lexicon
  publication still deferred; this pitch lifts only the *internal*
  lexicon SoT, not on-wire publication
