# Secretariat agent — Claude subagent supervisor (slice 1)

Pitch — 2026-05-18. Source: free-text follow-up to \[\[project\_secretariat\_agent]] and \[\[project\_daemon\_v03\_subsystems]] (2026-05-18 session)

## Boundaries

### Job to be done

As the principal, I want my Secretariat agent to run on a schedule inside the existing daemon — reading my subscribed channels, drafting a digest, signing it with the agent's DID, and dropping it in my local queue — so that ambient correspondence surfaces without me invoking anything, and the digest is waiting when I open my next review session.

*When:* every morning (or whatever cadence the principal sets) the daemon ticks, agent runs, digest envelope appears at `_self/.../inbox/`, signed-only (not stamped — per \[\[project\_stamp\_is\_selective\_weight]]). Baseline today: nothing. Principal either does a manual `sec review` walk or skips entirely.

### Appetite

`medium` — couple of days. Override with `--appetite=<size>`.

The work shrinks because we don't write the agent loop. Claude Agent SDK is the agent loop. We orchestrate: spawn, point at the channel-dir, hand it a brief, capture stdout as the digest body, sign + write.

## Elements

Four primary elements:

* **Agent identity bootstrap.** `sec init` provisions a second `did:key` for the agent alongside the principal's, persisting key under `~/.secretariat/_agent/` (sibling to `_self/`). New `Signer` instance bound to the agent key — same trait as principal's, biometric gate omitted (agents sign autonomously, principals stamp interactively).

* **`AgentSupervisor`** **subsystem in the daemon.** Lives next to `outbox_watcher.rs` and `ipc/` under `crates/daemon/src/`. Spawns Claude subagent via `CognitionLauncher` (already at `ports/mod.rs:170`), waits for completion, captures stdout, fails-soft on crash. Tracing + restart counter; no fancy supervision tree.

* **`ScheduleTicker`** **subsystem.** Cron-style trigger reading cadence from `preferences.toml` (extend existing `delivery.poll_interval_minutes` shape with `agent.digest_cron` or similar). Fires `AgentSupervisor` on tick. Reuses the daemon's existing tokio runtime — no separate scheduler crate.

* **Digest task spec.** Markdown brief handed to the subagent: "you are the Secretariat agent for `<principal-did>`; read `<channel-dirs>`; produce a digest in `<format>`; write to `_self/inbox/<date>-digest.md`." Subagent's cwd is the principal's `~/.secretariat/` root, so the channel tree is naturally walkable. Output captured by supervisor and written *as an agent-signed envelope*, not as raw markdown.

Three subsystems plus the identity. Fits inside the existing daemon process — no new binary, no new install ceremony.

## Risks

### 🐇 Rabbit holes

* **Agent key custody.** Agent key sits at-rest in the principal's `~/.secretariat/_agent/`. Whoever controls the principal's filesystem controls the agent's signature. Acceptable for v1 (single-device, principal owns the machine); flag if multi-device sync lands.

* **Claude subagent invocation contract.** What command, what flags, what env, what stdout format? Verify Agent SDK gives us a non-interactive headless invocation path with deterministic exit codes. If it doesn't, fall back to Claude Code's `claude -p <prompt>` headless mode and document.

* **Cost / rate-limit drift.** Daily digest = N tokens/day per principal. Acceptable for Rafa/Marcelo/Christophe at v1 cardinality. Worth a circuit-breaker (`agent.daily_budget_usd`?) before this ships to more principals, but cut for slice 1.

* **What "subscribed channels" means before subscriptions exist.** Today the principal's filesystem holds whatever channels they've manually `cd`'d into. Slice 1 reads the whole `~/.secretariat/` tree minus `_self/` and treats every channel-dir as in-scope. Per-channel opt-in is a contract-file concern, not a v1 problem.

### 🏴 Off-sides called

* **Reactive (event-driven) agent.** Schedule-only for v1. No "envelope arrives → agent reacts." Add when a concrete use case demands it.

