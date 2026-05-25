---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:90cb66182555f48082a0424ff64e823d5e4f52bdc3f47a4254dd1c1b7a62a6f4
  docFilename: 2026-05-21-substrate-for-themia.md
  stampedAt: 2026-05-24T10:19:47.858374Z
  signature: ed25519:1rI8SBwPZj8bxhHHwmpyqEjotmPZ1niVAJOF3h5XLe4NwvyJa7MAoC/qbJjNs7qPxkgyhMAzHRuhDMdkd7UgAQ==
---

# Substrate for Themia

Pitch — 2026-05-21. Source: shaping conversation 2026-05-21. Supersedes the partial "collapse DM primitive into channels" pitch from earlier today; the conversation surfaced that the real slice is broader — collapse + simplify + ship the README's "AI agents with their own keys" promise. Same date, broader scope.

**Builds on:**

* v0.10.0 (drop-outbox, 2026-05-21) — staging cleanup just landed

* v0.8 channel-relay-sequencer (receiver-side sync substrate)

* Existing org-invite primitive — `OrgInviteContext` already carries `org_did`, `role`, `channel_handles` (verified in `crates/core/src/application/invite_ops.rs:95-101`)

* README's stated direction: *"AI agents draft into channels with their own keys"*

* AGENTS.md rule #4: signature mandatory, stamp selective

**Driving framing:** Secretariat is git-for-company-comms. Every envelope is a signed commit. Channels are protected branches. Stamps are maintainer sign-offs. Claude is a first-class committer with its own DID. Principals become reviewers-and-stampers, not authors. This pitch lands that model in code.

## Boundaries

### Job to be done

As Claude (scribe) participating in Themia's substrate and as the principal (Rafa, Christophe) reviewing accumulated traffic, I want:

1. Claude composes-and-signs autonomously, no per-envelope Touch ID
2. Principal stamps only the subset they elevate to authoritative
3. One queue primitive (channel), one envelope state, one address shape
4. Org-scoped access with role + channel-scope (Christophe co-owner sees all; Freelance external sees one channel)
5. Sign ≠ stamp at the cryptographic layer (separate keys, distinguishable signatures, verifiable distinction)

**The when:** Themia onboarding for Christophe (co-owner) and a partial-access freelance is blocked today by:

* `SelfAddressed` short-circuit at `crates/core/src/application/send_envelope.rs:82` — Alice (org owner) cannot publish on own-org channel. Walkthrough bug, surfaced 2026-05-21 against `secretariat.equanimi.tech`.

* No Claude-as-signer — principal forced to sign-AND-stamp every envelope, doesn't survive AI-volume traffic.

* Three addressing models stacked (`is_local` / `inbox:default` / bare-handle) — substrate complexity for no semantic value.

* Drafts on disk before sign — staging artifact with no purpose once Claude signs at compose.

**The baseline:** current substrate (post-v0.10) has clean staging (drafts → envelopes → sent), but the *authorship* axis is still pre-scribe: principal is the only signer, agents have no DID, drafts are first-class disk objects. The drop-outbox slice cleaned the staging axis; this slice cleans the authorship axis and finishes the substrate to match the README.

### Appetite

`medium`. The conceptual shape is finally one substrate, one address, two-tier trust — but it touches every layer:

* domain (`Recipient`, `Identity`, new `Scribe` / `Agent` shape)

* application (`compose_envelope`, `send_envelope` *deleted*, `stamp_document`, `invite_ops`, `sync`, new `agent_ops`)

* infrastructure (`queue_dir`, `transport/relay`, `markdown`, keychain abstraction)

* CLI (`init`, `compose`, `stamp`, `invite`, new `agent` subcommand family with `--role` flag)

* MCP (`compose`, `stamp`, tool wiring)

* daemon (outbox-drain, inbox-write, federation logic absorbed from `send_envelope.rs`, identity-migration on upgrade)

* relay (`routes/invite`, `state`)

* Tauri shell ("Add Claude as your scribe?" onboarding screen on first launch)

