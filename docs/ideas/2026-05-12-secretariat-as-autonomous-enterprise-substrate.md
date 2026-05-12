# Secretariat as the operational substrate of an autonomous enterprise

**Date:** 2026-05-12
**Status:** shaping report, pre-pitch
**Predecessor docs:**
- `docs/milestones/2026-05-05-substrate-and-menubar.md` (v0.3 substrate + tray — now narrowed scope)
- `docs/milestones/2026-04-30-first-signed-message.md` (Day 1 milestone, shipped)
- Marcelo Ballestiero, *Autonomous Enterprise* (245pp draft, April 2026)
- Brainstorm notes m.2–m.6 (2026-05-12, source for this report)

---

## The shift in one sentence

**Secretariat is the context substrate for autonomous enterprises.**

This is the new elevator tagline (2026-05-12). It replaces the prior "bilateral correspondence primitive" framing with a sharper one-line positioning. Secretariat moves from a two-party stamp-and-send protocol to a **multi-principal organizing system** where AI agents draft, humans curate via selective stamping, and verified envelopes serve as the durable breadcrumbs of decisions that both humans and agents continuously act on.

The relationship to *Autonomous Enterprise* (Marcelo Ballestiero's book, 245pp draft, April 2026) becomes explicit and recursive: the book describes the framework; Secretariat is one operational instance of it. Building the system *using* the framework while *writing* the framework is the strongest possible validation loop.

The product/CLI name remains **Secretariat** (the executive-secretary metaphor — the first-class agent embodying that role keeps the name). The framing for *what Secretariat helps you operate* is **an autonomous enterprise**. The experience-level tagline (*async generative communication for professionals, stamped by humans*) still holds at the UX/copy altitude — both taglines coexist.

---

## Problem the direction addresses

The v0.2.x line shipped a working bilateral protocol — Rafa ↔ Marcelo, Rafa ↔ Christophe — and validated:

- The signed envelope substrate works end-to-end (CLI + MCP + Tauri shell)
- The "show body verbatim → Touch ID → stamp" ceremony is sound
- did:key + did:web both function for identity rooting
- Selective transports (Gmail bootstrap + planned relay) compose cleanly

But three signals from real use surfaced limits of the bilateral framing:

1. **Marcelo's 2026-05-04 onboarding** (audit: `docs/audits/2026-05-04-onboarding-ux.md`) — sending one-off messages between two principals felt clunky and ceremonial. The friction was disproportionate to the surface area.
2. **Themia's actual workflow** — Christophe and Rafa coordinate across distinct domains (dommage-corporel, droit-du-travail, baux-commerciaux), with sub-cohorts inside each (paris cohort, regional cohorts). Bilateral DMs flatten this structure into a noisy stream.
3. **The autonomous-enterprise alignment** — Marcelo's book describes the org as a substrate that humans and agents inhabit together, with decisions persisting as breadcrumbs. Bilateral DMs don't compose into that.

The pivot: move the primitive from **the principal-pair** to **the channel inside an org**, with the principal-pair becoming a special case (channel-of-two).

---

## The new direction

### Core paradigm

Three layers of trust, replacing the prior "every envelope is stamped" model:

| Layer | Mandatory? | Authority | Purpose |
|---|---|---|---|
| Signature (DID-keyed) | Yes — every envelope | Author (human or agent DID) | Provenance |
| Stamp (Touch-ID) | No — selective | Principal | Attestation / commitment |
| Counter-stamp | No — multi-party | Multiple principals | Joint commitment (process-verbaux) |

Most envelopes flow signed-only — ambient context written by agents or humans. The *stamped subset* is the org's authoritative decision ledger. Curation, not transport gating. The tagline holds, sharper: *async generative communication for professionals, stamped by humans* — humans stamp **what matters**.

### Five primitives that compose the lived experience

1. **Channel tree.** `(owner_did, handle:dept:team:subteam)` — colon-pathed handle. Tree depth = colon depth. Nested channels emerge for free from the existing substrate. Primary navigation is the tree.

2. **Channel membership.** Subscribe / publish roles per member. Roster lives in a meta-channel; view-only members (Christophe at start) get subscribe-only. Roster mutations are signed envelopes — everything is envelopes.

3. **Consumption contract.** Per-member-per-channel, local file at `~/.secretariat/contracts/<org>/<handle-path>.md`. Declares cadence, depth, notify, filter. Inherits from global attention-envelope. **Never published** — pairs with the "no read receipts" invariant.

4. **Cross-channel review.** Primary review surface. MCP and CLI verb `review` — tree-grouped digest scoped by glob, timeframe, contract filters. Session ends naturally when queue empties. Supersedes the inbox/outbox split from v0.2.0.

5. **Skill scope inheritance.** Channel-context resolution walks up the tree, accumulating skills + instructions. Channel-local, dept-scoped, org-wide, or global — all addressable by directory position. "Dive in" mode loads the resolved skill set as active.

### The first agent — Secretariat

The product is named after this role; the agent embodies it. First-class, shipped by default, runs locally:

- Has its own `did:key`, distinct from the principal's identity
- Reads subscribed channels, applies consumption contracts, drafts personalized digest envelopes to a local digest queue (`inbox:digest:morning` by default)
- Triggered by schedule (daemon cron) or on demand (`sec digest now` / MCP `digest`)
- Signs digests with its own DID; never auto-stamps. Principal stamps in review if the digest is a record worth keeping.

The roundtable workflow already in `~/.claude/skills/roundtable/` unifies with channel review — same verb, different scope. Roundtable becomes a named review profile (`_self/inbox:*` scope with shaping skills). One mental model, not two.

### Channel context delivery — meta-channel pattern

A channel's instructions, skills, and roster live in a co-located meta-channel `<channel-uri>:_meta`. Members subscribe to the meta-channel alongside the channel itself; daemon resolves meta-envelopes to local disk. Updates propagate over existing transport.

**Why not shared git as authoritative store:** git host introduces a central server (invariant violation), loses provenance unless signed separately, and adds an unnecessary adapter. The meta-channel pattern preserves "everything is envelopes."

**Shared git as optional upstream adapter is fine.** Teams that prefer PR-based skill iteration can wire git → CI → meta-channel. Git becomes transport (like Slack-as-transport); the envelope remains the unit of trust.

### Trust extension to external systems

Two modes for non-principal participants:

1. **DID-keyed external service** — service has its own DID, signs envelopes itself. Webhook is pure transport. Service appears as a publishing member in the channel roster.
2. **Agent-proxied** — local adapter agent with its own DID wraps external payload (Sentry alert, GitHub webhook), signs with attribution metadata. Provenance chain visible: "agent attested that source X said this."

MCP tools for human members come for free — `publish_to_channel(uri, body)` composes, signs with the member's DID, posts. Webhook adapter is a v0.4 wedge that opens the "external sources flowing into channels" story.

---

## Infrastructure decisions

### The channel directory IS the Claude Code activation surface

The vision sharpens: the principal `cd`s into a channel directory and runs `claude`. The session inherits everything for free. The same directory is the working directory for always-on headless agents spawned by the daemon via Claude Agent SDK. One activation surface, two consumption modes.

Each channel directory is **literally a Claude Code project**, using the standard `.claude/` convention. Claude Code's existing tree-walk handles scope inheritance for free — no custom resolver, no parallel layout, no learning curve.

```
~/.secretariat/
  themia.pro/                                    (alias for did:web:themia.pro)
    .identity                                    (canonical DID + key refs)
    CLAUDE.md                                    (org-wide context)
    .claude/
      agents/                                    (org-wide agents)
      skills/                                    (org-wide skills)
      commands/                                  (org-wide slash commands)
    channel/
      dommage-corporel/
        CLAUDE.md                                (dept-wide context)
        .claude/
          agents/                                (dept-wide agents)
          skills/
        paris-cohort/
          CLAUDE.md                              (channel-leaf context, resolved from meta)
          .claude/
            agents/                              (channel-leaf agents)
            skills/
            commands/
          instructions.md
          contract.md                            (MY consumption contract for this channel)
          envelopes/                             (decrypted history, time-sharded)
            2026/05/12/
              2026-05-12T09-23Z-7f3a.md
          outbox/                                (drafts daemon picks up to sign+send)
          meta/                                  (resolved meta-channel envelopes)
          _ciphertext/                           (encrypted-at-rest blobs for transport)
```

`cd ~/.secretariat/themia.pro/channel/dommage-corporel/paris-cohort && claude` activates:

- `CLAUDE.md` walks UP the tree — channel + dept + org context all loaded by Claude Code's native resolver
- `.claude/agents/` at each ancestor level discovered with leaf-overrides-ancestor precedence
- `.claude/skills/` at each level inherited the same way
- `.claude/commands/` at each level inherited the same way
- `envelopes/` grep-able for history
- `contract.md` declares principal's consumption preferences
- Drafts written to `outbox/` get picked up by daemon, signed with the principal's or agent's DID, and queued for transport (or for principal stamp if elevation is needed)

The same shape powers always-on agents. The daemon launches per-channel Claude Agent SDK loops with the channel directory as cwd. Per-channel agents (`.claude/agents/triage-incoming.md`) and skills (`.claude/skills/summarize-daily.md`) declare the duty cycle. Triggers: cron, filesystem watch on `envelopes/`, daemon RPC. Outputs flow through `outbox/` and get ferried by the daemon — same path the interactive principal uses.

This collapses several earlier abstractions into "the directory tree IS the scope tree":

| Earlier abstraction | Now |
|---|---|
| Custom skill / agent scope resolver | Claude Code's existing `.claude/` + CLAUDE.md tree-walk |
| "Dive in" mode | `cd <channel-dir>` |
| Resolved cache directory parallel to channel | `meta/` co-located inside channel |
| Per-channel skill / agent indexing | `.claude/{skills,agents}/` at each level |
| Top-level scattered state (`queues/`, `cache/`, `contracts/`, `skills/` all separate) | One per-channel directory containing everything, structured as a standard Claude Code project |

**Path aliasing.** DIDs in raw form (`did:web:themia.pro`, `did:key:z6Mk...`) are visually noisy and have `:` portability issues on some filesystems. Use friendly aliases on disk — `themia.pro/`, `marcelo/`, `christophe/` — with the canonical DID stored in `.identity` at the alias root. Daemon maintains the alias-to-DID map; on-disk paths stay human, canonical addresses stay cryptographic.

**Principal overrides vs daemon-resolved files.** The daemon writes meta-channel-resolved content (channel-level `CLAUDE.md`, files under `skills/`, etc.). The principal hand-edits to override. Convention: `*.local.md` is principal-authored and never touched by the daemon; the daemon's resolver writes `*.md` and respects the existence of a `*.local.md` override.

### Filesystem stays authoritative

Markdown-in-filesystem is the source of truth. The AI feedback loop is the architectural moat — Claude (and any future LLM) reads markdown directly, no SDK, no proprietary format. Switching to a database-as-authority would close the loop. Don't.

Layout time-shards from day one — `<channel>/envelopes/YYYY/MM/DD/<iso-timestamp>-<hash>.md`. Prevents pathological flat directories. Two-tier storage co-located inside each channel directory: `_ciphertext/` holds encrypted-at-rest blobs (what crosses transports); `envelopes/` holds the decrypted markdown (what Claude and grep see). Key never leaves the device. See the channel-directory-as-activation-surface section above for the full layout.

SQLite (or similar) is an optional **read** cache for cross-channel queries — daemon-maintained, regenerable from a full walk, never authoritative. Defer to v0.4+ when query latency demands it.

### Owner-as-sequencer for consistency

The hard question: if every member has a local filesystem copy, how do they all have the same view at the same time, especially agents acting in the organizing system?

Resolution: each channel has exactly one canonical sequencer — the **owner's relay/daemon**. Subscribers read the owner's sequence. Decentralization happens at the cross-org level (many channels × many owners), not within a channel. Same shape as email federation or Slack-per-channel ordering, just without a central company.

Two consumption modes on the same primitive:

- **Humans:** cadenced poll, 15-min floor (anti-compulsion, per the vision tagline)
- **Agents:** long-lived push subscription (WebSocket/SSE). Sub-second freshness. Same append-only sequence, faster transport.

Filesystem-authority sharpens per-channel-per-owner: **owner's filesystem is canonical** for channels they own; **subscriber's filesystem is a synchronized cache**. Lose subscriber disk → re-sync from owner. Free natural redundancy emerges: every subscriber is a partial backup of channels they read.

Cross-channel global ordering is explicitly **not** provided. Each channel is its own log. If agents need cross-channel causality, express it via envelope-hash references. No consensus protocols, no central registry, no Byzantine fault tolerance — those would close the substrate.

### URI grammar

Canonical: `did:web:themia.pro#channel:dommage-corporel:paris-cohort`

- Inside the handle: all colons (`channel:dept:team:subteam`). Tree depth = colon depth. Consistent with existing substrate grammar (`inbox:default`, `inbox:triage`).
- Between DID and handle: `#` — W3C DID URL fragment grammar. Channel is sub-resource of identity.
- Wire format: substrate keeps `(owner_did, handle)` as two fields. Composite URI is display only.

Keystone: **one identity, many queues.** Rejecting "one DID per channel" preserves the org-as-identity model.

### Portability is a first-class property

Filesystem-authoritative + AI-readable markdown deliver substrate-level portability for free — no design work needed for the basics. `tar`, `rsync`, `git`, `cat`, any LLM — they all work because the substrate is plain files in well-known locations.

| Operation | Mechanism today |
|---|---|
| Move a channel between machines | `tar -czf <channel-dir>` → unpack |
| Move whole installation | `rsync ~/.secretariat/` to new machine |
| Backup / restore | git, tar, Time Machine — pick your tool |
| Inspect on a foreign machine | `cd && cat`; or run Claude Code against the dir |
| Recover from disk loss (subscriber) | Re-sync from owner relay |
| Recover from disk loss (owner) | Re-hydrate from subscriber caches (per the owner-as-sequencer redundancy property) + user-encrypted key backup |
| Fork a channel | Copy the directory tree, re-root under own org DID, optionally re-sign — works without any design |

Gaps to design when concrete drivers appear (not v0.3):

1. **Multi-device same-principal sync.** Per AGENTS.md invariant #3 (keys never leave the device), running on laptop + iPhone requires user-encrypted key migration. Shape: `sec identity export --passphrase` → encrypted key bundle; `sec identity import` on the second device unlocks. v0.4 wedge.
2. **Channel ownership transfer.** New `rosterUpdate.op = transfer_ownership` + re-signed `channelDef` under the new owner's DID. Subscribers update their relay endpoints. Lexicon extension when needed.
3. **Org-level migration.** Moving an entire org to a new owner DID or domain. Deferred until a real driver appears.

The architectural decisions made elsewhere in this report — filesystem authority, markdown bodies, owner-as-sequencer, no central server — already make Secretariat strictly more portable than any SaaS communication tool. Worth saying so explicitly in the elevator pitch: *your data is yours, in plain files, on your disk, forever.*

### CRDTs explicitly rejected for core

Yjs / Automerge solve eventual consistency on mutable state with concurrent edits. Secretariat doesn't have mutable state — envelopes freeze at signature. The signed append-only log is strictly stronger than CRDT for this problem (provenance + immutability + total order per queue).

If multi-author **pre-sign** drafting becomes a real use case (e.g. Rafa + Christophe co-drafting a process-verbaux before joint stamp), revisit Automerge (Rust bindings, doc-shape fit) for the drafting layer only — with freeze + sign at commit boundary into the immutable envelope substrate.

---

## What was superseded along the way

The shaping conversation invalidated several earlier v0.3-era decisions:

| Prior decision | Why superseded |
|---|---|
| "v0.3 is pure simplification" — every slice net subtractive | New direction is necessarily additive (orgs, channels, membership, push). Subtraction principle re-targets v0.2.x chrome only. |
| "Two-buttons home + inbox/outbox split" | Tauri walker removed entirely; inbox/outbox split obsolete with channels. Review session principle survives; surface moves to MCP `review` verb. |
| "Every sent envelope is stamped" (AGENTS.md rule #4) | Stamp is selective weight. Volume tension dissolves at framing level. AGENTS.md rule #4 needs revision. |
| "Batch-stamp Merkle ceremony" | No longer needed once stamp became selective. Design preserved as future affordance, not in v0.3 critical path. |
| "Filesystem authoritative globally" | Sharpened to per-channel-per-owner — subscribers are synchronized caches with re-sync recovery. |

---

## v0.3 revised ship order

Re-shaped from the prior `docs/milestones/2026-05-05-substrate-and-menubar.md`:

1. **Org primitive** — `OrgDid` value object, did:web org-doc resolution. Org doc advertises channels + roster.
2. **Channel membership + owner-as-sequencer** — relay endpoint serves monotonic per-channel sequence; subscriber poll (humans) + push subscription (agents). Catch-up protocol for new members.
3. **Invite v2** — invite-to-org and invite-to-channel, extending the existing invite-as-correspondence pattern.
4. **Channel-scoped context (meta-channel pattern)** — `<channel>:_meta` carries instructions + skills + roster as signed envelopes. Daemon resolver populates local cache; principal-authored overrides take precedence.
5. **Layered verify + stamp-as-verb** — `sec verify --json` returns `{signature, stamp, counter_stamps}` separately. `sec stamp <envelope-id>` operates on existing signed envelope. AGENTS.md rule #4 revised to selective-weight model.

Deferred to v0.4+: webhook adapter for external systems; full agent-DID + counter-stamp ledger (m.3 process-verbaux); SQLite read cache; freelancing billable-hours concrete wedge; shared-git skill iteration adapter; Perceive-style KG over verified envelopes; PDF-share of verified envelopes.

Explicitly rejected: read receipts, urgent mode, central registries, vendor-hosted org infrastructure, CRDTs in core substrate, cross-channel global ordering, real-time mode for human consumption.

---

## Concrete v0.3 cohort

Three orgs to validate the design:

- `did:web:equanimi.tech` — Rafa's org, channels for secretariat dev work + book progress with Marcelo. First channel: `equanimi.tech#channel:secretariat:dev`.
- `did:web:themia.pro` — Themia, invite Christophe as view-only member with elevated role on `themia.pro#channel:dommage-corporel:general`. Validates subscribe-only role.
- A third org (Nwyana or similar) — validates multi-org membership for the principal.

---

## Open questions to resolve in the pitch phase

1. **Roster mutation propagation** — new member can't see `_org/roster` before they're in it. Invite-v2 must carry a roster snapshot at claim time. Subsequent updates flow through the meta-channel.
2. **Channel discovery within an org** — org DID doc lists channels; how granular? Public channels only, or all channels with per-channel visibility metadata?
3. **Subscribing across orgs** — Rafa is in three orgs. Default review scope is "all subscribed channels filtered by contracts." Does priority ordering need explicit declaration, or does cadence in the contract suffice?
4. **Relay sovereignty for did:key principals** — Marcelo/Christophe/dad need a reachable relay. Self-hosted requires a domain; peer-hosted is the v0.3 answer. Community-relay UX needs design.
5. **Catch-up history bounds** — when a new subscriber joins, default to "since claim" or full history? Per-channel policy or per-invite parameter?
6. **Push subscription resource cost** — N agents × M channels × push = persistent connection count on the owner's relay. Sustainable for v0.3 personal scale; revisit at org-scale.
7. **Editing channel context** — meta-channel envelopes are append-only; how is "current state" derived? Latest-wins per skill slug, or explicit supersedes-refs? Same model as roster updates probably wins.
8. **Attention routing + UI (v0.4 wedge, not v0.3).** The envelope already carries `depth` + `urgency` as attention proposals; the receiver's `attentionEnvelope` declares acceptance criteria. The routing daemon that evaluates proposal × bounds × per-channel contract — deciding which channel/queue an inbound envelope lands in, whether to surface, defer, digest, or bounce — composes from existing primitives but is unbuilt. UI affordances for principal review of routing decisions (what got promoted, what got filtered, why) also future. Wait for 2-3 weeks of real channel traffic before designing rules; armchair rules will be wrong.

---

## Naming and framing

Two distinct layers, no portmanteaus:

- **Secretariat** — the product/CLI/daemon you install. The executive-secretary metaphor is sharp; the first-class agent embodies it. Keep the name.
- **Autonomous enterprise** — what Secretariat helps you operate. This is Marcelo's framework (the book); the substrate Secretariat creates is one operational instance of an autonomous enterprise. Use this framing in pitches, onboarding copy, and conceptual documentation.

The relationship is recursive in the strongest way: the book *describes* the autonomous enterprise; Secretariat is *built using* the autonomous enterprise pattern, *to enable* autonomous enterprises. Building the system while writing the book is the validation loop.

(An earlier portmanteau attempt — "orgosystem" — was rejected. Plain language wins; the book already has the right vocabulary.)

---

## Next steps

1. Update AGENTS.md rule #4 to reflect selective-stamp model. Touch the wording when shaping slice 5; don't push a doc-only commit before the code lands.
2. Draft pitch at `docs/pitches/2026-05-12-orgs-channels-ledger.md` from this report. Audience: Rafa-for-shaping, then Marcelo as primer for the autonomous-enterprise alignment.
3. Validate URI grammar with a small lexicon update (no runtime validation yet, just shape).
4. Sketch the owner-as-sequencer relay endpoint shape — single-sequence + cursor + push — to confirm slice 2 is implementable on the existing relay crate.
5. Decide whether to start the v0.3 implementation in the current branch or fork off a dedicated feature line. Subtractive principle (per the older v0.3 memory) suggests staying on main and deleting as we go; additive surface suggests a feature branch. Open question.

---

*Drafted by AI, reviewed by a human.*
