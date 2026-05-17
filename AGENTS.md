# Secretariat — Claude orchestration

Secretariat is a cryptographically attested AI-mediated correspondence system.
The principal is the human; Claude is the scribe. Claude composes, the principal
stamps. Stamps are biometrically gated.

This file is read at the start of every Claude Code session. It overrides the
generic template defaults (the project was bootstrapped from
`dannysmith/tauri-template` but the bounded context is Secretariat, not a generic
desktop app).

## What's here today

- **CLI (`sec`)** — works end-to-end on macOS. `init` / `compose` / `stamp` /
  `verify` / `list` / `daemon install` / `mcp install`. See
  `docs/developer/secretariat-architecture.md`.
- **MCP server (`sec-mcp`)** — built. Exposes `compose`, `stamp`, `read`,
  `verify`, `list_outbox`, `list_inbox`, `list_contacts`, `add_contact`,
  `invite_create`, `invite_claim`, `init`, `daemon_install`,
  `daemon_status`. Source at `crates/mcp/`.
- **Tauri shell** — running. Bundles `sec` + `sec-mcp` as sidecars and on
  launch wires them silently (`sec mcp install`, `sec daemon install`).
  Per `project_mcp_is_primary_interface`, the app is tray + quick-pane +
  daemon + MCP wiring; principal-facing review/compose happens via Claude
  (MCP) or CLI.