* **Multi-agent / agent roster.** One named agent per principal. No "send agent X this task, agent Y that one." Speculative until we have two named agents.

* **`RoutingEngine`.** Explicit v0.4 wedge in AGENTS.md. Out.

* **Windows AgentSupervisor.** Waits for Windows port slice 2 (Windows Service install). Mac-only for v1.

* **Auto-stamp of agent digests.** Never. Stamp is principal-only per Hard Rule #4. Digest is signed-only — principal stamps if they want to commit to the digest as authoritative; otherwise it stays informational.

### 🥩 Fat cut

* **Custom prompt-templating system.** Hard-code the digest brief in a single Rust string for v1. Externalize when a second prompt exists.

* **Multi-cognition (local LLM, BYOK).** `CognitionLauncher` is already pluggable. Slice 1 wires Claude Agent SDK only.

* **Persistent agent memory across runs.** Each digest is fresh-context. The substrate IS the memory — agent reads filesystem each run.

* **Digest format polish.** Plain markdown, no fancy formatting. AG template comes later if the principal asks.

### 🧪 Domain knowledge

* **Claude Agent SDK invocation.** Confirm headless flag, stdout streaming, exit code semantics, max-turn config. Cite Anthropic docs in the implementation PR. (Use `context7` for current SDK reference.)

* **Cron-on-tokio.** Pick one of `tokio-cron-scheduler` / `chrono` + interval loop / a hand-rolled `tokio::time::sleep_until`. Lean toward hand-rolled — one job, no need for a scheduler crate.

## Pitch

### Problem

The Secretariat agent is named in design (\[\[project\_secretariat\_agent]]) and listed in the daemon subsystem inventory (\[\[project\_daemon\_v03\_subsystems]]), but doesn't exist as code. Without it, every primitive in the v0.3 substrate — channels, contracts, signed envelopes — is principal-driven. The point of an autonomous-enterprise substrate is that the substrate *does work* when the principal isn't there. Today it just stores work the principal did.

The daemon already runs as a LaunchAgent on macOS. The cognition port is already pluggable. The channel-dir is already a Claude Code activation surface. All the prerequisites are in place. What's missing is the small piece that connects them: a supervised, scheduled, signing agent loop. And Claude's Agent SDK means we don't even write the loop — we orchestrate its lifecycle.

### The bet

Two days of focused work to ship a working daily digest. Three deliverables:

1. `sec init` provisions an agent `did:key`; subsequent runs reuse it. Key file is loadable and signs envelopes via the existing `Signer` port.
2. Daemon gains `AgentSupervisor` + `ScheduleTicker` subsystems. On tick, supervisor spawns Claude subagent in headless mode rooted at `~/.secretariat/`, waits, captures stdout, writes the digest as an agent-signed envelope into `_self/inbox/`. Crash recovery: log, retry next tick.
3. End-to-end smoke: cadence configured to fire once, watch the daemon tick, observe digest envelope land in `_self/inbox/<date>-digest.md`, `sec verify` returns `{signature: ok, stamp: none}`.

If overrun, the circuit-breaker: stop, ship whatever subset compiles, re-pitch the residue.

### No-gos

* No reactive (envelope-arrival-triggered) agent.

* No multi-agent / agent roster.

* No `RoutingEngine`.

* No Windows AgentSupervisor (waits for Windows slice 2).

* No auto-stamp of agent output. Ever.

* No prompt-templating system, persistent agent memory, or digest format polish.

### Composes with

* **Windows port slice 1** ([this session's other pitch](./2026-05-18-windows-foundation-headless-port.md)) — independent. Agent supervisor stays Mac-only until Windows slice 2 ships daemon-as-service.

* **`/shaping`** **skill integration** — once the agent identity exists, shaping can sign pitch envelopes via the same primitive. Skill patch is trivial after this slice lands.

