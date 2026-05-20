---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:640b2d5d62c82d5cb34ad3df507fe2fd75046e68fcc91330cbc8016acd290a14
  docFilename: 2026-05-18-channel-relay-sequencer.md
  stampedAt: 2026-05-18T10:17:35.781272Z
  signature: ed25519:nBn267S4+7hXOaVuPriAZuC3EFuSA3bDHJ2J0+64hSAmpM6qLnrL6N/fYFNtgvOo8UNfGzT+dqeL4BC09pzEAA==
---
# Owner-as-sequencer channels on the relay (v0.7 → v0.8)

Pitch — 2026-05-18 (revised twice: once after grounding in code, once after expanding scope from "envelope sync" to "channel-dir sync"). Source: conversation thread 2026-05-18 ("outbox / rsync / Kafka / Beelay / everything is files"), grounded in `crates/relay/src/state.rs`, `crates/relay/src/queue.rs`, `crates/daemon/src/serve.rs`, `crates/core/src/application/sync.rs`, `lexicons/tech.equanimi.secretariat.channelDef.json`, `lexicons/tech.equanimi.secretariat.rosterUpdate.json`, `lexicons/tech.equanimi.secretariat.orgDoc.json`, `docs/decisions/2026-05-12-substrate-layout-v03.md`, `docs/pitches/2026-05-17-collapse-namespaces.md`, AGENTS.md architectural invariants #4 and #8.

**Hard dependency:** v0.7.0 layout-complete is shipped. Channel-tree directory layout exists locally; lexicons for channelDef + rosterUpdate + orgDoc are fully specified; the relay's DM substrate (\~2.1K LOC) is working. This pitch wires the existing parts into a channel protocol AND adds the missing primitive (`fileUpdate` + blob substrate) so the *whole channel directory* — not just its `envelopes/` subtree — syncs between owner and subscribers.

## What already exists (do not rebuild)

* **Relay** — `state.rs` has `enqueue(recipient, body, content_type, sender_did, now) -> u64` + `since(recipient, after) -> Vec<QueuedEnvelope>`. `queue.rs` `TenantQueue` is shape-agnostic (monotonic `next_id`, `push` / `since` / `prune_older_than`). Persisted via `persist.rs` (versioned `StateFile`). JWT-style DID challenge-response auth (`auth.rs`, 261 LOC).

* **Lexicons** — `channelDef` (handle grammar, visibility, relayHint, parent), `rosterUpdate` (signer/op/subject/channel URI/roles `[subscribe, publish, admin]`/effectiveAt/signature), `orgDoc` (org identity, relays, advertised channels), `envelope`, `stamp`, `identity`, `contact`, `org` — all defined. The rosterUpdate doc specifies the "replay envelope stream filtered by `$type` → derive roster, latest-wins" model already.

* **Daemon** — `serve.rs` runs `sync_now` cycles; `application/sync.rs:88` `sync_now()` with per-relay cursor state via `RelayState::load`/`save`. `decide_poll` + `CadenceConfig` + `PollDecision` implement the cadence model. `launchagent.rs:172` exposes per-relay cursors for health checks.

* **Drain** — `application/sync.rs:332` `drain_outbox` sends stamped envelopes to the relay.

* **Architectural invariants** — owner-as-sequencer per channel (\[\[project\_owner\_as\_sequencer]]); filesystem-authoritative (\[\[project\_filesystem\_authoritative]]); namespace collapse with structural mutations as `$type`-tagged envelopes on the main stream (\[\[project\_namespace\_collapse\_drops\_meta]]); no CRDT in core (\[\[project\_no\_crdt\_in\_core]]).

## Boundaries

### Job to be done

As a principal subscribed to `did:web:themia.pro#dommage-corporel:paris-cohort`, I want my local channel-dir — `envelopes/`, `CLAUDE.md`, `.claude/skills/`, `template.md`, `contract.md`, `channel.md` — to mirror what the owner publishes, so that `cd <channel-dir> && claude` activates the same agent context the owner intended (\[\[project\_channel\_dir\_is\_activation\_surface]]). Messages flow as one *kind* of file; agent context flows as another; both ride the same per-channel ordered stream because the substrate's invariant #8 says **everything is a file**.