Days, not weeks. Override with `--appetite=big` if the scribe-key custody plumbing in the keychain abstraction proves nastier than a one-structural-change pass.

## Elements

Five primary elements.

### 1. Agent primitive — explicit add ceremony (today's only role: scribe)

* **Two-layer vocabulary.** At the protocol/cryptographic layer, the underlying DID-keyed identity is an *agent*. At the substrate/UX layer (CLI, prose, onboarding), an agent with `role: "scribe"` is what the principal experiences. Today's only role is scribe. Future roles (`auditor`, `scheduler`, `reader`) reuse the same field shape without migration.

* **Identity record carries** **`authorized_agents`** in the existing `tech.equanimi.secretariat.identity.json` lexicon. Each entry shaped `{did, role, name, substrate, added_at}` — where `name` is the principal-chosen nickname (free-form) and `substrate` is the cognition provider identifier (`claude-code`, `opencode`, `ollama-<model>`, `anthropic-api`, etc.). Example:

  ```json
  "authorized_agents": [
    {"did": "did:key:z6Mk...", "role": "scribe", "name": "claude", "substrate": "claude-code", "added_at": "2026-05-21T..."}
  ]
  ```

* **`sec init`** **does NOT provision a scribe.** It generates principal DID + identity record with empty `authorized_agents: []`. The substrate works without any scribe — manual compose via CLI remains possible. Granting signing authority to a scribe is an explicit act of delegation, not an init default.

* **Explicit add ceremony —** **`sec agent add <name> --role scribe --substrate <substrate>`.** Provisions a new agent DID (`did:key`), key stored in keychain under identifier `secretariat.agent.<name>`, appends entry to `authorized_agents`, re-signs identity record, publishes (or refreshes local cache for `did:key` principals). Today the only wired `<role>` is `scribe` and the only wired `<substrate>` is `claude-code`; `<name>` is principal-chosen (defaults to substrate identifier if omitted).

* **CLI surface.** `sec agent add <name> --role <role> --substrate <substrate>`, `sec agent list [--role <role>]`, `sec agent remove <name>`, `sec agent rotate <name>` (mints fresh key for existing agent entry — preserves the `name` + `role` + `substrate` slots, replaces the DID, when key compromise suspected). The `--role` flag future-proofs for `auditor`, `scheduler`, `reader` without adding new top-level commands.

* **Cognition-provider selection (onboarding surface).** Aligns with architectural invariant #5 (cognition is pluggable). Tauri first-launch screen presents *"Choose your cognition provider"* — a list of supported substrates. UX vocabulary stays "scribe" (*"Add Claude as your scribe"*); selecting a substrate invokes `sec agent add <default-name> --role scribe --substrate <substrate>` under the hood, then wires the provider-specific channel (e.g. `sec mcp install` for Claude Code). The selection IS the explicit agent-add — same authority delegation, framed as a provider choice.

  * **Today's list** (show only what works): Claude Code · Skip. No promise-debt for un-wired providers.

  * **Future entries** as substrates land: OpenCode, Anthropic API (BYOK), Ollama / local LLM, Bedrock. Each adds to the list without re-engineering the agent primitive.

  * **Multi-provider setups:** principal returns to settings to `sec agent add <name> --role scribe --substrate <other>`. Each cognition provider is its own agent entry with its own DID. Coexist, no exclusivity.

* **Verifier chain (4 hops, trace on paper before code):** envelope signature → resolve author DID to anchoring principal via identity records → check author DID present in principal's `authorized_agents` → verify principal identity-record signature → return `{signature: ok, author_role: agent | principal, agent_role: scribe | null, substrate: claude-code | ... | null}`.

* **Daemon-bootstrap upgrade path.** Existing principals upgrading from the current release: first `daemon_tick` migrates identity record to new shape (empty `authorized_agents: []`); Tauri tray surfaces "Add a scribe?" notification. No silent provisioning. Principal explicitly opts in.

### 2. Substrate collapse — one address, one envelope state, two roots

