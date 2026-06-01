# Event-sourced envelope substrate — H↔H, H↔A, A↔A on one log

Raw capture — 2026-05-05.

## The reframe

Secretariat today reads as "stamped peer mail tool." Substrate underneath is
file-system-shaped (outbox dir per recipient DID, inbox dir, drafts as files).
Stamp ceremony rides on top.

Vision is _"async generative communication for professionals, stamped by
humans"_ — i.e. AI agents in pro comms without losing H2H trust. That requires
**one substrate** carrying three traffic kinds, not a stamped-mail tool with
agent features bolted alongside.

## Three traffic kinds, one envelope primitive

| Kind  | Stamped? | Why                                 |
| ----- | -------- | ----------------------------------- |
| H ↔ H | yes      | trust boundary crosses principals   |
| H ↔ A | no       | inside principal's sovereignty zone |
| A ↔ A | no       | agents acting on principal's behalf |

If H↔A and A↔A live in a different system (Things, `docs/`, slash commands
writing files), agents are bolted on — vision unmet. Same substrate is what
makes "agents participating in correspondence" structurally true instead of
just rhetorical.

## Event-sourcing shape

- **Envelope = event** (immutable, addressed, typed, append-only)
- **Stamp = signature** applied only at H↔H trust boundary
- **Queue = stream** (per area / project / peer DID)
- **Inbox / outbox / review-walker = projections**
- **Replay possible** because log is append-only

Recipient is either:

- a **peer DID** → stamp required (H↔H)
- a **local queue handle** (`area:writing`, `project:secretariat`,
  `inbox:triage`) → no stamp (H↔A, A↔A)

Envelope carries lightweight `kind` tag (`idea` / `pain` / `note` / `task` /
`letter`). Processors subscribe to queues, filter by tag if they care.

## What this absorbs

- `/idea` → unstamped envelope, kind=idea, queue=area or project
- `/pain` → unstamped envelope, kind=pain, queue=area or project
- `/roundtable` → processor that reads across queues filtered by
  `kind in {idea, pain}`
- `/share` → unstamped envelope, kind=note (eventually — see existing memory
  on three-stage convergence)
- Today's `~/.secretariat/outbox/<recipient-did>/` → just one queue type
  (peer DIDs); local queues are siblings

## Equanimitech alignment

- **Sovereignty** — principal owns the log, the queue taxonomy, the
  agent-to-queue assignments
- **Awareness** — walker = projection over log filtered by attention bounds
- **Equanimity** — cadenced reviews, no notifications, principal-initiated
  sync (already established in v0.2)

## Questions

- Queue taxonomy — fixed names (`area:*`, `project:*`, `inbox:*`) vs free
  string? Things 3 area/project model maps cleanly; should mirror it?
- Where does the log live? Per-queue files vs single append log + index?
  (Affects backup, sync, replay cost)
- How do agents register as subscribers to a queue? Local config? MCP
  capability negotiation?
- Stamped envelopes addressed to a _peer's queue_ (not just their root DID) —
  e.g. `did:web:marcelo.example/area:book` — does that make sense, or is
  queue-routing strictly local?
- Cognition agents writing to A↔A queues (one agent prompting another):
  who authorizes? Implicit on behalf of principal, or explicit grant per
  agent-pair?
- Migration: today's outbox-as-directory works. Do we generalize the
  in-memory model first (queue trait) and keep filesystem layout, or move
  to event log immediately?
- Does this collapse `/share` convergence into a one-step move (everything
  becomes an envelope) instead of the three-stage path memory currently
  records?
- Naming: "queue" is engineer-speak. Principal-facing word? "channel"
  (overloaded), "stream" (overloaded), "thread" (overloaded), "folder"
  (Things-flavored), "area" (Things-flavored), "lane"?

Don't shape yet.