**The** ***when*****:** Rafa wants to post to `dev:secretariat` and have Christophe + Marcelo see the messages AND inherit the channel's `CLAUDE.md` + skills the moment they subscribe. Today the relay can't carry channel-keyed traffic at all, and the design split that ships envelope-sync first and file-sync later would guarantee two divergent sync primitives.

**The baseline:** today this is unreachable. The pieces (lexicons for structural records, daemon cursor infrastructure, queue persistence, auth, handle grammar, local channel-dir layout) are all ready. The second index axis on the relay doesn't exist; the file-sync primitive doesn't exist; the blob substrate doesn't exist.

### Appetite

`big`

~2 weeks. Real new work is the blob substrate + `fileUpdate` lexicon + path validation + roster-symmetric encryption; the channel-stream sequencing piece is essentially what previous pitch revisions had. Override with `--appetite=medium` only if we split file sync off (option B from the parking discussion) — but the architectural cost of that split is what makes A the right call.

## Elements

Fat-marker sketch — six primary elements (one more than v1 because file sync is in scope).

* **Place — second index axis on the relay.** `AppState` (`crates/relay/src/state.rs:77`) gains a sibling field: `channels: RwLock<HashMap<(Did, ChannelHandle), TenantQueue>>` next to the existing `queues: RwLock<HashMap<Did, TenantQueue>>`. `TenantQueue` reused as-is. New methods `enqueue_channel(owner, handle, body, content_type, sender_did, now) -> u64` and `since_channel(owner, handle, after) -> Vec<QueuedEnvelope>` parallel the DM ones. Persistence layer extends its `StateFile` with a `channels:` map and a `blobs:` map (see next element). \~120 LOC.

* **Place — content-addressed blob substrate.** Files over ~4KB inline (skill bodies, attachments, longer CLAUDE.md sections) land in a per-relay blob store keyed by blake3 hash: `blobs: HashMap<BlobHash, BlobEntry>` on `AppState`; persisted under `<data_dir>/blobs/<hash-prefix>/<hash>`. Routes `POST /blobs` (upload, returns hash) and `GET /blobs/:hash` (download, gated on caller being in *some* roster that references the hash — defense in depth, not strict). GC runs against TTL or "no envelope/fileUpdate references this hash" condition. \~250 LOC.

* **Affordance —** **`fileUpdate`** **lexicon as the channel-dir sync primitive.** New lexicon: `tech.equanimi.secretariat.fileUpdate` carrying `{path, op: write|delete, content_inline | blob_ref, content_hash, signer, signature, effectiveAt}`. Lands in `lexicons/` in the same commit per hard rule #3. Path validation refuses absolute paths, `..` traversal, and the `*.local.md` suffix (private-by-extension per hard rule #6). Replay = walk all `$type: fileUpdate` records ordered by `effectiveAt` then TID, apply last-writer-wins per path. Per \[\[project\_no\_crdt\_in\_core]] this is signed append-only log; no merge logic. \~80 LOC plus lexicon JSON.

* **Affordance — roster derivation and roster-symmetric encryption.** Per the existing `rosterUpdate` lexicon ("Roster state is derived by replaying ordered roster updates filtered by `$type` from the channel's main stream — latest-wins per subject DID"), the relay reads `channelDef` + `rosterUpdate` envelopes as they're enqueued and maintains a cached `roster: HashMap<(Did, ChannelHandle), HashMap<Did, RoleSet>>` for fast gate-checks. `since_channel` returns empty if caller's authenticated DID lacks `subscribe`; `enqueue_channel` rejects if it lacks `publish`. Envelope body + fileUpdate content\_inline are encrypted with the channel's roster-symmetric key (shared with all members on `rosterUpdate.op = add`). **Key rotation on** **`remove`** **is deferred to v0.4** (explicit limitation: removed members retain decryption of pre-removal content; already true at filesystem level — they have local copies). \~200 LOC.