* **Address shape:** bare `(owner_did, channel_slug)`. No namespace prefix. `assemblee_generale` not `channel:assemblee_generale`. Drops the `inbox:` / `peer:` / `channel:` vocabulary entirely.

* **One envelope state:** `<channel-dir>/envelopes/YYYY/MM/DD/<rkey>.md`. No `_drafts/`. No `sent/`. Delivery state = `delivered: <relay-seq-id>` frontmatter field, written by daemon after relay confirms.

* **Two channel-tree roots:**

  * `~/.secretariat/orgs/<alias>/channels/<slug>/` — org-scoped, federates to org owner's relay

  * `~/.secretariat/channels/<slug>/` — self-owned, local-only (journal, capture)

* **Compose-path resolution:** `compose_envelope` picks root based on channel-handle lookup in org-membership index. Own-org publication → `orgs/<alias>/...`. Personal channel → `channels/...`. Fixes the "draft under `_self/` for own-org publication" issue noted in the walkthrough.

Resulting tree for a Themia member:

```
~/.secretariat/
├── identity.json              # principal DID + authorized_agents (scribes)
├── relay-state.json
├── orgs/
│   └── themia/
│       ├── contract.md        # org governance (signed by Rafa)
│       ├── contract.local.md  # subscriber prefs (private)
│       └── channels/
│           ├── assemblee_generale/
│           │   ├── channel.md
│           │   ├── contract.md            # requires_stamp: true
│           │   ├── membership.local.md
│           │   └── envelopes/YYYY/MM/DD/<rkey>.md
│           └── dev-project-x/
│               └── ...
└── channels/
    └── journal/                # local-only, never federates
        └── envelopes/YYYY/MM/DD/<rkey>.md
```

### 3. Sign at compose, stamp on review

* **MCP** **`compose`** **tool:** generates envelope body → signs with agent DID's signing key → writes to local `envelopes/YYYY/MM/DD/<rkey>.md` already-signed → daemon picks up + federates. **No Touch ID at compose.** The chat conversation IS the draft surface; body renders in chat before the tool call so principal sees what's being signed.

* **MCP** **`stamp`** **tool:** loads an already-signed envelope → renders full body verbatim → awaits principal "stamp it" → Touch ID gate fires → principal signature appended as `tech.equanimi.secretariat.stamp` record → frontmatter updated. Stamp ceremony unchanged from today; only the *vocabulary* shifts (stamp is curation, not authorship).

* **Sign ≠ stamp cryptographically:** signatures by different keys are byte-distinguishable on the wire. Receivers verify both layers independently. `sec verify --json` returns `{signature: ok | invalid, signed_by: did, signed_by_role: agent | principal, stamp: none | ok | invalid_hash_mismatch, stamp_by: did | null, stamped_hash, current_hash, stamped_at}`.

* **Three-state verify surface.** Any document carrying a `$attestation` block renders in one of three states across all consumer surfaces:

  * **✓ Stamped & verified** — body hash matches attested `docHash`; signature valid
  * **✗ Stamped but modified** — signature cryptographically valid (key signed legitimately at the time) but body hash differs from attested `docHash`. Tamper-evidence in practice — the failure mode the substrate exists to surface
  * **◯ Unstamped** — no attestation block; document is unattested working state

  Required surfaces (all must render the three states consistently):

  * Tauri review pane — badge above body, click reveals hash diff + stamped-at + signer
  * `sec verify --json` — machine-readable triple-state result
  * `sec read` — prepends warning header on state ✗ (*"⚠️  STAMP INVALID — body modified since stamping"*)
  * MCP `verify` tool — same shape as `sec verify --json`
  * Daemon inbox-write — on receiving an envelope, compute hash, compare to attestation's `docHash`; state-✗ envelopes route to tamper-flagged review queue + log

  Without UI surfacing, the failure mode is silent: file *looks* stamped (block present, signature valid), but trust is misplaced. Sibling concern to `[[requires_stamp]]` channel policy.

* **Cadence:** stamping happens in batched review sessions per `contract.local.md`, not per-envelope at compose. Matches `[[review-session-model]]` and anti-compulsion design.

