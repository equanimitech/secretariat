# Plan — Substrate for Themia

Derived from the stamped pitch at `docs/pitches/2026-05-21-substrate-for-themia.md` (docHash `sha256:90cb6618...`, stamped 2026-05-24).

The pitch carries the _why_; this plan carries the _how_. If they disagree, **the pitch wins** — flag the divergence, update the plan, re-stamp.

## Pre-flight findings (2026-05-25)

Pre-flight P1, P2, P3 completed before Move 1. Four amendments to the plan as a result:

1. **Keychain stays filesystem-based.** `crates/core/src/infrastructure/keys.rs` exposes path-agnostic `save_signing_key` / `load_signing_key`; agent keys live at `<self_root>/identity/agents/<name>/key` (mode 0600), mirroring the principal-key pattern. Pitch phrase _"key stored in keychain under identifier `secretariat.agent.<name>`"_ is aspirational — platform Keychain Services migration is a separate, later slice.

2. **Identity records become signed in this slice.** Today identity records are unsigned plain markdown+frontmatter. Pitch's verifier chain hop 4 ("verify principal identity-record signature") requires them to be signed. Move 1 gains: add `$signature: ed25519:<base64>` field to identity lexicon, sign on save, verify on load. Existing identity records migrate on first daemon tick (sign with active key, no preimage break since no prior signature existed).

3. **`authorized_agents` ride the wire via MembershipClaim.** Identity record stays _"private to the principal's device — never sent on wire"_ per lexicon. The receiver-side `authorized_agents` lookup uses the existing MembershipClaim envelope flow: MembershipClaim carries a `authorized_agents` snapshot (signed by the member principal), receivers cache per-principal. On `sec agent add` / `sec agent rotate` / `sec agent remove`, a fresh MembershipUpdate envelope is emitted to every org the principal is a member of, so each org's other members can update their cache. No new public artifact, no DID-document augmentation — stays within the correspondence substrate.

4. **Themia channel set:** `assemblee_generale` (exists, `requires_stamp: true`), `finance` (`requires_stamp: true` for decisions, ambient for ops), `dev` (ambient). The freelance walk-through uses a per-engagement subchannel `dev/<engagement>` (super-channel pattern, per commit `049529d`) rather than a top-level `dev-project-x`.

These amendments override the original Move 1 / P-section scope where they conflict.

## Pre-flight

### P1. Keychain multi-key spike

**Goal:** Confirm `crates/core/src/infrastructure/` keychain abstraction can carry N keys (principal + N agents) without invasive refactor.

**Steps:**

- Read current keychain implementation

- Identify what writes/reads keys (likely flat-file under `~/.secretariat/identity/key` per the identity lexicon, or platform keyring crate)

- Test: provision a second key under identifier `secretariat.agent.test`, read it back, sign + verify

- If structural change required: spec the change before moving on

**Pass criteria:** Two distinct keys can coexist in the keychain backend, distinguishable by identifier, both round-trip ed25519 sign/verify.

**Deliverable:** Notes in a scratch file or comment in the relevant infrastructure module. No code yet.

### P2. Verifier chain on paper

Before any code: trace the 4-hop verifier chain on paper. Envelope sig → resolve author DID to anchoring principal → check author DID in `authorized_agents` → verify principal identity-record sig → return `{signature, author_role, agent_role, substrate}`. Confirm shape, write the test up front.

### P3. Themia channel set

Rafa enumerates the Themia channel slugs to mint before Christophe accepts: `assemblee_generale` (exists), plus `finance`, `legal`, `dev-project-x`?, etc. Pre-implementation task; output goes into the org-invite step.

## Move 1 — `sec agent add` CLI + identity-record `authorized_agents` + identity-record signing + MembershipClaim publication

### Files to create

- `crates/core/src/application/agent_ops.rs` — application use case for add/list/remove/rotate; on mutation, calls into membership-republish path (see below)

- `crates/cli/src/commands/agent.rs` — CLI subcommand handler

- `crates/core/src/domain/agent.rs` — `Agent` value object (`{did, role, name, substrate, added_at}`)