* **Affordance — owner sequence-witness on the wire envelope.** When `enqueue_channel` assigns `seq = N`, the relay also signs a `witness: {owner_did, channel_handle, seq, envelope_hash, witnessed_at, signature}` field attached to the `QueuedEnvelope` returned by `since_channel`. Author's own DID-keyed signature on body is unchanged. Two layers: author signature attests *content*, owner witness attests *position*. **Requires the relay to hold the owner's signing key** — pick the single-tenant-relay-deployed-as-owner's-daemon path (matches Railway/sovereignty model; `crates/relay/src/main.rs` already loads one config-bound identity). \~80 LOC.

* **Connection — daemon channel subscription + replay loop.** Daemon's `RelayState` cursor map extends to `channel_cursors: HashMap<(OwnerDid, ChannelHandle), u64>`. `sync_now` grows a sibling channel-pull loop: for each subscribed channel, call `since_channel(owner, handle, cursor)`, verify witness against owner's pubkey, decrypt with roster-symmetric key, dispatch by `$type`: `envelope` → write to `envelopes/YYYY/MM/DD/`; `fileUpdate` → write to `<path>` (or delete) with path validation re-checked at the receive side; `rosterUpdate` → cache update; `channelDef` → channel-metadata update; advance cursor; save state. Reuses `decide_poll` + `CadenceConfig` for tick scheduling. New CLI verb `sec subscribe <channel-uri>` (URI per \[\[project\_queue\_uri\_grammar]]: `did:web:themia.pro#dommage-corporel:paris-cohort`) + matching MCP tool, following the four-surface rule. \~300 LOC.

## Risks

### 🐇 Rabbit holes

* **Relay needs owner's signing key for witness signatures.** Same as v1 of the pitch — pick single-tenant-relay path. Owner's daemon process *is* the relay; reads signing key from disk at startup. Defers capability delegation. Document the trade in `docs/developer/secretariat-architecture.md`.

* **Roster-symmetric key bootstrap on** **`rosterUpdate.op = add`.** When the owner admits a new subscriber, the existing roster-symmetric key must be conveyed to them. Path: encrypted to the new subscriber's DID-derived x25519 key (already in use for DM body encryption) and shipped as a `keyEnvelope` body alongside the `rosterUpdate` envelope. New small lexicon `tech.equanimi.secretariat.keyEnvelope`? Or fold into rosterUpdate via optional `wrapped_channel_key` field? Lean fold-into-rosterUpdate for v0.8 — one less lexicon to ship.

* **Blob GC correctness.** A blob is referenced by an envelope or fileUpdate. If the relay prunes the referencing envelope (TTL) but the blob is still pulled by a slow-catching-up subscriber, broken read. Mitigation: blob TTL ≥ envelope TTL + max-subscriber-lag (e.g. 30 days). Or: reference-count blobs at enqueue/prune time. Pick reference-count for correctness.

* **File-path validation paranoia.** Subscribers receive `fileUpdate` with attacker-controlled `path` field. Must refuse `..`, absolute paths, `*.local.md`, `.git/`, anything outside the channel-dir. Validation runs on BOTH relay (refuses to enqueue) and subscriber (refuses to write). Belt-and-suspenders; the cost of getting this wrong is local FS escape.

* **Skill auto-activation.** `.claude/skills/foo.md` arriving via fileUpdate writes the file — but Claude Code's tree-walk skill loader will then auto-load it on next `cd`. Trust gap: subscriber's principal didn't author this skill. **Mitigation: subscriber daemon writes incoming** **`.claude/skills/*.md`** **to a sibling** **`.claude/skills.incoming/`** **quarantine dir; principal explicitly moves to active.** Same model as deferred email attachments — landed, not opened.