### 4. Federation moves to daemon — `send_envelope.rs` deleted

* `crates/core/src/application/send_envelope.rs` deleted as a use case. The compose → stamp → send pipeline was orchestrated by this file; with sign-at-compose and one-envelope-state, the pipeline collapses to: write signed envelope to `envelopes/`. Federation is plumbing, not a user-facing operation.

* `Recipient::is_local` + `Recipient::is_remote` deleted.

* `SendError::SelfAddressed` + `SendError::NotStamped` deleted (stamp no longer gates send). `SendError::EnvelopeMissing` + `SendError::InvalidUtf8` move to envelope-parse error types. Remaining variants live with daemon: `NoEndpoint`, `Relay(String)`, `Io`.

* **Federation logic moves to** **`crates/daemon/src/`** — collapses into the existing `envelope_watcher` (per drop-outbox slice) or sibling `federate.rs`. Daemon picks up new files in `envelopes/`, resolves endpoint, pushes to relay, writes `delivered: <relay-seq-id>` frontmatter on success. Retry on next tick if no endpoint resolves.

* **Endpoint resolution (daemon-internal):**

  1. `envelope.recipient.owner == self_did AND channel root is orgs/<alias>` → own registered relay (`relay-state.json`)
  2. `envelope.recipient.owner ∈ org membership` → membership.relay\_endpoint
  3. `envelope.recipient.owner == self_did AND channel root is channels/` → local-only, no federation, `delivered: local` marker
  4. Else → daemon logs `NoEndpoint`, leaves envelope undelivered, retries

* **Stamp no longer gates federation.** Agent-signed envelopes federate immediately, even unstamped — that's the "ambient context" model. Stamps elevate the subset to authoritative; they don't gate publication. Channels needing federation-gating-by-stamp (rare) could add `local_only_until_stamped: true` to channel-contract — not in this slice.

* **No user-facing** **`send`** **verb.** No `sec send` command. No MCP `send` tool. Federation is invisible plumbing.

### 5. Channel policy + roster scope

* **`<channel-dir>/contract.md`** (shared, signed by org owner) gains `requires_stamp: true | false` field. Channels carrying authoritative records (e.g. `assemblee_generale`) set it; ambient-traffic channels don't.

* **Receiver-side discipline:** UI marks unstamped envelopes as "ambient" on `requires_stamp: true` channels; agents acting on received envelopes MUST NOT treat unstamped as authoritative on such channels.

* **Relay-side enforcement:** deferred to a later slice per `[[role-tamper-proof]]`.

* **Org-invite with role + channel-scope:** existing `OrgInviteContext` already shaped right. Invite carries `role: co_owner | external` and `channel_handles: [<slug>...]` (or `["*"]` wildcard for co-owners). Acceptance writes per-channel `membership.local.md`. Daemon polls only those channels.

## Walk-throughs

### Christophe — co-owner

1. Rafa: `sec invite create --org themia --role co_owner --channels '*'` → signed invite token.
2. Token delivered out-of-band. Christophe installs Secretariat, runs `sec init` → principal DID + empty identity record. Tauri first-launch screen offers *"Choose your cognition provider"* → Christophe picks Claude Code → `sec agent add claude --role scribe --substrate claude-code` provisions agent DID, updates identity record. MCP wired silently.
3. Christophe: `sec invite accept <token>`. Acceptance:

   * Writes per-channel `membership.local.md` for every Themia channel (wildcard enumerated at accept-time)

   * Caches Rafa's relay endpoint

   * Emits `MembershipClaim` envelope → Rafa's daemon appends Christophe to org roster + channel rosters, counter-signs
4. Christophe's daemon polls every Themia channel on Rafa's relay → mirrors envelopes locally.
5. Christophe says to his scribe (Claude): "draft a PV for last week's AG meeting." Christophe's-scribe composes → signs with the scribe's DID → writes to `orgs/themia/channels/assemblee_generale/envelopes/2026/05/21/<rkey>.md` → daemon federates to Rafa's relay.
6. Rafa's daemon polls, sees Christophe's-Claude's PV envelope, marks ambient (channel `requires_stamp: true`).
7. Rafa reviews in batched session → stamps → PV becomes authoritative.

