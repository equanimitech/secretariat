---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:9f6deaa39a89b623bcc6b953fcf6310d689abeb0409b307310076e50d26dc365
  docFilename: 2026-06-03-secretariat-session-schedule-ledger.md
  stampedAt: 2026-06-03T21:48:53.350630Z
  signature: ed25519:kMtezQd66dX1fcThuajHWyuzFoU8wzxAmsLnwHD2NxDudf68D7Qq5rsjr0zPjHTEAxSslzvJmYLeJc0JrOAwAw==
---
# Secretariat as session & schedule ledger

* Secretariat needs to **keep track of** **`/schedule`** — the scheduled remote agents (cron routines): what's queued, when it fires, what it's scoped to, what it produced.

* Ideally, track **all Claude sessions in general** — a running ledger of cognition activity, not just scheduled ones.

* It might **interface with keel (the hooks)** — `SessionStart` / `Stop` / `UserPromptSubmit` / wind-down hooks already fire; they could feed the ledger, and keel state (wind-down active, skip credits) becomes visible to the substrate.

* Why this matters: the *orchestration* half of Secretariat (the daemon's `AgentSupervisor`, deferred `RoutingEngine`) can't be the operational substrate of an autonomous enterprise if it's blind to what's running and what's scheduled. The ledger is the observability complement to [[delegation as a sealable decision]]: delegation seals the *outbound* dispatch; the ledger is the *running record* of dispatches, schedules, and sessions.

* Pairs with the daemon-watches-commits read: a stamp/commit is a ledger event; a `/schedule` firing is a ledger event; a session start/stop is a ledger event. One substrate, one timeline.

* Questions:

  * **Telemetry boundary (invariant #2).** A session tracker brushes against "no telemetry / nothing phones home." Must stay **local-only** — a private ledger under the principal's control, never an upload. Local session-tracking ≠ telemetry, but the line has to be explicit.

  * **Authoritative or read-cache (invariant #5)?** The ledger should almost certainly be a *regenerable read-cache* over the real events (hook logs, commits, schedule entries), never database-as-truth.

  * What's the unit — a session, a dispatch, a `/schedule` entry, a keel event? (Same root question as the delegation idea's "unit of the seal.")

  * **Keel interface mechanism** — do hooks write events Secretariat ingests (file-drop the daemon watches), or does Secretariat read keel's own state files? Who owns the schema?

  * **Privacy** — sessions touch personal journals (never-commit rule). The ledger must record *that a session happened* without leaking its contents into any repo.

  * Does the ledger surface in the editor (Attend-mode "what's running / scheduled") or stay daemon-internal? Risk: a "what's running" panel becomes a notification stream — the no-push anti-pattern. Keep it **pull**.

Don't shape yet.