* **Replay performance on subscribe.** A channel with 1000 envelopes + 50 fileUpdates means the subscriber pulls + verifies + writes all of them on first sync. Manageable at v0.8 traffic. Time-shard fetches (per-month windows) only if it becomes a UX issue.

* **Owner relay restart + replay idempotency on seq.** If owner's relay restarts from a backup behind the published state, must never re-assign a previously-witnessed seq. Persistence layer must fsync seq advances *before* returning to `enqueue_channel` caller. Verify `persist.rs` (133 LOC) supports this; if not, add.

* **Concurrent fileUpdate to same path.** Two admins write `contract.md` simultaneously. Owner-as-sequencer assigns distinct seqs; replay applies `effectiveAt`-then-TID-ordered last-write-wins. No merge. Documented limitation; if it bites, m.3 process-verbaux's Automerge layer (\[\[project\_no\_crdt\_in\_core]]) handles pre-stamp collaborative drafting separately.

### 🏴 Off-sides called

* **No push fanout.** Subscribers poll. The 15-min cadence floor for humans is anti-compulsion (\[\[project\_v03\_lived\_experience]]). Push (LongPoll/WebSocket) for agents is v0.4 work.

* **No cross-channel ordering.** Channels are independent logs. Cross-channel causality, if needed, expressed via envelope-hash references in body — never as protocol guarantee.

* **No Keyhive capability delegation.** Flat roster + role enum is what the lexicon already specifies. Convergent capabilities are research-grade.

* **No RIBLT / Sedimentree.** Linear cursor pull is enough at v0.8 traffic.

* **No CRDT or Automerge.** Decided (\[\[project\_no\_crdt\_in\_core]]). Append-only signed log; last-writer-wins per path. Reserve Automerge for m.3 process-verbaux pre-stamp collaborative drafting (separate v0.4+ wedge).

* **No** **`rosterUpdate.op = transfer_ownership`.** Already reserved-but-not-implemented in the lexicon's `knownValues`; defer until concrete driver appears.

* **No audience-bound envelope auth.** Beelay-style `{sender, audience, ts, sig}` envelope hardening — deferred separate pass. Existing JWT-style flow binds to relay domain at challenge issuance.

* **No** **`subscribe`** **RPC at the relay.** Subscription is daemon-local state.

* **No key rotation on** **`rosterUpdate.op = remove`** in v0.8. Removed members keep their copy of pre-removal content; that's already true at the filesystem layer. v0.4 wedge.

* **No skill auto-activation on receipt.** Incoming `.claude/skills/*.md` lands in `.claude/skills.incoming/` quarantine; principal promotes.

* **No multi-relay-per-org.** Defer `relayHint` per-channel resolution; assume orgDoc's default relay.

* **`*.local.md`** **never on the wire.** Hard rule #6. Watcher filters; relay refuses; subscriber refuses.

### 🥩 Fat to cut

* **Don't ship** **`keyEnvelope`** **as a separate lexicon.** Fold the wrapped channel-key as an optional field on `rosterUpdate` (only present for `op = add`). One fewer lexicon doc.

* **Don't model witness as a separate lexicon.** Folded as a field on `QueuedEnvelope` wire shape — same pattern as `rosterUpdate` riding the main stream rather than being a separate primitive.

* **Don't time-shard relay's in-memory channel queue.** Works for <10K entries.

* **Don't add per-channel** **`relayHint`** **resolution complexity.** Assume orgDoc's default relay for v0.8.

* **Don't build MCP-level channel subscribe surface beyond a thin wrapper.** CLI verb is enough for first traffic.

* **Don't add web UI for skill quarantine promotion.** `mv .claude/skills.incoming/foo.md .claude/skills/foo.md` is the v0.8 promotion path. Lift to a tray menu in v0.9 when there's actual quarantine volume.

* **Don't separate envelope content encryption from fileUpdate content encryption.** Both use roster-symmetric key; one code path.