### Freelance — partial access

1. Rafa: `sec invite create --org themia --role external --channels dev-project-x` → scoped invite.
2. Freelance accepts. Acceptance writes `membership.local.md` for `dev-project-x` only. No global org-channel access.
3. Freelance daemon polls only `dev-project-x` from Rafa's relay. Other channels: no membership → no poll → never sees handle exists.
4. Freelance's scribe (Claude) composes a status update → signs with its scribe DID → publishes to `dev-project-x`. Rafa sees it on poll.
5. Other org members do NOT see Freelance's `assemblee_generale` envelopes — Freelance can't publish there, has no write membership.
6. A later relay-side roster gate hardens this server-side. Until then: trust-by-discipline (Freelance's daemon literally has no path to other channels).

## Risks

### 🐇 Rabbit holes

* **Scribe-key custody.** Confirm keychain abstraction in `crates/core/src/infrastructure/` carries multiple keys (principal + N scribes) without invasive refactor. If current path is flat-file based, multi-key support is one structural change — needs a spike before committing.

* **Onboarding screen scope creep.** "Add Claude as your scribe?" is one screen, one message, one button. Easy to grow into a tutorial, first-channel wizard, identity-export walkthrough. Resist. If first-channel wiring needs explanation, defer to channel-relay-sequencer follow-up.

* **Scribe channel-scope.** A future-proof `authorized_agents` entry could carry `channel_scope?: ["dev-*"]` to limit a scribe's signing authority to specific channels (useful for draft-bot vs auto-reply-bot separation). Out of scope for this slice — today's scribe role is full-scope. Field can be added additively later.

* **Three scopes of** **`authorized_agents`.** Same record shape (`{did, role, name, substrate, added_at}`) generalizes to three scopes as channel/org-bound agents land in later slices:

  | Location                                   | Scope            | Typical role                    |
  | ------------------------------------------ | ---------------- | ------------------------------- |
  | `~/.secretariat/identity.json` (principal) | principal-scoped | scribe (today's case)           |
  | `<org-dir>/contract.md`                    | org-wide         | auditor, scheduler              |
  | `<channel-dir>/contract.md`                | channel-scoped   | indexer, build-bot, summary-bot |

  Three corresponding commands (only the first ships in this slice): `sec agent add ...`, `sec org <alias> agent add ...`, `sec channel <slug> agent add ...`. Different ceremonies, same protocol-layer record shape.

* **Invites vs. agent-add are distinct ceremonies.** Both produce `authorized_agents`-shaped entries in the relevant roster, but the *ceremonies* differ: invites are bilateral DID exchange between two principals (both daemons accept); agent-add is unilateral authority delegation by the owner (the agent has no daemon to accept). Don't merge the commands — merge the protocol-layer fact (an entry exists, signed by the right authority).

* **Identity-record evolution.** Adding `authorized_agents` is additive but breaks existing identity-record signature preimage if the canonicalization covers all fields. Preprod = wipe and republish identity records (cheap). Mark in pitch: this is the call.

* **`channel_handles=["*"]`** **wildcard semantics.** Existing invite code uses literal list. Wildcard support means invite-accept must enumerate the org's current channel set at accept-time (snapshot) OR resolve continuously (subscribe-to-all-future-channels). Decide. My read: snapshot at accept-time + relay roster-update emits a fresh signed-invite when org owner mints new channels for wildcard-scope members. Cleaner.

* **Daemon identity-migration on upgrade.** Existing principals upgrading: first daemon tick migrates identity record shape (adds empty `authorized_agents: []`), does NOT auto-provision scribe. Tauri tray surfaces "Add a scribe?" notification. Idempotent: re-running migration must not duplicate or republish.

* **`Recipient::is_local`** **semantic-empty test rewrites.** \~10 sites (`capture_ops.rs:299`, `stamp_document.rs:264`, `envelope.rs:377`, others). Each asserts something about a behavior that no longer makes sense. Rewriting requires deciding what each test should verify, not find-replace. Risk: shallow rewrite passes tests but leaves them empty.

* **Channel-policy** **`requires_stamp`** **enforcement surface.** Receiver-side flag must surface in four places: `sec review` CLI ordering, MCP review tool output, Tauri tray review pane, `sec verify --json` output. Easy to miss one.

### 🏴 Off-sides called

* **AT-URI adoption + lexicon codegen.** Out of scope. Excellent fit; separate pitch sequenced after this one. The AT-URI adoption uses this substrate's address shape as the input.

* **Counter-stamp / multi-party stamping.** Out of scope. Later slice. PV authoritativeness will demand it; the channel-policy `requires_stamp: true` is the foothold but doesn't ship multi-stamp now.

* **Relay-side roster enforcement.** Out of scope per `[[role-tamper-proof]]`. Receiver-side discipline is what this slice ships.

* **Multi-device same-principal sync.** Out of scope. Later slice (key-migration UX).

* **DM / peer / bilateral correspondence primitives.** Removed entirely, not deferred. If peer-DM UX returns later, it joins as a 2-roster channel in a separate pitch.

* **CLI** **`sec compose`** **interactive draft staging.** Removed. CLI compose signs immediately. No `--draft` flag.

* **Hierarchical key derivation** (agent key derived from principal key, BIP-32 style). Considered, rejected. Flat agent DID listed in `authorized_agents` is simpler and equally cryptographically sound.

* **Edit-at-most flow** (principal materially edits Claude-signed envelope). Defer edit-tooling to a follow-up — this slice ships compose-only flow; edit-then-resign is manual (delete envelope, re-compose).

* **Stamp comprehension gate.** Sibling pitch file noted in earlier session state; not blocking. Independent compose/stamp path here.

### 🥩 Fat to cut

* `Recipient::is_local`, `Recipient::is_remote`, `SendError::SelfAddressed`, `SendError::NotStamped`

* `crates/core/src/application/send_envelope.rs` — entire file (federation moves to daemon)

* `_drafts/` directory + `drafts_dir` resolver branches across `compose_envelope.rs`, `queue_dir.rs`, MCP server, CLI compose

* `sent/` directory + sent-move logic (was in `send_envelope.rs`, now nowhere)

* `_self/peers/` directory tree (preprod: `rm -rf`, no migration per \[\[envelopes-never-destroyed]] note — exception holds because we're wiping not moving)

* `ContactBook`, `contact_store`, `contact_ops`, `domain::contact` — entire surface

* `inbox:default` synthesizer at `envelope.rs:237-243`

* `process_correspondence_claims` — DM bootstrap remnant

* DM auto-subscribe branch at `sync.rs:219-227`

* Handle namespace prefix logic (`inbox:` / `peer:` / `channel:`)

* Handle-less legacy envelope files on disk — wipe (preprod)

* \~10 comment apologies for "DM is just (peer, inbox:default)"

* `is_local`/`is_remote` documentation in `recipient.rs:1-32` module docstring

* CLI `sec compose --draft` flag, MCP draft tool variants

### 🧪 Domain knowledge

* **Christophe + Freelance walk-throughs.** Both must work end-to-end on `secretariat.equanimi.tech` before pitch claims done.

* **Themia channel set.** Need explicit list pre-pitch: `assemblee_generale` (exists), plus operating channels (e.g. `finance`, `legal`, `dev-project-x`?). Enumerate before inviting Christophe.

* **Agent identity verification chain on paper.** 4 hops; trace once before code lands.

* **Stamp ceremony body-render rule.** `[[show-drafts-before-signing]]` holds. Body renders verbatim in chat BEFORE Touch ID gate. Comprehension risk lives at stamp, not at sign (signing is autonomous, body authored by Claude in conversation already).

* **Edit-at-most edge case.** Principal materially edits a Claude-signed envelope → cleanest: principal signs own version, state #3 in trust model. Out of scope for this slice; revisit when edit-tooling demand materializes.

## Pitch

### Problem

Themia onboarding for Christophe (co-owner) and a partial-access freelance is blocked by two converging issues: (a) own-org publication fails today because `is_local` short-circuits the send path (the walkthrough bug from 2026-05-21), and (b) the substrate's authorship model assumes the principal signs every envelope, which doesn't survive AI-volume traffic where Claude is the primary composer. The first is plumbing. The second is a substrate reframe.

The deeper insight: Secretariat is git-for-company-comms. Every envelope is a signed commit. Channels are protected branches. Stamps are maintainer sign-offs. Claude is a first-class committer with its own DID, signing on the principal's behalf. Principals become reviewers-and-stampers, not exclusive authors. This isn't a future evolution — it's the model the README already promised, just unshipped. The drop-outbox slice (yesterday) cleaned the staging axis; this slice cleans the authorship axis and unblocks Themia.

**Sign ≠ stamp cryptographically.** Claude's signing path is hot (every envelope, continuous); principal's signing path is cold (Touch ID only). Sharing keys = co-locating cold credentials with hot runtime, a credential-hygiene anti-pattern. Separate keys also make the sign/stamp distinction verifiable at the protocol layer rather than a UI convention pasted on identical signatures. A receiver inspecting an envelope can cryptographically distinguish "Claude-composed" from "principal-composed" from "Touch-ID-gated-by-principal" because the keys differ. With shared keys the distinction is narrative-only.

### The bet

Medium-appetite slice. Seven moves, landing together:

1. Add explicit `sec agent add <name> --role scribe --substrate <substrate>` ceremony (CLI + Tauri cognition-provider selection screen). Provisions agent DID, binds via `authorized_agents` entry with `{did, role: "scribe", name, substrate, added_at}` shape in principal identity record. `sec init` does NOT provision an agent. Today only `--role scribe` + `--substrate claude-code` ships.
2. Route MCP `compose` through scribe-key signing — no Touch ID at compose time.
3. Collapse substrate to bare-slug channels — no namespace prefixes, no DM, no peers, no contacts.
4. Delete `_drafts/` and `sent/` directories — one envelope state, `delivered:` frontmatter field.
5. Delete `crates/core/src/application/send_envelope.rs` — federation moves to daemon as background plumbing, no user-facing send verb.
6. Land `requires_stamp` field in channel `contract.md` — receiver-side discipline.
7. Fix the walk-through bug by deleting `is_local`/`SelfAddressed` and replacing with org-membership-aware routing inside the daemon.

Pays off because (a) the model finally matches the README — Claude composes, principal reviews, stamps are curation; (b) Themia's two onboarding cases walk cleanly through the same substrate primitives; (c) the substrate stops carrying three addressing models in parallel; (d) AT-URI adoption lands cleanly on this base in a follow-up; (e) later hardening (counter-stamp, roster gate) extends naturally from this coherent base; (f) simplification reduces ongoing maintenance load disproportionately to the diff size.

### No-gos

* No AT-URI / lexicon codegen (separate pitch)

* No counter-stamp / multi-party (later slice)

* No relay-side roster enforcement (later slice)

* No multi-device sync

* No DM / peer / bilateral primitives (removed, not deferred)

* No CLI interactive draft staging

* No hierarchical key derivation — flat scribe DIDs in `authorized_agents`

* No user-facing `send` verb — federation is daemon-internal plumbing

* No wire-format change beyond additive `authorized_agents` entry shape (`{did, role, name, substrate, added_at}`), additive `delivered:` frontmatter, additive `requires_stamp` channel-contract field

* No new dependency on `notify`, no daemon-watcher redesign

* No change to envelope canonical signature preimage shape — agent signs the same way principal signs; only the key differs

* No public-by-default repos, no Bluesky network membership

* No Tauri shell redesign — review surface already exists; this pitch wires agent-identity into it

* No stamp ceremony shape change — Touch ID gate, body-render-first remain intact

* No edit-tooling for principal-edits-Claude-output — defer to follow-up

