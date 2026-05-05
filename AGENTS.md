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

4. **Stamp ceremony is principal-attested, not Claude-attested.** Claude
   *may* initiate `stamp` (via the MCP tool or `sec stamp`) but MUST first
   show the principal the full decrypted body verbatim — code block or
   quoted region, never a summary — and obtain explicit confirmation in
   the same turn. Implicit consent from a prior turn does not count if the
   file changed.

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

5. **Compose envelopes following `~/.secretariat/template.md`.** This is the
   user-customizable AG (attentional-granularity) template, owned by the
   principal. It is to envelope composition what `CLAUDE.md` is to general
   Claude behavior.

6. **Respect `~/.secretariat/attention-envelope.md`.** The principal's
   declared bounds (depths, urgencies, cadence). Queue to `outbox` instead of
   surfacing inline if the bid would violate cadence. The protocol detects
   bound violations; Claude pre-empts them.

7. **Place drafts in `~/.secretariat/outbox/<recipient-did>/`.** Never write
   draft envelopes into the working directory. The outbox is the queue the
   principal stamps from.

8. **Use `sec verify --json`** when consuming incoming attested envelopes
   from another principal. Never trust an envelope without verifying.

9. **The `/share` signature line is required at the end of every envelope:**

   ```
   ---

   _Drafted by AI, reviewed by a human._
   ```

   This is in addition to the cryptographic stamp. Recipients without `sec`
   installed can still see that the document passed through a human edit pass.

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

6. **Contracts are bilateral and local.** No registry, no directory, no
   marketplace. Each pair holds its own contract document, signed by both.
   Discovery is via DID documents (`did:web` `.well-known/` or `did:key`
   prior-exchange cache).

7. **No SaaS distribution.** A hosted Secretariat collapses the primitive
   — the moment a server holds keys or routes envelopes, sovereignty is
   gone. Distribution is local daemon (menubar app + MCP) plus optional
   self-hosted `did:web` (user's domain) plus on-device transport OAuth
   tokens. App Store ok. Subscription-to-our-service not ok. Closer to
   1Password's old license model than to Notion.

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

Per the pitch's no-go list:

- Tauri GUI (the scaffold exists; ceremony surface is a future pitch)
- MCP server
- Bilateral correspondence transport (server, peer queue, push)
- AT-proto network federation, Iroh, IPFS
- Lexicon publication (schemas are mutable until self-use validates)
- Cross-platform — Mac-only Day 1; Windows when the GUI lands
- Defer / vouch / dispute / redirect acts (only `attest` for now)
- PDF / docx embedding (markdown only)
- Cryptographic stamp chain (each stamp signing the previous hash)

## Reference paths

- Pitch: `equanimitech/docs/pitches/2026-04-30-secretariat-stamping-client-mvp.md`
- Plan: `~/.claude/plans/wait-you-have-a-zazzy-aurora.md`
- Source idea: `equanimitech/docs/ideas/secretariat-pitch.md`
- Leverage diagnostic: `equanimitech/docs/ideas/secretariat-leverage-diagnostic.md`
- Primer for Marcelo: `equanimitech/docs/share/2026-04-30-primer-for-marcelo.md`
- Day 1 milestone: `docs/milestones/2026-04-30-first-signed-message.md`