### 🧪 Domain knowledge

* **Confirm** **`ChannelHandle`** **newtype already exists in** **`crates/core/src/domain/`** from the v0.5 namespace-collapse work. If yes, reuse; if not, add it in the same commit (lexicon defines grammar).

* **Confirm** **`crates/core/src/application/sync.rs::sync_now`** **is structured to accept a per-channel sibling loop** without re-architecting. Read its current shape before estimating daemon delta.

* **Owner key handling in the relay.** Today `crates/relay/src/main.rs` loads a single config-bound identity. Verify; if multi-tenant with per-tenant signing-key delegation, the picture changes.

* **Replay ordering for** **`rosterUpdate`** **and** **`fileUpdate`.** Per rosterUpdate lexicon: "replays are ordered by `effectiveAt` then by record TID." Verify the relay can extract `effectiveAt` from envelope bodies (cheap — parse only `$type ∈ {channelDef, rosterUpdate, fileUpdate}`; pass everything else through opaque).

* **Path validation library.** Use `std::path::Path::components()` walk for traversal detection; refuse any `Component::ParentDir` or `Component::RootDir`. No external crate.

* **Blake3 vs sha256 for blob hashing.** Blake3 is faster and already in some Rust crypto stacks. Verify nothing in `crates/core` standardizes a different hash for content addressing; if not, blake3.

* **Roster-symmetric key derivation.** XChaCha20-Poly1305 with the channel's symmetric key. Key length 32 bytes. Derivation from a channel "epoch" seed advertised on `channelDef` and re-wrapped per-member on `rosterUpdate.op = add`. Read existing DM encryption to match style.

## Pitch

### Problem

Lexicons define how a channel's structure works (`channelDef`, `rosterUpdate`, `orgDoc`). The daemon polls relays on a cadence. The relay queues envelopes per recipient and survives restarts. None of it composes into channels because (a) `enqueue` / `since` are keyed on a single recipient DID, and (b) the substrate's *files* — `CLAUDE.md`, `.claude/skills/`, `template.md`, `contract.md` — have no sync mechanism at all.

AGENTS.md invariant #8 says **everything is a file; the channel directory is the activation surface**. The earlier draft of this pitch split that surface: envelopes sync in v0.8, files later. That split contradicts the invariant — two sync primitives, two persistence shapes, two auth gates, guaranteed to diverge. The right shape is one primitive: the channel directory is the sync unit, envelopes are one kind of file inside it, structural records (channelDef, rosterUpdate) are another kind, and arbitrary content (CLAUDE.md, skills, template, contract) is a third — all riding the same per-channel ordered append-only log.

The local filesystem already has channel directories. The CLI and MCP have channel-aware commands. The handle grammar parses. The orgDoc lexicon advertises channels. The roster update lexicon defines mutations. *Every layer is ready except the relay's second index axis, the daemon's per-channel cursor, the file-update primitive, and the blob substrate.* This is the v0.3 design's load-bearing protocol slice, and "ship messages first" doesn't get there cheaper — it gets there *fragmented*.

Sync substrates exist (Beelay/Keyhive from Ink & Switch). Their problem domain is *concurrent mutable documents with capability-graph access*. Ours is *append-only signed envelope log with single-sequencer per channel + opaque file replication*. We steal the framing (membership/roster sync first, then content sync), skip the heavy machinery (CRDT, capability delegation, RIBLT, Sedimentree).

### The bet

Big-appetite slice (\~2 weeks). Six elements:

