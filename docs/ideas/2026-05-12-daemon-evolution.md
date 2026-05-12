# Daemon evolution — v0.2.x plumbing → v0.3+ local nervous system

**Date:** 2026-05-12
**Status:** shaping, no code changes
**Predecessor / companion docs:**
- `docs/ideas/2026-05-12-secretariat-as-autonomous-enterprise-substrate.md` (v0.3 direction)
- `~/.claude/projects/.../memory/project_daemon_v03_subsystems.md` (9-subsystem target shape)
- `docs/milestones/2026-04-30-first-signed-message.md` (Day 1 daemon shape)
- `crates/cli/src/commands/daemon.rs`, `crates/core/src/application/sync.rs`,
  `crates/relay/` (today's surface)

This doc shapes — does not implement — the daemon's evolution from
v0.2.16's poll-and-ferry loop to the v0.3+ local nervous system the
channel-substrate direction requires. Constraints kept verbatim:
owner-as-sequencer per channel; filesystem authoritative; selective
stamp; no central server; `CognitionPort` pluggable; LLM never primary
routing.

---

## 1. Current state (v0.2.16)

**Process shape.** One foreground binary, `sec daemon serve`, supervised
by a macOS LaunchAgent (`tech.equanimi.secretariat.daemon`). Survives
reboots. `KeepAlive = true`. CLI subcommands: `register`, `serve`,
`tick`, `install`, `uninstall`, `status`. No PID file, no health endpoint,
no IPC socket.

**Tick body.** `crates/cli/src/commands/daemon.rs::serve` runs
`decide_poll` (`core::application::delivery_policy`) on a 15-min floor
configurable via `~/.secretariat/cadence.toml`. Each tick calls
`core::application::sync_now`, which does three things, sequentially,
per registered relay:

1. **Poll inbound.** `RelayClient::poll(token, cursor)` over HTTP →
   write each envelope as a flat file in `~/.secretariat/inbox/`,
   advance the cursor.
2. **Drain claim notifications.** `RelayClient::claimed_invites` →
   `process_correspondence_claims` auto-adds the claimant as a contact
   (the defining behavior of `sec invite`).
3. **Drain outbox.** Walk `~/.secretariat/outbox/<recipient-did>/*.md`,
   skip unstamped drafts, deliver stamped ones via the recipient's
   relay (from contact book), move to `<recipient-did>/sent/`.

**Key handling.** Signing key loaded into the daemon process at startup
and held in memory for signing during `register`/`authenticate`/
outbox-drain. Bodies stay ciphertext on disk — daemon never decrypts.
Decryption happens lazily in `sec read <file>`.

**Relay.** `crates/relay/` ships as a separate binary
(`secretariat-relay`), axum-based, six routes (`/healthz`,
`/v0/register`, `/v0/auth/{challenge,answer}`, `/v0/inbox/:did`,
`/v0/invite*`). Per-DID inbox queue. Not embedded in the daemon
process; principals hosting their own relay run it separately.

**Callers of `sync_now`.** Three: `sec daemon serve` (loop),
`sec daemon tick` (one-shot), Tauri `sync_now` command (in-proc).
Each runs its own process — no shared state, no socket.

**Inbox path scheme.** Flat: `~/.secretariat/inbox/<iso>-<sender>-id<n>.md`.
No per-channel directory, no time-sharding, no `_ciphertext/` +
`envelopes/` split, no plaintext for AI/grep.

**Outbox path scheme.** `~/.secretariat/outbox/<recipient-did>/*.md`
(per-recipient-DID flat). No per-channel `<channel-dir>/outbox/`.

---

## 2. Target state (per [[project_daemon_v03_subsystems]])

Nine subsystems, organized around channel-dir layout (per the substrate
report, §"channel directory IS the activation surface"):

| # | Subsystem | Owns | Maps to today |
|---|---|---|---|
| 1 | RelayServer | Per-channel canonical sequence for channels this principal owns; HTTP + WebSocket/SSE | `crates/relay/` (separate binary, per-DID only) |
| 2 | RelayClient | Push subscription to owner relays for channels this principal reads; cadenced poll fallback for humans | `core::infrastructure::transport::relay::RelayClient` (poll-only) |
| 3 | OutboxWatcher | Watches `<channel-dir>/outbox/`; signs draft; encrypts to recipients; hands to transport | `sync::drain_outbox` (tick-only, per-DID flat scheme) |
| 4 | InboxWriter | Decrypts inbound → writes `_ciphertext/<hash>.env` (canonical) + `envelopes/YYYY/MM/DD/<iso>-<hash>.md` (plaintext) | `sync::file_inbound` (ciphertext-only, flat scheme) |
| 5 | MetaResolver | Pulls `<channel>:_meta`; writes resolved `CLAUDE.md` + `.claude/{agents,skills,commands}/` to channel-dir; respects `*.local.md` | — (none) |
| 6 | AgentSupervisor | Spawns per-channel always-on agents via Claude Agent SDK; cwd = channel-dir; triggers on cron + FS-notify on `envelopes/`; respects consumption contracts | — (none) |
| 7 | RoutingEngine | Per-envelope: consumption contract × attention-envelope × declared depth/urgency → promote/digest/mute/bounce | — (none; v0.4 wedge per [[project_attention_routing_future]]) |
| 8 | ScheduleTicker | Cron-like duty registry — relay poll, outbox drain, daily digest, meta-resolve, subscription keepalive | `decide_poll` + serve-loop sleep (single duty) |
| 9 | IPC | Unix socket; CLI / Tauri tray / MCP all talk to running daemon | — (Tauri calls in-proc; CLI tick spawns separate process) |

Constraint reminders this shape respects:

- **Owner-as-sequencer.** RelayServer is the *only* sequencer for channels
  this principal owns; subscribers' RelayClient reads that sequence. No
  consensus, no global ordering.
- **Filesystem authoritative.** InboxWriter + MetaResolver write to disk
  *first*; any in-memory index is a derivable cache.
- **Selective stamp.** OutboxWatcher signs every draft but never auto-stamps;
  drafts marked `elevate=true` queue for principal review instead of
  direct send.
- **No central server.** RelayServer is per-principal-local; RelayClient
  talks directly to each peer's owner-relay; no broker.
- **Pluggable cognition.** AgentSupervisor and (future) RoutingEngine
  call `CognitionPort`; SmolLM2 / Ollama acceptable for *enrichment*
  (subject, topic, sanity), never for the *decision* (route, surface,
  stamp).

---

## 3. Gap (what the v0.2.16 daemon doesn't do yet)

Listed roughly by "blocking for which slice."

1. **No socket / IPC.** Tauri spawns in-proc; CLI spawns sibling process.
   Multiple sync_now invocations can race against `RelayState` save.
2. **Single hardcoded duty.** Serve loop = one `decide_poll` + one
   `sync_now`. Adding a "morning digest at 07:00" or "meta-resolve every
   30s" requires forking the loop. No registry, no per-duty cadence.
3. **No FS-notify.** Outbox latency = floor(cadence) (default 15 min).
   Stamp → send window is principal-visible. Tauri's "Sync now"
   compensates but only when the app is open.
4. **No push subscription.** RelayClient is poll-only. Agents that want
   sub-second freshness can't get it; humans poll on a 15-min floor
   (correct), but agents on the same code path is wrong by §"two
   consumption modes" of the substrate report.
5. **Per-DID, not per-channel.** Inbox queue keyed by recipient DID;
   no `(owner_did, handle)` tuple. Relay routes `/v0/inbox/:did` not
   `/v0/channel/:owner/:handle`. Cursor tracked per-relay, not
   per-channel.
6. **Flat inbox / outbox paths.** No time-sharding, no `_ciphertext/`
   vs `envelopes/` split, no per-channel directory. Plaintext never
   written → AI + grep can't see history (architectural moat closed).
7. **Daemon doesn't decrypt.** v0.2.16 was deliberate — keep the key
   cold. v0.3 substrate (plaintext markdown for AI/grep) needs eager
   decrypt. Threat-model decision must be made explicit before this
   ships.
8. **No meta-channel concept.** `<channel>:_meta` doesn't exist yet;
   nothing pulls / applies channel context envelopes.
9. **No agent supervisor.** Always-on per-channel agents (Secretariat
   agent included) need spawn / restart / triggers / cwd context.
   Claude Agent SDK integration is unbuilt.
10. **Relay sequence semantics.** Today's relay is "per-DID inbox";
    v0.3 needs "per-(owner,handle) sequence." Single-sequence with
    cursor + live push is a different shape.
11. **Embedded relay vs sibling binary.** v0.2.x deliberately separates
    `secretariat-relay` from `sec daemon`. For principals who own
    channels, two LaunchAgents is friction; embedding the relay in the
    daemon process is the natural shape — but it crosses a process
    boundary that was useful for isolation. Decision needed.
12. **No routing.** Every inbound surfaces (file-into-inbox). No
    contract evaluation, no depth/urgency check, no digest promotion.
    Fine for v0.2 (low volume); breaks under channel traffic.

---

## 4. Recommended ship order

The goal: each slice ships independently, doesn't break v0.2.16
behavior for existing users (Rafa, Marcelo, dad), and unlocks the
next slice. Numbers below ≠ semver versions; group by milestone.

### Phase A — factor & socket (no behavior change)

**Slice 0. Daemon library.** Extract `crates/cli/src/commands/daemon.rs`'s
guts into a new `crates/daemon/` crate. Today's `serve`/`tick` become
thin wrappers over `secretariat_daemon::Daemon::run()`. Subsystems
become modules (`relay_client/`, `outbox_watcher/`, `inbox_writer/`,
`schedule_ticker/`, `ipc/`). v0.2's logic registers as a single
`LegacyPollDuty` under ScheduleTicker. Zero observable change.

**Slice 1. IPC socket.** Add Unix socket at
`~/.secretariat/daemon.sock`. Line-delimited JSON-RPC. First three
methods: `tick`, `status`, `version`. Tauri's `sync_now` IPC command
routes through the socket when present; `sec daemon tick` from the
CLI does the same. Both fall back to in-proc when no daemon running.
Eliminates the race over `RelayState`. v0.2.16 behavior preserved
because the socket is optional.

### Phase B — channel-dir paths (additive, dual-write)

**Slice 2. OutboxWatcher with FS-notify.** Add `notify` crate;
watch `~/.secretariat/outbox/`. New `.md` → debounce 200ms → enqueue
outbox-drain duty. Periodic-tick safety net stays. Latency: 15min →
seconds for stamped → sent. No path-scheme change in this slice;
just trigger source.

**Slice 3. Channel-dir path scheme (dual-write).** Inbox & outbox start
dual-writing:

- Legacy: `~/.secretariat/inbox/<flat>.md` and
  `~/.secretariat/outbox/<recipient-did>/*.md` — kept as-is.
- New: `<channel-dir>/envelopes/YYYY/MM/DD/<iso>-<hash>.md` and
  `<channel-dir>/outbox/*.md`.

New path used when the envelope's `(owner_did, handle)` resolves to a
known channel directory; legacy path otherwise. Resolver is a small
function: `subscription_registry.lookup(owner, handle) -> Option<PathBuf>`.
v0.2 contacts continue working (peer-bilateral = channel-of-two; map
to a synthetic channel-dir at `<peer-alias>/`).

### Phase C — substrate semantics

**Slice 4. InboxWriter — eager decrypt + two-tier.** When the envelope
is channel-routed (Slice 3 path), write ciphertext to
`<channel-dir>/_ciphertext/<hash>.env` *and* decrypt + write plaintext
to `<channel-dir>/envelopes/YYYY/MM/DD/<iso>-<hash>.md`. Requires the
daemon to hold the principal's x25519 decryption key. Threat-model
decision is open (see §"Open questions" #1). Bridges the
filesystem-authoritative + AI-grep-able property — this is the
architectural moat unlocking.

**Slice 5. RelayClient push subscription.** Add WebSocket/SSE subscribe
alongside existing poll. RelayClient mode = `Push` for agent-served
channels, `Poll` for human-cadenced channels. Owner-relay extends its
inbox route to a per-channel sequence endpoint
(`/v0/channel/:owner_did/:handle` GET + WS). Existing `/v0/inbox/:did`
deprecated-but-kept-for-bilateral. Owner-as-sequencer invariant lands
here.

**Slice 6. RelayServer embedded mode.** New flag
`sec daemon serve --with-relay`. When set, daemon boots
`crates/relay/`'s axum router in the same tokio runtime, bound to the
configured port. Saves a LaunchAgent for principals who own channels
(Rafa with `did:web:equanimi.tech`). Sibling-binary `secretariat-relay`
stays for principals who don't want embedded (or who run the relay on
a separate host).

### Phase D — channel context + agents

**Slice 7. MetaResolver.** Subscribe to `<channel>:_meta` envelopes
(uses Slice 5 push when available). Apply latest-wins per slug → write
resolved `CLAUDE.md` + `.claude/{agents,skills,commands}/*.md` into the
channel-dir. Respect `*.local.md` overrides (never overwrite). Roster
mutations (`tech.equanimi.secretariat.rosterUpdate`) flow same channel.
Channel-dir becomes a self-contained Claude Code project.

**Slice 8. ScheduleTicker generalization.** Replace the
single-cadence serve loop with a duty registry. v0.3 duties:
`relay_poll` (existing), `outbox_drain` (FS-triggered + periodic
safety net), `meta_resolve`, `subscription_keepalive` (WS health
checks). Cron-expression backed. Sets up Slice 9.

**Slice 9. AgentSupervisor.** Discover per-channel
`.claude/agents/*.md`. For each agent declaring a trigger (cron from
ScheduleTicker, FS-notify on `envelopes/`, IPC RPC), supervise its
launch via Claude Agent SDK with channel-dir as cwd. Output written
to `<channel-dir>/outbox/` flows through OutboxWatcher → signed →
sent. First customer: the Secretariat agent (named first-class agent,
per `project_secretariat_agent` memory) drafting the morning digest.

### Phase E — v0.4 wedge (deferred)

**Slice 10. RoutingEngine.** Wait for 2-3 weeks of real channel
traffic before designing rules (per [[project_attention_routing_future]]).
Deterministic core (declared fields × contract × bounds → decision);
SmolLM2 enrichment *after* filing (subject inference, topic tagging,
urgency sanity, tone tag for notify policy). Never gate filing on
inference.

**Slice 11. SQLite read-cache.** Cross-channel query latency optimization.
Defer until grep + `rg` over `envelopes/YYYY/MM/DD/` actually becomes
slow. Regenerable, never authoritative.

---

## 5. Open questions

1. **Decryption key in daemon (Slice 4 blocker).** v0.2.x deliberately
   kept the key cold to limit blast radius if the daemon process is
   compromised. v0.3 substrate (markdown for AI/grep) needs eager
   decrypt → daemon must hold the x25519 decryption key derived from
   the ed25519 identity. Options:
   - (a) Daemon holds the key in memory (simplest; matches the
     sovereignty rule's "the device is the threat boundary, not the
     process boundary"). Use `zeroize` + memory-locked region. Document
     the change in AGENTS.md threat model.
   - (b) Daemon writes ciphertext only; a separate short-lived
     "decryptor" process the principal authorizes per session writes
     plaintext. Higher operational complexity; doesn't actually
     improve the threat model meaningfully (same disk, same
     attacker).
   - (c) Decrypt lazily on AI/grep request via IPC RPC. Defers the
     decision but blocks Claude Code sessions from reading history
     without a running daemon. Wrong tradeoff.

   **Lean:** (a) with explicit AGENTS.md amendment.

2. **Daemon crate location.** Three options:
   - New `crates/daemon/` crate, separate from `crates/cli` and
     `crates/core`. Hosts its own deps (`notify`, `tokio-tungstenite`,
     Claude Agent SDK bindings). Cleanest. **Recommended.**
   - Library module under `crates/core/`. Inflates core; violates the
     "domain has no IO" rule indirectly because `notify` and WS are IO.
   - Stay in `crates/cli/`. Couples to the CLI binary; can't be a
     LaunchAgent target without the full CLI.

3. **IPC protocol.** JSON-RPC over Unix socket is enough for v0.3.
   Capnp / Cap'n Proto only if push-volume across IPC matters (it
   probably doesn't — push lives daemon-internal, IPC is for control
   plane).

4. **Embedded RelayServer vs sibling binary (Slice 6).** Two install
   profiles or one? Sibling binary is operationally cleaner (independent
   restart, independent log, independent crash). Embedded saves a
   LaunchAgent. Decision can defer to Slice 6; both shapes already
   factored into `crates/relay/`'s library + main split.

5. **AgentSupervisor concurrency budget.** N channels × always-on
   Claude Agent SDK loops = N concurrent API connections + token cost.
   Default: launch-on-trigger (cron / FS-notify), not persistent. A
   persistent-loop agent declares `persistent = true` in its frontmatter
   and is supervised differently. Resource cap configurable.

6. **Channel resolution at file-inbound time (Slice 3 blocker).**
   InboxWriter needs `(owner_did, handle)` → channel-dir. Source:
   substrate report §5 says envelope carries `(owner_did, handle)` as
   wire-level fields. v0.2.x envelope wire format doesn't yet —
   lexicon extension blocks Slice 3. May want to land the lexicon
   shape (no runtime validation, per the substrate report's "next
   steps" #3) before Phase B.

7. **Legacy path migration.** Dual-write through v0.3, one-shot
   `sec migrate` when v0.4 lands. Alternative: read-old / write-new
   for inbox (new arrivals only land in channel-dirs) and let
   `~/.secretariat/inbox/` go stale. Cleaner; loses history.
   **Lean:** dual-write through v0.3, migrate at v0.4.

8. **LaunchAgent KeepAlive vs embedded RelayServer.** If port-bind
   fails (port already in use), `KeepAlive = true` makes launchctl
   respawn-loop. Need explicit unhealthy-exit semantics: distinguish
   "daemon failed, retry" from "port conflict, stop." Probably an
   exit-code convention + a launchd `ExitTimeOut`. Slice 6 problem.

9. **Per-duty cadence config.** Today's `cadence.toml` has one knob
   (`poll_interval_minutes`). Slice 8 needs per-duty entries. Schema
   migration: extend the same TOML file with named tables (`[relay_poll]`,
   `[meta_resolve]`, etc.); the existing top-level `poll_interval_minutes`
   becomes `[relay_poll].interval_minutes` with a back-compat shim.

10. **MCP exposure of daemon control.** Should MCP expose
    `daemon_tick`, `daemon_status` already? Today only the CLI does
    (`sec daemon tick`/`status`). Adding to MCP is cheap once Slice 1
    socket exists; aligns with the AGENTS.md rule "every principal-
    facing primitive ships on both interfaces." Daemon-only operations
    are exempt — these are *control* operations, principal-facing, so
    they qualify. **Lean:** yes, ship them on MCP in Slice 1.

---

## 6. Non-goals (worth saying out loud)

- Cross-channel global ordering (substrate invariant).
- Consensus protocols / Byzantine fault tolerance.
- Centralized routing service.
- LLM as primary routing decision-maker.
- Synchronous LLM gating on envelope arrival.
- Daemon owning Claude Agent SDK conversation state (daemon supervises;
  SDK owns the conversation).
- Read receipts / delivery state surfaced to senders.
- Real-time push for human consumption (15-min floor stays).
