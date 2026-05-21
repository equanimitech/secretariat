# Event-sourced envelope substrate — generalize recipient, prove with `/idea`

Pitch — 2026-05-05. Source: `docs/ideas/event-sourced-envelope-substrate.md`

> **2026-05-05 implementation note (post-collapse):** the substrate
> shipped _flatter_ than this pitch describes. `Recipient` is a flat
> struct `{ owner: Did, handle: QueueHandle }` — **no enum variants**.
> Direct messages, local captures, and channel/newsletter posts all
> share that shape; `owner == self_did?` is a runtime predicate, not
> a type discriminator. `EnvelopeKind` was dropped entirely — recipient
> already encodes kind, and the queue handle's namespace (e.g.
> `inbox:` vs `channel:`) carries any further refinement. The
> "stamp-forbidden on local-queue" invariant was also dropped — stamps
> are allowed on any envelope, including self-attestations of one's
> own captures. See `memory/project_substrate_simplifications.md` and
> `docs/milestones/2026-05-05-substrate-and-menubar.md` for the final
> shape. The breadboarding and risks below remain accurate as
> _motivations_; the type structure they describe was simplified
> further during slice 1 implementation.

## Boundaries

### Job to be done

As the principal, when I capture an idea (or a pain, or a roundtable item), I
want it to land in the same review surface as my outbox drafts — one substrate,
one walker, one mental model — instead of a parallel pile of `docs/ideas/*.md`
files I read separately, so that "AI agents in pro comms" is a structural
property of the system rather than a slogan bolted onto a peer-mail tool.

Baseline today: stamped peer envelopes go to
`~/.secretariat/outbox/<recipient-did>/`, surfaced by the review walker
(`crates/core/src/application/review_queue.rs:30`). Unstamped self-notes go to
`docs/ideas/`, `docs/pain/`, surfaced nowhere — read by hand, processed by the
`/roundtable` skill walking the filesystem.

### Appetite

`medium`

Appetite picked: `medium` — the _substrate_ reshape touches envelope domain,
one application use case, walker projection, and one CLI/MCP entry point. We
prove with one queue (`inbox:triage`) and one kind (`idea`); we do not migrate
`/pain`, `/roundtable`, or `/share` in this bet, and we do not replace the
filesystem layout. Override with `--appetite=<size>`.

## Elements

Breadboard, four primary elements:

- **Place: `Envelope.to` becomes `Recipient`** — replace
  `to: Option<Did>` (`crates/core/src/domain/envelope.rs:67`) with an enum
  `Recipient::Peer(Did) | Recipient::LocalQueue(QueueHandle)`. New value
  object `QueueHandle` (newtype around a parsed string, e.g.
  `inbox:triage`) under `crates/core/src/domain/`.
- **Place: `kind` tag on the envelope** — add `kind: EnvelopeKind` field
  with variants `Letter | Idea` for v1 (`Pain | Note | Task` reserved
  for follow-on pitches but unused). Default `Letter` keeps existing
  peer-mail behavior unchanged.
- **Affordance: stamp-required ↔ recipient kind invariant** — domain
  constructor enforces: `Recipient::Peer` → stamp eventually required;
  `Recipient::LocalQueue` → stamp forbidden. Sealed at the type level,
  not at the call site.
- **Connection: walker projection reads both outbox + local queues** —
  extend `list_outbox_queue` (or sibling `list_review_queue`) to union
  `outbox/<peer-did>/` _and_ `queues/<queue-handle>/` under
  `~/.secretariat/`. Walker UI groups by recipient kind. One CLI/MCP
  entry point (`sec capture --kind=idea --queue=inbox:triage <body>`)
  proves the loop end-to-end.

## Risks

### 🐇 Rabbit holes

- **Wire-format compatibility.** `Envelope.to` is serialized today as
  `Option<Did>` over the relay (`crates/core/src/infrastructure/transport/relay.rs:328`).
  Local queues never travel transport, so the wire format only needs to
  represent `Peer` — but the serde derivation must not break existing
  inbound envelopes from v0.2.x peers. Decision: serialize `Recipient::Peer`
  identically to today's `Some(Did)`; `LocalQueue` variants are local-only,
  rejected at transport boundary.
- **Queue handle parsing.** `inbox:triage` vs `area:writing` vs
  `project:secretariat` — schema choice baked in at v1 will be load-bearing.
  Mitigation: ship one fixed queue (`inbox:triage`) for the bet; defer the
  taxonomy to a follow-on pitch (the idea file already lists this as an
  open question). `QueueHandle` parser accepts `^[a-z]+:[a-z0-9-]+$`,
  rejects everything else; only `inbox:*` is a recognized prefix in v1.