1. Extend `AppState` with `(Did, ChannelHandle)`-keyed `channels` queue alongside the per-recipient `queues`.
2. Add content-addressed blob substrate (`blobs:` map + `POST/GET /blobs` routes + reference-counted GC).
3. Ship `tech.equanimi.secretariat.fileUpdate` lexicon — `{path, op, content_inline|blob_ref, content_hash, signer, signature, effectiveAt}`. Path validation everywhere. `*.local.md` never on wire.
4. Derive roster from in-stream `channelDef` + `rosterUpdate`; gate `since_channel` on `subscribe` role and `enqueue_channel` on `publish` role. Encrypt envelope bodies and fileUpdate content with roster-symmetric key; wrap key per-member on `rosterUpdate.op = add` (folded onto rosterUpdate as optional field). Key rotation on `remove` is v0.4.
5. Relay (deployed as owner's single-tenant daemon) signs `witness: {owner, handle, seq, env_hash, ts, sig}` per enqueue; subscribers verify against owner's pubkey.
6. Daemon `sync_now` grows sibling channel-pull loop: pull → verify witness → decrypt → dispatch by `$type` (envelope → `envelopes/`, fileUpdate → path, rosterUpdate/channelDef → cached state). Incoming `.claude/skills/*.md` lands in `.claude/skills.incoming/` quarantine. CLI `sec subscribe` + thin MCP wrapper.

Pays off because: (a) it ships the v0.3 channel primitive end-to-end and unblocks Themia `dev:*` + Rafa↔Marcelo channels; (b) the channel-dir is one sync unit, not two — invariant #8 is honored, no substrate fragmentation; (c) every existing layer (lexicons, daemon cursor infrastructure, queue persistence, auth, handle grammar) gets used as-is; (d) the relay's DM substrate is unchanged — additive only; (e) by *not* adopting Keyhive/RIBLT/Sedimentree/Automerge, we preserve the boring-substrate property; (f) by *not* attempting key rotation on remove, we ship a working v0.8 with an honest, documented limitation that real channel traffic can teach us how to fix.

### Stolen ideas (what Beelay/Keyhive gave us)

* **Three-stage sync framing** — adopted in spirit, simplified: roster → cursor → content. Beelay's membership-graph/state-summary/content maps onto roster-derived-from-stream / per-channel-cursor / envelopes+fileUpdates+blobs.

* **Defense-in-depth: only sync with replicas that hold valid roster membership** — adopted (roster gate on `since_channel`).

* **Per-message Ed25519 signing** — already have; now extends to owner sequence-witness.

* **Encrypt to roster, sync ciphertext** — adopted (relay sees ciphertext; defense-in-depth at the relay layer matches Beelay's "server cannot decrypt").

* **Capability-based access (Keyhive)** — *not* adopted. Flat role enum is enough.

* **Audience-bound envelope auth** — *deferred separate hardening pass*.

* **CRDT for documents (Automerge)** — *not* adopted. Wrong shape.

* **RIBLT set reconciliation** — *not* adopted. Linear cursor pull suffices.

* **Sedimentree storage** — *not* adopted. Day-shard markdown is fine until \~10K envelopes/channel.

### No-gos

* No push fanout (v0.4).

* No cross-channel ordering.

* No Keyhive capability delegation graph.

* No CRDT or Automerge anywhere.

* No `subscribe` RPC at the relay — subscription is daemon-local state.

* No channel ownership transfer (lexicon reserves it; defer).

* No new lexicon record types beyond `fileUpdate` — `witness` and `wrapped_channel_key` are fields on existing records.

* No retroactive change to DM relay semantics — second axis is purely additive.

* No `rsync`-style file-level diff. Append-only log; cursor pull; content-addressed blob for large files.

* No abandoning the existing relay implementation. The \~2.1K LOC of DM substrate stays.

* No audience-bound envelope auth in this slice (deferred hardening).

* No multi-relay-per-org (defer `relayHint` resolution; assume orgDoc default relay).

* No key rotation on `rosterUpdate.op = remove` — v0.4 wedge with explicit documented limitation.

* No skill auto-activation on receipt — quarantine-then-promote.

* No `*.local.md` on wire ever.

* No merge logic on concurrent path writes — last-writer-wins by `effectiveAt` + TID.

* No web UI for quarantine in v0.8 — manual `mv`.

* No separate code path for envelope encryption vs fileUpdate encryption — one roster-symmetric primitive.