- **`sec launch`** — opens Claude Code (or any configured cognition CLI)
  with `cwd` set to a channel's bound directory. The channel's
  `contract.local.md` carries an optional `root_path: <abs-path>`
  override; when set, the channel-dir resolves to that host path
  (typically a git repo). Cognition substrate is config-driven via
  `[cognition] launch_command / launch_args / launch_env` in
  `preferences.toml` — LM Studio integration is a config block, no
  fork. See `docs/developer/launch.md`. The headless `dispatch`
  counterpart and the `sec bind` writer ship in a separate slice
  (`docs/pitches/2026-05-13-launch-dispatch-root-path.md`).

  Sidecars are staged into `src-tauri/binaries/` by
  `src-tauri/scripts/build-sidecars.sh`, which Tauri's `beforeBuildCommand`
  runs automatically during `pnpm tauri:dev` / `pnpm tauri:build`. For a
  bare `cargo check -p secretariat` on a clean clone you must run that
  script once first (otherwise `tauri-build` fails with *"resource path
  `binaries/sec-<triple>` doesn't exist"*).

## Hard rules

These are non-negotiable. They override the template's defaults where they conflict.

1. **Use pnpm, not npm.** This overrides the template's npm-only rule. All
   front-end commands run as `pnpm install` / `pnpm tauri:dev` etc.

2. **Domain layer (`crates/core/src/domain/`) imports no IO.** No `std::fs`,
   no `reqwest`, no `chrono::Utc::now()`. Time and randomness enter via
   parameters. Aggregates enforce invariants at construction.

3. **AT-proto-lexicon-shaped records.** Every record type has a `$type`
   discriminator (e.g. `tech.equanimi.secretariat.stamp`). Schemas are mirrored
   under `lexicons/` — that directory is the source of truth for the on-wire
   shape, even though it does not yet drive runtime validation.

4. **Three-layer trust model: signature mandatory, stamp selective,
   counter-stamp multi-party.** Updated 2026-05-12 for the v0.3 substrate
   shift (see `docs/ideas/2026-05-12-secretariat-as-autonomous-enterprise-substrate.md`).

   - **Signature** — every envelope carries a detached DID-keyed signature
     from its author (human principal or agent DID). Mandatory. Drives
     `sec verify` provenance: *"did this come from the claimed author?"*
   - **Stamp** — principal Touch-ID attestation. **Selective, not
     mandatory.** Applied to envelopes the principal elects to elevate
     (decisions, commitments, process-verbaux, external comms, contracts).
     Most envelopes — agent-drafted ambient traffic in particular —
     flow signed-only. The stamped subset *is* the org's authoritative
     decision ledger.
   - **Counter-stamp** — multi-principal stamp on the same envelope
     (m.3 process-verbaux model). Reserved for v0.4+; the design space is
     defined but no record type ships in v0.3.

   The earlier model ("every sent envelope is stamped") is superseded —
   it didn't survive contact with AI-volume traffic, and resolving the
   tension by reducing per-stamp ceremony friction (batch-stamp Merkle
   roots) was the wrong move; the right move was to make stamping the
   *curation act* it always wanted to be.

   **Stamp ceremony is principal-attested, not Claude-attested.** *When
   stamping happens*, Claude *may* initiate `stamp` (via the MCP tool
   or `sec stamp`) but MUST first show the principal the full decrypted
   body verbatim — code block or quoted region, never a summary — and
   obtain explicit confirmation in the same turn. Implicit consent from a
   prior turn does not count if the file changed.

   The biometric gate (Touch ID) blocks until the principal physically
   authorizes; the dialog's reason string carries the document's first-line
   headline + a short hash prefix so the principal can cross-check what
   they're signing against what Claude displayed. If those differ, abort.

   Tradeoff recorded explicitly: an earlier draft of this rule forbade
   Claude from initiating stamp at all (terminal-only). That eroded the
   workflow without meaningfully changing the threat model — Touch ID
   already gates regardless of caller, and the principal's responsibility
   is to read what they're stamping, not to type the command. Phishing
   risk is mitigated by the show-body-first contract + headline-in-dialog,
   not by who-types-the-command.

   **Receiver-side discipline:** `sec verify --json` returns layered
   results — `{signature: ok|invalid, stamp: none|ok|invalid,
   counter_stamps: [...]}`. Recipient policy decides what they require.
   An unstamped-but-signed envelope is *informational* (the author wrote
   this); a stamped envelope is *authoritative* (the principal vouches
   for it). UI MUST surface this distinction; agents acting on received
   envelopes MUST treat signed-only ≠ stamped.

5. **Compose envelopes following the principal's template.** Global default
   at `~/.secretariat/template.md`; channel-scoped override (when present)
   at `<channel-dir>/template.md`. Both are user-customizable AG
   (attentional-granularity) templates, owned by the principal — they are
   to envelope composition what `CLAUDE.md` is to general Claude behavior.
   Channel-level override wins for envelopes addressed to that channel.

6. **Respect attention envelopes and per-channel consumption contracts.**
   `~/.secretariat/attention-envelope.md` declares the principal's global
   bounds (depths, urgencies, cadence). Per-channel
   `<channel-dir>/contract.local.md` files declare per-channel overrides
   (cadence, depth, notify, filter); a parallel org-root file at
   `<org-dir>/contract.local.md` carries org-wide overrides that
   accumulate down the channel tree. Per-channel overrides win for
   traffic in that channel. Queue to a local outbox rather than surfacing
   inline if the bid would violate cadence. The protocol detects bound
   violations; Claude pre-empts them.

   The `.local` suffix is load-bearing: these files are **private to
   the subscriber** — receiver-side preferences/filters, never sent on
   wire, never shared with the roster, ignored by any future `git`
   backup of `~/.secretariat/`. The bare `contract.md` filename
   (without `.local`) is reserved for the future **channel governance**
   artifact — roster, channel-wide artifact policy ("this channel only
   carries stamped envelopes"), shared with all roster members,
   eventually a signed envelope. Don't conflate the two; the file
   extension carries the visibility contract.

7. **Place drafts in the queue's local `outbox/` directory.** For channel
   posts: `<channel-dir>/outbox/`. For direct messages (queue owned by a
   peer): `<peer-alias-dir>/<handle-path>/outbox/` — the sender's local
   mirror of the recipient's queue. For local captures (queue owned by
   self): `<self-dir>/<handle-path>/outbox/`. Never write draft envelopes
   into the working directory or a flat top-level outbox keyed by
   recipient DID. The per-channel outbox is what the daemon watches and
   what the principal reviews/stamps from.

8. **Verify before trusting incoming envelopes.** `sec verify --json` returns
   layered results — `{signature, stamp, counter_stamps}` — per the
   three-layer trust model in rule #4. Never trust an envelope's claims
   without verifying. An unverified envelope is not actionable; an
   envelope whose signature fails is malformed and must be quarantined,
   not surfaced.

9. **The principal owns the closing line.** Earlier versions of this rule
   required appending `_Drafted by AI, reviewed by a human._` to every
   envelope. The signature line is now configurable — the principal
   decides what (if anything) closes the body. Claude does NOT auto-append.
   If the principal has a configured closing line, use it verbatim; if
   not, end without one. The cryptographic stamp (when applied — see
   rule #4) is what records human disposition; the body closing line is
   editorial, not protocol.

10. **Tauri v2 only.** Modern Rust formatting (`format!("{variable}")`).

## Architectural invariants

These are properties of the *system*, not rules of *behavior*. Violating
one means we shipped the wrong thing, not that we acted wrong. They derive
from the sovereignty/privacy/equanimity stack and shape every adapter we add.

1. **No central server.** Federation is direct DID resolution. There is no
   broker, registry, directory, or marketplace. Adding one breaks the
   model.

2. **No telemetry.** The daemon never phones home. No usage analytics, no
   crash reporting, no "anonymous metrics." Verification of envelopes is
   self-contained — signature + DID, no external lookup beyond the
   signer's own published `did:web` document (cached on first fetch).

3. **Keys never leave the device.** Backups, if any, are user-encrypted
   with a key only the user holds. No vendor-managed keystore. No cloud
   keychain sync without user-provided pre-encryption.

4. **Transports are adapters, not authorities.** Gmail, Slack, IMAP,
   iMessage, SMS, paper QR — each is a dumb pipe. The envelope body is
   end-to-end encrypted to the recipient's DID-derived encryption key
   (ed25519 → x25519). Transports see *signed ciphertext* — never
   plaintext, never envelope structure beyond outermost addressing,
   never contract terms. Adding a transport must not weaken the trust
   model.

   **Metadata leakage is acknowledged, not hidden.** Email leaks
   who-to-whom-and-when to the provider; the social graph is visible
   even when content is sealed. Users choose transports knowing this.
   Email is the universal bootstrap adapter (everyone has it); steady-
   state correspondence between two installed Secretariats may
   negotiate stronger transports (self-hosted relay, peer-to-peer)
   via the bilateral contract's `preferred_transports` field.

5. **Cognition is pluggable.** The agent loop talks to a `CognitionPort`,
   not a vendor SDK. Adapters wire concrete substrates: Claude Code
   (user's subscription), Anthropic API (BYOK), local models
   (Ollama / llama.cpp / MLX), Bedrock, etc. Choosing a substrate is the
   principal's decision, not the product's. Sovereignty over cognition is
   parallel to sovereignty over keys — the principal must always be able
   to swap the brain.

6. **Correspondence is bilateral or multi-party; always local.**
   Generalized 2026-05-12 from "bilateral and local" to accommodate
   channels. Bilateral correspondence (DM, peer pair) and multi-party
   correspondence (channel membership) share the same primitive — each
   relationship holds its own contract document, signed by the relevant
   principals, carried in the relationship's meta queue (`<channel>:_meta`
   for channels; per-pair contract for DMs). No registry, no directory,
   no marketplace. Discovery is via DID documents (`did:web` `.well-known/`
   exposing channels via the `SecretariatOrg` service entry per
   `tech.equanimi.secretariat.orgDoc`; `did:key` prior-exchange cache for
   peers without a domain).

7. **No SaaS distribution.** A hosted Secretariat collapses the primitive
   — the moment a server holds keys or routes envelopes, sovereignty is
   gone. Distribution is local daemon (menubar app + MCP) plus optional
   self-hosted `did:web` (user's domain) plus on-device transport OAuth
   tokens. App Store ok. Subscription-to-our-service not ok. Closer to
   1Password's old license model than to Notion.

8. **Filesystem is authoritative; the channel directory is the activation
   surface.** Every envelope, contract, skill, agent, and meta record
   exists as a markdown file at a deterministic path under
   `~/.secretariat/`. There is no database-as-source-of-truth — optional
   read-caches (e.g. SQLite for cross-channel queries) are regenerable
   from filesystem walks and never authoritative. Each channel directory
   is *literally* a Claude Code project, using the standard `.claude/`
   convention; `cd <channel-dir> && claude` activates the full context
   for free, with `.claude/{agents,skills,commands}/` tree-walk
   inheritance from org → dept → channel-leaf. Same directory powers
   interactive sessions and headless agents launched by the daemon via
   the Claude Agent SDK. Switching to a DB-as-authority would close the
   AI feedback loop — the architectural moat.

9. **Owner-as-sequencer per channel; cross-channel order not provided.**
   Each channel `(owner_did, handle)` has exactly one canonical
   sequencer — the owner's relay/daemon. Subscribers read the owner's
   sequence; per-channel strong consistency emerges from federation, not
   central authority. Cross-channel global ordering is explicitly NOT
   provided — channels are independent logs; cross-channel causality, if
   needed, is expressed via envelope-hash references. Consensus
   protocols, central registries, and Byzantine-fault-tolerant ordering
   are out of scope; they would close the substrate. Two consumption
   modes on the same primitive: humans poll (15-min floor — anti-
   compulsion), agents push-subscribe (sub-second, no attention to
   compromise).

These constraints are not obstacles to multi-medium reach — they're what
make it possible without becoming yet-another-vendor.

## Architecture at a glance

```
crates/cli         (binary `sec`)            ──▶ application + infrastructure
src-tauri          (GUI shell, placeholder)  ──▶ application + infrastructure
crates/core::application                     ──▶ ports, domain
crates/core::infrastructure                  ──▶ ports, domain (impls)
crates/core::ports                           ──▶ domain (traits)
crates/core::domain                          (no internal deps; no IO)
```

**Hard rule (repeated):** the dependency arrows go down. Domain never
depends on anything else.

See `docs/developer/secretariat-architecture.md` for module-by-module detail,
the wire format, and the threat model.

## Bounded context

Two real downstream use cases shape the wedge:

- **Rafa ↔ Marcelo (the book).** Marcelo Ballestiero is co-authoring
  *Autonomous Enterprise* (245pp draft, April 2026); Secretariat is the
  operational artifact embodying that framework's principles. Recursive
  validation: the book *about* bounded autonomy is being co-authored
  *using* bounded autonomy.
- **Rafa ↔ Christophe (Themia legal briefs).** Will eventually need
  Windows support; today Mac-only.

When making decisions, optimize for these two flows.

## Development practices

- **Read before editing.** Always understand surrounding code.
- **Follow existing patterns** — look at peer modules in the same layer first.
- **Type-driven** — make illegal states unrepresentable. New value objects
  are newtypes with parse-time validation, like `Did` / `DocHash` / `Signature`.
- **Comprehensive tests for domain logic.** Domain is the core; cover the
  invariants. Infrastructure tests focus on real integrations (file IO,
  signature round-trips), not mocks.
- **Quality gates:** run `cargo test --workspace` and `cargo clippy -- -D warnings`
  before claiming work complete.
- **No unsolicited commits.** Only commit when the user explicitly asks.
- **Removing files:** always use `rm -f`.
- **Every principal-facing primitive ships on both interfaces.** When adding
  any new operation that the principal uses (compose, contact ops, invite,
  read, etc.), implement the four surfaces in parallel:
  1. **Application use case** in `crates/core/src/application/<verb>_ops.rs` —
     pure orchestration, IO via the existing port traits.
  2. **CLI command** in `crates/cli/src/commands/<verb>.rs`, registered in
     `cli/src/main.rs`.
  3. **MCP tool** in `crates/mcp/src/server.rs`, exposed via `#[tool]` —
     same parameter shape as the CLI flags, same return shape as the use
     case's output struct.
  4. **Tests** for the use case (unit) + integration tests for the CLI
     and/or MCP surface where the cross-layer contract matters.

  Daemon-only operations (poll, send) and principal-only operations
  (stamp ceremony) are exceptions — see rule 4 and the milestone doc.

## Reuse — skills shipped with this user's `~/.claude/`

When composing envelopes or shaping new primitives, defer to existing skills:

- **`attentional-granularity`** — content structure (gross → subtle, deepening
  pathway). Drives the default content of `~/.secretariat/template.md`.
- **`share`** — drafting shareables. The signature line `_Drafted by AI,
  reviewed by a human._` comes from this skill.
- **`behavioral-design`** — BCT/PDP analysis. Used pre-build to validate the
  ceremony surface against social-reward anti-patterns (we explicitly avoid
  BCT 10.4 — leaderboards, streaks, counts).
- **`ddd`** — when adding new aggregates / value objects, follow the layered
  shape already in place (domain / ports / infrastructure / application).
- **`leverage-points`** — Meadows lens for strategic decisions. Used in
  `equanimitech/docs/ideas/secretariat-leverage-diagnostic.md` for the
  category-fit analysis.

## DID methods

Two methods supported. Default is `did:key`.

| Method | When to use | Hosting |
|---|---|---|
| **`did:key`** | New users, individuals without a domain (Marcelo, Christophe, dad) | Zero — the DID *is* the public key |
| **`did:web`** | Users with a domain they control (Rafa) | A static `.well-known/did.json` over HTTPS |

`sec init` (no args) auto-derives a `did:key` from the freshly generated
verifying key. `sec init --did did:web:rafa.equanimi.tech` opts into the
domain-anchored variant.

## Out of scope (for now)

Pruned 2026-05-12 against the v0.3 direction shift (see
`docs/ideas/2026-05-12-secretariat-as-autonomous-enterprise-substrate.md`).

**Shipped or in-flight (no longer out of scope):**

- Tauri GUI — scaffold + tray + sidecar wiring shipped in v0.2.x; review/onboarding surfaces moving to MCP per the MCP-primary direction.
- MCP server — shipped (`crates/mcp/`).
- Bilateral correspondence transport — relay + invite ship in v0.3; multi-subscriber poll + owner-as-sequencer push come with channels.

**Still out of scope (v0.3 boundary):**

- AT-proto network federation, Iroh, IPFS
- Lexicon publication (schemas remain mutable until self-use validates)
- Cross-platform — Mac-only; Windows when Christophe needs it
- Defer / vouch / dispute / redirect stamp acts (only `attest` for now; reserved values present in the lexicon)
- Counter-stamp record + multi-party stamping ceremony (m.3 process-verbaux — design space defined, v0.4+)
- PDF / docx embedding (markdown only; PDF-share of a stamped envelope is a separate future feature)
- Cryptographic stamp chain (each stamp signing the previous hash)
- Multi-device same-principal sync (key migration UX — v0.4 wedge)
- Channel ownership transfer (`rosterUpdate.op = transfer_ownership` — defer until concrete driver)
- SQLite read-cache for cross-channel queries (defer to v0.4+ when query latency demands it)
- Shared-git skill iteration adapter (optional upstream pattern; not authoritative store)
- Attention routing daemon + UI (composes from existing `depth`/`urgency`/`attentionEnvelope` — v0.4 wedge)
- Webhook adapter for external sources (DID-keyed external services or agent-proxied — v0.4 wedge)

## Reference paths

- v0.3 design report: `docs/ideas/2026-05-12-secretariat-as-autonomous-enterprise-substrate.md`
- Pitch (Day 1): `equanimitech/docs/pitches/2026-04-30-secretariat-stamping-client-mvp.md`
- Plan: `~/.claude/plans/wait-you-have-a-zazzy-aurora.md`
- Source idea: `equanimitech/docs/ideas/secretariat-pitch.md`
- Leverage diagnostic: `equanimitech/docs/ideas/secretariat-leverage-diagnostic.md`
- Primer for Marcelo: `equanimitech/docs/share/2026-04-30-primer-for-marcelo.md`
- Day 1 milestone: `docs/milestones/2026-04-30-first-signed-message.md`
- Onboarding audit (Marcelo): `docs/audits/2026-05-04-onboarding-ux.md`
