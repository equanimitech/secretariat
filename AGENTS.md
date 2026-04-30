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
  `verify` / `list`. 61 unit tests passing. See `docs/developer/secretariat-architecture.md`.
- **Tauri shell** — scaffolded but unused. The ceremony GUI is a future increment.
- **MCP server** — not built yet. Claude orchestrates via `Bash` against `sec` for now.

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

4. **Never inline-stamp.** Only the principal stamps. Claude never calls
   `sec stamp` on its own. Claude composes envelopes; the principal triggers
   stamping at cadence. The biometric gate is the firewall — if Claude could
   stamp, the whole primitive collapses to forgery.

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