### Files to modify

- `crates/core/src/domain/identity.rs` — add `authorized_agents: Vec<Agent>` field; add `signature: Option<Signature>` field (None for pre-signed legacy records, Some for every record written post-Move 1)

- `crates/core/src/infrastructure/identity_store.rs` — sign-on-save + verify-on-load. Canonical preimage = serialized frontmatter (sans `$signature` field) + body. Active key from `key_path` signs; verifier reads `$signature` field, recomputes canonical preimage, checks against active key. Migration on first daemon tick: detect `signature: None` records, sign with active key, save.

- `crates/core/src/infrastructure/markdown.rs` — YAML serialization of `authorized_agents`; carry `$signature` field in identity frontmatter (skip on canonical preimage build)

- `crates/core/src/infrastructure/keys.rs` — add `agent_signing_key_path(name) -> PathBuf` resolver returning `<self_root>/identity/agents/<name>/key`; extend `ensure_dirs` to create the agents/ root

- `crates/cli/src/main.rs` — register `agent` subcommand

- `crates/mcp/src/server.rs` — expose `agent_add`, `agent_list`, `agent_remove`, `agent_rotate` MCP tools (per AGENTS.md "every principal-facing primitive ships on both interfaces")

- `lexicons/tech.equanimi.secretariat.identity.json` — **already updated this session** (2026-05-24) with `authorized_agents`; add `signature` field (optional, ed25519 detached over canonical preimage)

- `lexicons/tech.equanimi.secretariat.membershipClaim.json` (or analog) — add `authorized_agents` snapshot field; receivers cache per-principal

- `crates/core/src/application/invite_ops.rs` — when emitting MembershipClaim on accept, populate `authorized_agents` snapshot from local identity record

- `crates/daemon/src/` — on receiving MembershipClaim / MembershipUpdate, cache the carried `authorized_agents` indexed by member-principal-DID; verifier consults this cache

### Tests

- Unit: `agent_ops` use case (add validates substrate against supported list, name uniqueness per principal, role against knownValues)

- Unit: identity record YAML round-trip with `authorized_agents`

- Integration: `sec agent add claude --role scribe --substrate claude-code` end-to-end (file written, key stored, identity record re-signed)

### Migration

- Existing principals: first `daemon_tick` post-upgrade adds empty `authorized_agents: []` field to identity record + re-signs. Idempotent.

- Fresh `sec init`: writes empty `authorized_agents: []`.

### Validation gate

- `sec agent list` shows the added entry

- `sec verify ~/.secretariat/identity.md` returns `signature: ok` (identity record signed by principal, still valid after the agent addition)

## Move 2 — MCP `compose` signs with agent key

### Files to modify

- `crates/mcp/src/server.rs` — `compose` handler resolves the calling agent's key (first scribe in `authorized_agents` by default; explicit `--agent <name>` to disambiguate), not principal's

- `crates/core/src/application/compose_envelope.rs` — accept a `signer: SigningContext` parameter; principal can still call but agent is the typical path

- `crates/core/src/infrastructure/markdown.rs` — envelope frontmatter `from` field reflects agent DID; add optional `signed_by_role: agent | principal` for receiver transparency

### Tests

- Unit: `compose_envelope` signs with provided key (parameterized over principal vs agent)

- Integration: MCP `compose` invocation produces envelope signed by Claude's agent DID, verifiable via principal's `authorized_agents` chain

### Validation gate

- Compose an envelope via MCP → `sec verify` returns `signed_by: <agent-did>, signed_by_role: agent`

## Move 3 — Substrate collapse

Wide refactor. Touch many files. Could land in one focused PR or sequenced small ones. Recommended: one PR per major sub-section below.

### 3a. Address-shape collapse (bare slugs, no namespaces)

**Delete:**

- Handle namespacing parser logic (`inbox:` / `peer:` / `channel:` prefixes) in `crates/core/src/domain/queue_handle.rs`

- `inbox:default` synthesizer at `crates/core/src/domain/envelope.rs:237-243`

