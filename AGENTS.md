# Secretariat — Claude orchestration

Secretariat is a cryptographically attested, AI-mediated **markdown editor**.
The principal is the human; Claude is the scribe. Claude composes and edits,
the principal **stamps**. Stamps are biometrically gated (Touch ID).

This file is read at the start of every Claude Code session. It overrides the
generic template defaults (the project was bootstrapped from
`dannysmith/tauri-template` but the bounded context is Secretariat).

> **v0.12.0 git-native teardown.** The correspondence apparatus —
> federation/relay, channels, orgs, contracts, compose, capture, invite, the
> review queue — was cut. What remains is the markdown editor + the Signet
> **stamp / verify / read** core over a **git-native substrate** (documents are
> markdown in git repos; identity + key live under `~/.secretariat/`). Pitches
> and ideas describing the cut apparatus are **historical records**, not
> current intent. See `CHANGELOG.md` and
> `docs/ideas/2026-05-31-git-native-substrate.md`.

## What's here today

- **CLI (`sec`)** — works end-to-end on macOS. Subcommands:
  `init` / `agent` / `stamp` / `verify` / `read` / `launch` / `mcp` /
  `daemon` / `profile` / `view`. See
  `docs/developer/secretariat-architecture.md`.
- **MCP server (`sec-mcp`)** — built. Tools: `stamp`, `read`, `verify`,
  `agent_add`, `agent_list`, `agent_remove`, `agent_rotate`. That is the
  complete current set. The server `instructions` carry the stamp ceremony.
  Source at `crates/mcp/`.
- **Tauri shell** — the **markdown editor**: read/edit markdown, frontmatter
  sidebar, the stamp ceremony UI, command palette, quick pane. Bundles `sec` +
  `sec-mcp` as sidecars and wires them on launch (`sec mcp install`,
  `sec daemon install`). No notifications, no push — anti-compulsion by design.