- **Filesystem layout for local queues.** Mirroring `outbox/<did>/`
  shape gives `queues/<handle>/<envelope-id>.md`. Same markdown
  serialization, same frontmatter. No new file format. Risk: queue
  handles with `:` in directory names — replace with `/` so
  `inbox:triage` → `queues/inbox/triage/`.

### 🏴 Off-sides called

- **Migrating `/pain` and `/roundtable` in this bet.** Out. Substrate
  must be proven with one kind first; pain and roundtable get their own
  pitches once the substrate is real.
- **Replacing the filesystem with an append-only event log.** Out. The
  idea file frames the _model_ as event-sourced; the _implementation_
  stays markdown-files-in-directories for v1. Append-only log is a
  follow-on infrastructure pitch only if/when projection cost
  demands it.
- **Agent-to-agent traffic, cognition adapters writing to queues.** Out.
  This pitch only adds H↔A (principal capturing into a local queue).
  A↔A needs authorization model (idea file open question) and is its
  own pitch.
- **Cross-principal queue addressing** (e.g.
  `did:web:marcelo.example/area:book`). Out. Queues are strictly local
  in v1.

### 🥩 Fat cut

- **Full `EnvelopeKind` enum (`Pain | Note | Task` etc).** Ship `Letter`
  - `Idea` only; the others are reserved variants without code paths.
    Adding them later is additive.
- **Walker UI redesign.** Current two-button home + walker (v0.2.3,
  commit `21cc416`) handles the new envelopes with one extra group
  header. No screen rework.
- **`sec capture` as a fully-featured composer.** Single-shot CLI takes
  body from stdin or `-m`; no editor invocation, no template, no
  attention-envelope routing. Just: write envelope to local queue file.

### 🧪 Domain knowledge

- **The "no real DIDs in tests" rule** (memory:
  `feedback_no_real_dids_in_tests.md`) — `Recipient::Peer` test data
  must use `Did::from_ed25519_public_key(&[seed; 32])`. Verified.
- **The "show drafts before signing" rule** (memory:
  `feedback_show_drafts_before_signing.md`) — applies to stamped
  envelopes only; local-queue captures are unstamped, so the inline
  show-body-first contract is not triggered. Worth a CLAUDE.md
  amendment in a follow-on doc-only commit, not in this bet.
- **Lexicon impact.** `tech.equanimi.secretariat.envelope` schema in
  `lexicons/` would gain a `kind` field. Schema is mutable until
  self-use validates (AGENTS.md "out of scope" note). Update lexicon
  alongside the domain change; no consumer is reading lexicons at
  runtime today.

## Pitch

### Problem

Secretariat reads as a stamped peer-mail tool with a slash-command
sidecar (`/idea`, `/pain`, `/roundtable`) writing files to `docs/`.
The vision — _"async generative communication for professionals,
stamped by humans"_ (memory: `project_vision_tagline.md`) — requires
that AI agents participate in correspondence as first-class traffic,
not as bolt-on tooling. Today they don't: ideas, pains, agent bids
all live in a parallel filesystem world the review walker can't see.
The principal has two queues to check, two mental models to keep,
and the "agents in pro comms" promise is rhetorical instead of
structural.

The fix is a one-line reframe that has been hiding in plain sight:
the envelope is the substrate, and the stamp is a property of
envelopes whose recipient crosses the H↔H trust boundary. Local-queue
envelopes don't get stamped because there's no boundary to cross.
Same primitive, three traffic kinds (H↔H stamped, H↔A unstamped, A↔A
unstamped), one walker, one log.

### The bet

Generalize `Envelope.to` from `Option<Did>` to a `Recipient` enum that
admits a `LocalQueue(QueueHandle)` variant. Add an `EnvelopeKind` tag
(`Letter` | `Idea` for v1). Walker projection unions outbox + local
queues. Ship one CLI/MCP entry point (`sec capture --kind=idea
--queue=inbox:triage`) and rewrite the `/idea` skill in this repo to
call it instead of writing to `docs/ideas/`. End state: capturing an
idea lands the envelope in the same walker the principal already opens
to review stamped drafts. Substrate proven; subsequent pitches migrate
`/pain`, `/roundtable`, agent processors.

Medium appetite. Domain change is small (one enum, one newtype, one
field), application change is one new use case + one projection
extension, infrastructure is filesystem (mirror existing layout under
`queues/`), CLI/MCP add one verb. Tests and lexicon update included.

### No-gos

- No append-only event log infrastructure — keep markdown-files-in-dirs.
- No queue taxonomy beyond `inbox:triage` — defer to follow-on pitch.
- No migration of `/pain`, `/roundtable`, `/share` — separate pitches.
- No A↔A traffic, no cognition adapters writing to queues — H↔A only.
- No cross-principal queue addressing — local only.
- No walker UI redesign — one extra group header on existing surface.
- No biometric / ceremony changes — local queues are unstamped by
  domain invariant; ceremony untouched.