**Rewrite:**

- `QueueHandle::parse` — bare slug only, no namespace

- `Recipient` — drop `is_local`, `is_remote`

- Module docstrings in `recipient.rs`, `envelope.rs`, `transport/relay.rs`, `relay/src/persist.rs`, `relay/src/state.rs` removing the "DM is just (peer, inbox:default)" apology comments

### 3b. DM / peer / contact removal

**Delete entire surface:**

- `crates/core/src/application/contact_ops.rs`

- `crates/core/src/infrastructure/contact_store.rs`

- `crates/core/src/domain/contact.rs`

- `crates/cli/src/commands/contact.rs` (if exists)

- MCP `secretariat://contacts` resource + related tools in `crates/mcp/src/server.rs`

- DM auto-subscribe branch at `crates/core/src/application/sync.rs:219-227`

- `crates/core/src/application/process_correspondence_claims.rs` + CLI surface

**Wipe disk (preprod):**

- `~/.secretariat/_self/peers/` — `rm -rf` (exception to `[[envelopes-never-destroyed]]` because we're wiping unmigratable preprod state, not moving in-use data)

- Handle-less legacy envelope files — `rm`

### 3c. Two channel-tree roots

**Modify:**

- `crates/core/src/infrastructure/queue_dir.rs` — `compose_dir` resolves to `orgs/<alias>/channels/<slug>/` (org-scoped) or `channels/<slug>/` (self-owned) based on org-membership lookup

- All callers of `queue_dir` updated to provide the org-membership index

### Tests

- Unit: `QueueHandle::parse("foo")` succeeds; `QueueHandle::parse("inbox:foo")` errors with helpful message

- Unit: `queue_dir.resolve` returns correct root for own-org vs personal channel

- Integration: compose to own-org channel lands draft under `orgs/<alias>/...`, not `_self/...`

- Filesystem snapshot test: tree after fresh install + compose matches the pitch's example tree

### Validation gate

- Address tests pass; channel handles are bare slugs throughout

- `find ~/.secretariat -type d -name "_self"` (peers tree) returns nothing

- No `ContactBook` symbol resolvable in the codebase

## Move 4 — Delete `_drafts/` and `sent/`

### Files to modify

- `crates/core/src/infrastructure/queue_dir.rs` — drop `drafts_dir`, `sent_dir` resolvers

- `crates/core/src/application/compose_envelope.rs` — write directly to `envelopes/YYYY/MM/DD/<rkey>.md`

- `crates/core/src/application/stamp_document.rs` — operate on envelopes/, no move-from-drafts

- `crates/cli/src/commands/compose.rs` — drop `--draft` flag

- MCP server — drop draft-staging tool variants if any

- `crates/daemon/src/envelope_watcher.rs` — watch `envelopes/` for new files lacking `delivered:` frontmatter

### Frontmatter additions

- `delivered: <relay-seq-id> | null` — written by daemon after relay confirms

- `delivered: local` — for self-owned channels under `channels/`, never federate

### Tests

- Unit: compose writes to `envelopes/`, no `_drafts/` artifact

- Unit: stamp finds envelope in `envelopes/`, updates in place

- Integration: full flow compose → stamp → envelope is at expected path with no intermediate files

### Validation gate

- `find ~/.secretariat -type d -name "_drafts"` returns nothing

- `find ~/.secretariat -type d -name "sent"` returns nothing

## Move 5 — Delete `send_envelope.rs`, federation moves to daemon

### Files to delete

- `crates/core/src/application/send_envelope.rs` — entire file

- Any `crates/core/tests/send_envelope_*.rs` — delete or migrate behaviors to daemon tests

### Files to create / modify

- `crates/daemon/src/federate.rs` (or absorb into existing `envelope_watcher.rs`) — federation logic

- `crates/daemon/src/lib.rs` — wire the federate path into tick loop

- `crates/cli/src/commands/stamp.rs` — stop triggering send (stamp updates frontmatter only)

- `crates/mcp/src/server.rs` — drop `send` tool if any; stamp tool no longer triggers send

### Error type cleanup

- `SendError::SelfAddressed` deleted

- `SendError::NotStamped` deleted

- `SendError::EnvelopeMissing`, `SendError::InvalidUtf8` → move to a new `EnvelopeReadError` or analog

- Daemon owns: `NoEndpoint`, `Relay(String)`, `Io`

### Endpoint-resolution chain (daemon-internal)

1. `envelope.recipient.owner == self_did AND channel root is orgs/<alias>` → own registered relay (`relay-state.json`)
2. `envelope.recipient.owner ∈ org membership` → membership.relay_endpoint
3. `envelope.recipient.owner == self_did AND channel root is channels/` → local-only, no federation, `delivered: local` marker
4. Else → daemon logs `NoEndpoint`, leaves envelope undelivered, retries

### Tests

- Unit: daemon `federate.rs` resolves endpoints per the 4-rule chain

- Unit: daemon writes `delivered:` frontmatter on success, retries on failure

- Integration: compose → envelope appears in `envelopes/` → daemon picks up → envelope on relay → daemon writes `delivered` frontmatter

### Validation gate

- No user-facing `send` verb exists (`sec send` errors with _"removed; federation is automatic via daemon"_)

- Compose to own-org channel federates without principal action

## Move 6 — `requires_stamp` channel-contract field

### Files to modify

- `lexicons/tech.equanimi.secretariat.channelDef.json` (or `contract.json`) — add `requires_stamp: boolean` field (optional, default false)

- `crates/core/src/domain/contract.rs` — getter

- `crates/core/src/application/review_queue.rs` (or analog) — surface unstamped envelopes on `requires_stamp: true` channels as "ambient" not "authoritative"

- Tauri review pane (frontend) — visual distinction (the three-state badge from element #3 already covers stamp/no-stamp; this adds _channel-policy expectation_)

- `sec verify --json` — include `channel_requires_stamp` in output

### Tests

- Unit: contract round-trip with `requires_stamp` field

- Unit: review queue marks correctly per channel policy

### Validation gate

- `assemblee_generale` channel's `contract.md` has `requires_stamp: true`; an unstamped envelope in it surfaces as ambient in the review pane

## Move 7 — Walk-through fix (`is_local`/`SelfAddressed` ghost removal)

Most of the work is already in Move 5 (delete `send_envelope.rs`). This move is the cleanup tail.

### Files to modify

- `crates/core/src/domain/recipient.rs` — delete `is_local`, `is_remote`, rewrite module docstring (currently codifies the pre-collapse three-case model)

- \~10 test sites referenced in pitch (`capture_ops.rs:299`, `stamp_document.rs:264`, `envelope.rs:377`, others) — rewrite or delete with intent (NOT find-replace)

- Daemon endpoint resolution (in Move 5) uses the 4-rule chain instead of `is_local`

### Tests

- **Walk-through test:** Alice (org owner) → compose to own-org channel → stamp → federate → Bob (subscriber) polls and sees the envelope. End-to-end on staging relay. This IS the walkthrough bug from 2026-05-21.

### Validation gate

- The original walkthrough bug is now fixed: own-org channel publication works.

## Three-state verify surface (cross-cutting)

Required by element #3 of the pitch. Cross-cuts Moves 1, 5, and the daemon work.

### Files to modify

- `crates/core/src/application/verify.rs` (or wherever verify lives) — return triple-state result `{signature, signed_by, signed_by_role, stamp: none | ok | invalid_hash_mismatch, stamp_by, stamped_hash, current_hash, stamped_at}`

- `crates/cli/src/commands/verify.rs` — `--json` flag returns the triple

- `crates/cli/src/commands/read.rs` — prepend warning header on state ✗ (_"⚠️ STAMP INVALID — body modified since stamping"_)

- `crates/mcp/src/server.rs` — `verify` tool returns same shape as CLI

- `crates/daemon/src/inbox_writer.rs` — on write, compute hash, compare to attestation's `docHash`, route mismatches to tamper-flagged queue + log

- Tauri frontend — verify-status badge component on review pane (✓ / ✗ / ◯)

### Tests

- Unit: verify returns correct state for each of three cases (clean stamp, modified-since-stamp, no-attestation)

- Integration: edit a stamped envelope on disk → `sec verify` reports state ✗ with hash diff

### Validation gate

- Stamp this very plan doc, edit it, run `sec verify` → see state ✗ with the hash mismatch

## Tauri onboarding — cognition-provider selection

### Files to create / modify

- `src-tauri/src/commands/onboarding.rs` — Tauri command that invokes `sec agent add <name> --role scribe --substrate <substrate>` plus the substrate-specific wiring (e.g. `sec mcp install` for `claude-code`)

- `src/lib/onboarding/CognitionProviderSelect.svelte` (or analog component) — UI list

### Today's list

- **Claude Code** (recommended)

- **Skip** (substrate works without scribe; principal composes manually)

Future providers added by extending the enum + adapter; not in this slice.

### Tests

- Manual: fresh install, Tauri opens onboarding screen, principal clicks Claude Code, agent is provisioned, MCP wired, identity record updated.

## Christophe + Freelance walk-throughs (final validation)

Both must execute end-to-end on `secretariat.equanimi.tech` before the slice claims done.

### Christophe — co-owner

1. Rafa: `sec invite create --org themia --role co_owner --channels '*'`
2. Christophe: install Secretariat, `sec init`, onboarding screen → pick Claude Code → agent provisioned
3. Christophe: `sec invite accept <token>` → membership written for every Themia channel
4. Christophe's scribe composes PV in `assemblee_generale` → signs → daemon federates to Rafa's relay
5. Rafa's daemon polls, sees Christophe's-Claude's PV → ambient (channel `requires_stamp: true`)
6. Rafa reviews in batched session → stamps → PV becomes authoritative

### Freelance — partial access

1. Rafa: `sec invite create --org themia --role external --channels dev-project-x`
2. Freelance accepts → membership written for `dev-project-x` only
3. Freelance daemon polls only `dev-project-x`
4. Freelance's scribe composes status update → signs → publishes to `dev-project-x`
5. Other channels remain invisible to Freelance (trust-by-discipline pre-roster-gate)

### Pass criteria

- All steps executed via documented CLI / UI surfaces — no manual fixups

- `sec verify --json` returns `{signature: ok, stamp: ok}` on stamped PV

- Other org members see Christophe's stamped PV as authoritative; unstamped versions as ambient

## Ordering summary

```
P1 keychain spike  →  Move 1
                      ├─→  Move 2  ──┐
                      ├─→  Move 3 ───┤
                                     ├─→  Move 4  →  Move 5  →  Move 7
                                     │                              ↓
                      Move 6 ────────┘                  Three-state verify
                                                                 ↓
                                                       Tauri onboarding
                                                                 ↓
                                                         Walk-throughs
```

Each move can land as its own PR; review gate per move. Move 3 is the widest and could be split into 3a/3b/3c.

## Open questions

- **Themia channel set:** enumerate (pre-implementation task for Rafa, per P3)

- **Identity-record canonicalization:** does adding `authorized_agents` break existing identity-record signatures? Preprod = wipe and republish. **Confirm before Move 1.**

- **Daemon tamper-flagged review queue location:** new directory (`tamper-flagged/`) or a frontmatter tag (`tamper_flagged: true`) within existing review-queue file structure? Decide in Move 5 / three-state verify.

- **Wildcard** **`channel_handles=["*"]`** **resolution:** snapshot at accept-time vs subscribe-to-future-channels. Pitch leans snapshot + relay roster-update emits fresh signed-invite when org owner mints new channels for wildcard-scope members. Confirm with relay-side spec before Move 1.

- **MCP** **`compose`** **agent disambiguation:** if principal has multiple scribes (`claude` + `opencode` future), which one signs by default? First scribe by `added_at`? Most recently used? Explicit `--agent <name>` flag required? Decide before Move 2.