- **`sec launch`** — opens Claude Code (or any configured cognition CLI) with
  `cwd` set to a repo. The optional `root_path` override resolves the bound
  directory (typically a git repo). Cognition substrate is config-driven via
  `[cognition]` in `preferences.toml`. See `docs/developer/launch.md`.

  Sidecars are staged into `src-tauri/binaries/` by
  `src-tauri/scripts/build-sidecars.sh`, which Tauri's `beforeBuildCommand`
  runs automatically during `pnpm tauri:dev` / `pnpm tauri:build`. For a bare
  `cargo check -p secretariat` on a clean clone you must run that script once
  first (otherwise `tauri-build` fails with _"resource path
  `binaries/sec-<triple>` doesn't exist"_).

## Vocabulary

Two-layer naming. Same actor, two names depending on layer of discussion:

- **Protocol / cryptographic layer — `agent`.** Any non-principal DID-keyed
  identity that signs on a principal's behalf. The neutral term in low-level
  descriptions (the DID, the signature, the `authorized_agents` field).
- **Substrate / UX layer — `scribe`.** An agent with `role: "scribe"`. The
  role surfaced in onboarding prose ("Add Claude as your scribe?"). Today the
  only role; future roles (`auditor`, `reader`) reuse the same field shape.

Identity-record `authorized_agents` entries are shaped
`{did, role, name, substrate, added_at}` — `name` is the principal-chosen
nickname, `substrate` is the cognition provider (`claude-code`, `opencode`,
`anthropic-api`, etc.). Granting an agent is an explicit act of authority
delegation (architectural invariant #4). `sec init` does NOT auto-provision
agents; the principal runs `sec agent add <name> --role scribe --substrate
<substrate>` (or `mcp__secretariat__agent_add`). CLI/MCP vocabulary uses
`agent`; UX surfaces use the role name (_"scribe"_).

`authorized_agents` lives in `identity.md` (principal-scoped). The
org-/channel-scoped variants the pre-teardown design speculated about lapsed
with the orgs/channels cut — only principal-scoped ships.

## Hard rules

These are non-negotiable. They override the template's defaults where they
conflict.

1. **Use pnpm, not npm.** All front-end commands run as `pnpm install` /
   `pnpm tauri:dev` etc.

2. **Domain layer (`crates/core/src/domain/`) imports no IO.** No `std::fs`,
   no `reqwest`, no `chrono::Utc::now()`. Time and randomness enter via
   parameters. Aggregates enforce invariants at construction.

3. **AT-proto-lexicon-shaped records — `lexicons/` is the source of truth, by
   practice.** Every record type has a `$type` discriminator (e.g.
   `tech.equanimi.secretariat.stamp`). Schemas live under `lexicons/` and are
   authoritative for the on-wire shape. When you change any record shape, the
   lexicon edit lands in the **same commit** as the Rust change. A
   record-shape PR without a `lexicons/` diff is a stop-the-line event.

4. **Three-layer trust model: signature mandatory, stamp selective,
   counter-stamp reserved.**
   - **Signature** — every authored body carries a detached DID-keyed
     signature from its author (human principal or agent DID). Drives
     `sec verify` provenance.
   - **Stamp** — principal Touch-ID attestation, embedded as an `$attestation`
     block in place. **Selective, not mandatory.** Applied to documents the
     principal elects to elevate (decisions, commitments, contracts). The
     stamped subset _is_ the authoritative record.
   - **Counter-stamp** — multi-principal stamp on the same document. Design
     space defined in the lexicon; no record type ships yet.

   **Stamp ceremony is principal-attested, not Claude-attested.** Claude _may_
   initiate `stamp` (via the MCP tool or `sec stamp`) but MUST first:
   1. `read` the file.
   2. Render the **full body verbatim** — code block or quoted region, never a
      summary.
   3. Obtain **explicit consent in the same turn**. Prior-turn consent does
      not count if the file changed.
   4. Then stamp. The Touch ID dialog reason carries the document's first-line
      headline + a short hash prefix; if it differs from what Claude displayed,
      **abort**.

   The biometric gate blocks until the principal physically authorizes. Claude
   never stamps — it proposes; the principal seals. (Touch ID gates regardless
   of caller, so who-types-the-command is not the threat model; reading what
   you stamp is.)

   **Receiver-side discipline:** `sec verify --json` returns layered results —
   `{signature, stamp, counter_stamps}`. An unstamped-but-signed document is
   _informational_; a stamped document is _authoritative_. Agents acting on a
   document MUST treat signed-only ≠ stamped.

5. **Verify before trusting.** Never trust a document's claims without
   `sec verify`. A document whose signature fails is malformed and must be
   quarantined, not surfaced. (See `/review-repos` for the git-native review
   walker that derives stamp state per doc.)

6. **The principal owns the closing line.** Claude does NOT auto-append a
   signature line. If the principal has a configured closing line, use it
   verbatim; if not, end without one. The cryptographic stamp records human
   disposition; the body closing line is editorial, not protocol.

7. **Tauri v2 only.** Modern Rust formatting (`format!("{variable}")`).

8. **Prod binary for live invocations.** For any `sec` call against the live
   identity/keys, use `/Applications/Secretariat.app/Contents/MacOS/sec`, never
   `./target/debug/sec`.

## Architectural invariants

Properties of the _system_, not rules of _behavior_. Violating one means we
shipped the wrong thing.

1. **No central server.** Identity is direct DID resolution. No broker,
   registry, directory, or marketplace.

2. **No telemetry.** Nothing phones home. Verification is self-contained —
   signature + DID, no external lookup beyond the signer's published `did:web`
   document (cached on first fetch).

3. **Keys never leave the device.** Backups, if any, are user-encrypted with a
   key only the user holds. No vendor-managed keystore.

4. **Cognition is pluggable.** The agent loop talks to a `CognitionPort`, not a
   vendor SDK. Adapters wire concrete substrates: Claude Code (subscription),
   Anthropic API (BYOK), local models (Ollama / llama.cpp / MLX), Bedrock.
   Choosing a substrate is the principal's decision. Sovereignty over cognition
   parallels sovereignty over keys.

5. **Filesystem is authoritative; the git repo is the substrate.** Every
   document, identity, and instruction is a markdown file — in a git repo
   (documents) or under `~/.secretariat/` (identity + key). No
   database-as-truth; optional read-caches are regenerable and never
   authoritative. A repo's doc surface _is_ the activation surface:
   `cd <repo> && claude` activates the full context for free.

6. **No SaaS distribution.** A hosted Secretariat collapses the primitive the
   moment a server holds keys. Distribution is local app + CLI plus optional
   self-hosted `did:web`. App Store ok. Subscription-to-our-service not ok.

> **Lapsed invariants (historical).** Pre-teardown the system also asserted
> transports-as-adapters, bilateral/multi-party correspondence, and
> owner-as-sequencer channels. These lapsed with the correspondence apparatus.
> If transports return (e.g. Slack-forward keyed off a seal event — see the
> Signet↔Secretariat boundary), they re-enter as adapters that never weaken the
> trust model.

## Architecture at a glance

```
crates/cli         (binary `sec`)              ──▶ application + infrastructure
crates/mcp         (binary `sec-mcp`)          ──▶ application + infrastructure
crates/daemon      (macOS LaunchAgent surface) ──▶ install/uninstall/status + keepalive
src-tauri          (markdown editor + tray)    ──▶ sidecar wiring + editor UI
crates/cognition-claude-sdk                    ──▶ CognitionPort adapter
crates/core::application                       ──▶ ports, domain
crates/core::infrastructure                    ──▶ ports, domain (impls)
crates/core::ports                             ──▶ domain (traits)
crates/core::domain                            (no internal deps; no IO)
```

**Hard rule (repeated):** the dependency arrows go down. Domain never depends
on anything else. The `relay/` crate was deleted in the teardown.

See `docs/developer/secretariat-architecture.md` for module-by-module detail,
the wire format, and the threat model.

## Bounded context

Two real downstream use cases shape the wedge:

- **Rafa ↔ Marcelo (the book).** Marcelo Ballestiero authors _Autonomous
  Enterprise_ (245pp draft, April 2026); Secretariat is the operational
  artifact embodying that framework's principles. (Marcelo is the sole author;
  Rafa builds Secretariat, does not co-author.)
- **Rafa ↔ Christophe (Themia legal briefs).** Will eventually need Windows
  support; today Mac-only.

When making decisions, optimize for these two flows.

## Development practices

- **Read before editing.** Always understand surrounding code.
- **Follow existing patterns** — look at peer modules in the same layer first.
- **Type-driven** — make illegal states unrepresentable. New value objects are
  newtypes with parse-time validation, like `Did` / `DocHash` / `Signature`.
- **Comprehensive tests for domain logic.** Domain is the core; cover the
  invariants. Infrastructure tests focus on real integrations (file IO,
  signature round-trips), not mocks.
- **Quality gates:** run `cargo test --workspace` and
  `cargo clippy -- -D warnings` before claiming work complete.
- **No unsolicited commits.** Only commit when the user explicitly asks.
- **Removing files:** prefer `git rm` for committed files (history preserves
  them); never silently lose history.
- **New principal-facing primitives ship on parallel surfaces.** When adding
  an operation the principal uses (e.g. a new stamp act, an agent op),
  implement: (1) the application use case in
  `crates/core/src/application/<verb>_ops.rs` (pure orchestration, IO via
  ports); (2) the CLI command in `crates/cli/src/commands/<verb>.rs`,
  registered in `cli/src/main.rs`; (3) the MCP tool in
  `crates/mcp/src/server.rs` via `#[tool]`; (4) tests for the use case + the
  cross-layer contract. (Stamp ceremony is principal-only — see hard rule #4.)

## DID methods

Two methods supported. Default is `did:key`.

| Method        | When to use                                | Hosting                            |
| ------------- | ------------------------------------------ | ---------------------------------- |
| **`did:key`** | New users, individuals without a domain    | Zero — the DID _is_ the public key |
| **`did:web`** | Users with a domain they control (Rafa)    | A static `.well-known/did.json`    |

`sec init` (no args) auto-derives a `did:key`. `sec init --did
did:web:rafa.equanimi.tech` opts into the domain-anchored variant.

## Out of scope (for now)

Current shipping state lives in `CHANGELOG.md`. Deliberately not built:

- **The cut correspondence apparatus** — federation/relay, channels, orgs,
  contracts-as-feature, compose, capture, invite, review queue. Removed in
  v0.12.0; not coming back as-was. The direction is git-native.
- Cross-platform — Mac-only; Windows when Christophe needs it.
- `defer` / `vouch` / `dispute` / `redirect` stamp acts (only `attest`;
  reserved values present in the lexicon).
- Counter-stamp record + multi-party stamping ceremony.
- Lexicon publication (schemas remain mutable until self-use validates).
- PDF / docx embedding (markdown only).
- Cryptographic stamp chain (each stamp signing the previous hash).
- Signet-crate convergence to a single stamp core (CI-gated on seal
  continuity — see the boundary decision).
- Multi-device same-principal sync (key migration UX).

## Reference paths

- Architecture: `docs/developer/secretariat-architecture.md`
- Launch: `docs/developer/launch.md`
- Git-native substrate (the teardown rationale):
  `docs/ideas/2026-05-31-git-native-substrate.md`
- Signet protocol: `docs/ideas/2026-05-27-signet-protocol.md`
- Bounded-context boundary (Signet ↔ Secretariat):
  `docs/decisions/2026-06-01-signet-secretariat-bounded-context-boundary.md`
- Review walker: `.claude/skills/review-repos/SKILL.md`

## Reuse — skills

- **`/review-repos`** — the git-native review walker: derives stamp state per
  doc across a repo, renders coarse→fine, stamps on consent. The live review
  surface (the old `/review` queue-triage skill was deleted).
- The personal capture skills (`/decision` `/idea` `/pain` `/question` `/log`)
  moved out of the product into `~/.claude/skills/`. `/decision` writes
  `docs/decisions/*.md` then stamps in place; the others capture to Things
  (repo `docs/` when code-tied).
- **`ddd`** — when adding aggregates / value objects, follow the layered shape
  (domain / ports / infrastructure / application).
