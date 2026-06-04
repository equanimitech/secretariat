# Scribe auto-signature — design

**Date:** 2026-06-04
**Status:** Design approved; implementation plan pending.

## Problem

Hard rule #4 mandates the **signature layer**: every authored body carries a
detached DID-keyed signature from its author — human principal *or agent DID*.
On the git-native substrate this is silently unmet. The teardown cut the
compose/envelope path that used to attach the author signature, so when the
scribe (Claude) authors an idea, pitch, or spec, the doc lands as **plain
markdown, unsigned**. It only ever acquires a principal `$attestation` later,
selectively, via Touch ID.

The data model and verify path already support the missing piece: `$signature`
with `signer_role: agent` is a recognized frontmatter layer, and
`verify_document` already returns `SignatureOutcome::OkUnverifiedAgent` for
scribe-authored docs. **What is missing is the write side** — nothing embeds a
`$signature` when the scribe authors. There is no `sec sign`, no MCP `sign`
tool, and no trigger.

This is the narrow gap this design closes. It is the **signature** layer, not
the stamp:

| Act       | Who               | Trigger              | Meaning                          |
| --------- | ----------------- | -------------------- | -------------------------------- |
| **Sign**  | scribe (agent DID)| **auto** (this work) | "Claude authored this" — info    |
| **Stamp** | principal         | Touch ID, selective  | "I attest this" — authoritative  |

Auto-signing from the scribe is consistent with — and required by — invariant
#4. Auto-*stamping* remains forbidden (hard rule #4). This work only does the
first; it never writes `$attestation` and never invokes the biometric gate.

## Feasibility (existing primitives)

No new cryptography is required.

- Each agent gets its own ed25519 keypair at `sec agent add`
  (`generate_keypair`), stored at `<root>/identity/agents/<name>/key`
  (raw PKCS#8, mode `0600`, on-device).
- The key is loadable via `load_signing_key` **without Touch ID** — the
  biometric gate is exclusive to the principal stamp ceremony.
- `EnvelopeSignature::sign_body(did, SignerRole::Agent, body, when, &key)`
  exists (`crates/core/src/domain/signature.rs`).
- `canonical_body_hash` (body-only preimage), `embed_frontmatter` /
  `parse_document` (`crates/core/src/infrastructure/markdown.rs`), and
  `verify_document` all exist.

A hook can therefore call `sec sign`, which loads the scribe key and embeds a
`$signature` block non-interactively.

## Architecture

Layering follows the AGENTS.md down-arrow (domain → ports → infrastructure →
application → CLI / MCP). The Claude hook sits outside the crates as a trigger.

```
.claude Stop hook ──▶ sec sign <file>… ──▶ application: sign_document
                       mcp sign tool   ──┘            │
                                                       ▼
                                    canonical_body_hash + load_signing_key
                                    + EnvelopeSignature::sign_body(Agent)
                                    + embed_frontmatter → write back
```

### 1. `sign_document` use case

New file `crates/core/src/application/sign_ops.rs`. Pure orchestration; all IO
via ports; time enters as a parameter (domain rule #2). Per file:

```
read → parse_document → guards (below) →
canonical_body_hash(body) → resolve scribe agent → load agent key →
EnvelopeSignature::sign_body(agent_did, Agent, body, when, key) →
embed $signature → write back
```

The preimage is **body-only** via the existing `canonical_body_hash` — this
keeps the new signatures consistent with the 31 live seals and avoids the
signet posture-B divergence (body-only vs frontmatter+body). Do not introduce a
new preimage here.

### 2. Guards (trust-model-critical)

The use case MUST refuse to sign when any of these hold:

- **`$attestation` present** — the doc is stamped; signing under a principal
  seal would re-author authoritative content. Skip.
- **Excluded path** — anything outside `docs/`, plus `data/`, `journals/`, any
  dated `YYYY-MM-DD.md` file, and `identity.md` (reserved-key file). Skip.
- **Unchanged body** — if a scribe `$signature` already exists and its hash
  equals the current `canonical_body_hash`, no-op (idempotent).

The use case only ever writes a `signer_role: agent` `$signature`. It never
writes `$attestation` and never calls the signer port's Touch-ID path.

### 3. Agent (scribe) resolution

A helper resolves which authorized agent signs:

1. Exactly one entry in `authorized_agents` → use it.
2. Otherwise, match the entry whose `substrate` equals the active
   `[cognition]` substrate in `preferences.toml` (e.g. `claude-code`).
3. Zero matches or 2+ matches → error naming the candidates.

This needs no new config for the common case (single agent, or Claude vs
LM-Studio split where each signs as itself). An explicit `--as <name>` flag
(see CLI) overrides resolution.

### 4. `sec sign` CLI

New `crates/cli/src/commands/sign.rs`, registered in `crates/cli/src/main.rs`.

```
sec sign <file>… [--as <name>]
```

Signs each path through the `sign_document` use case. `--as` overrides agent
resolution. Reports per-file outcome (signed / skipped:reason / error).

### 5. MCP `sign` tool

New `#[tool]` in `crates/mcp/src/server.rs`, calling the same use case. Fits the
server's stamp / read / verify bounded context. Accepts a file path (or paths);
returns per-file outcome. Carries no Touch-ID ceremony — signing is not
stamping.

### 6. Stop hook (trigger)

A `Stop` hook in `.claude/settings.json` runs a script that:

1. Enumerates dirty docs: `git diff --name-only` + untracked, filtered to
   `docs/**/*.md`.
2. Runs `sec sign` on each, using the **prod binary**
   `/Applications/Secretariat.app/Contents/MacOS/sec` (hard rule #8 — live
   identity/keys).
3. Is best-effort: logs failures, never blocks or fails the turn.

The Stop boundary (turn end) is chosen over PostToolUse (per-write churn) and
git post-commit (can't distinguish scribe from principal edits). One signature
per authoring boundary. The hook only fires under the Claude Code substrate;
the `sec sign` mechanism remains substrate-portable for other cognition
providers.

## Data flow

```
Claude authors/edits docs/*.md (N edits)
        │
   [turn ends] ──▶ Stop hook
        │
   dirty docs/*.md ──▶ sec sign (prod binary)
        │
   sign_document ──▶ $signature { signer_role: agent, did, sig, hash, signed_at }
        │
   later: sec verify ──▶ SignatureOutcome::OkUnverifiedAgent
```

## Error handling

- No agent configured → error (cannot sign; nothing to attribute authorship
  to).
- Ambiguous substrate match → error naming the candidate agents.
- Path missing or not markdown → error for that path; other paths proceed.
- Hook layer swallows and logs all of the above so a turn never fails because
  signing failed.

## Testing

Use-case (`sign_ops`):

- signs an unsigned doc;
- skips a stamped doc (`$attestation` present);
- skips an excluded path (`data/`, `journals/`, dated, `identity.md`, non-`docs/`);
- idempotent — no rewrite when body hash unchanged;
- re-signs when the body changed;
- agent resolution: single agent, substrate match, ambiguous error, none error.

Cross-layer contract: the CLI command and the MCP tool both route through the
same `sign_document` use case (one behavior, two surfaces).

Test hygiene: never embed real DIDs — generate via
`Did::from_ed25519_public_key(&[seed; 32])`.

Quality gates before complete: `cargo test --workspace` and
`cargo clippy -- -D warnings`.

## Out of scope

- Counter-stamp / multi-party signing.
- Re-signing docs the principal authored or stamped.
- Signing non-`docs/` content (code, config, journals).
- Substrate adapters other than Claude Code wiring the Stop hook (the `sec
  sign` mechanism is portable; only the Claude trigger ships here).
- Verifier chain Phase C (agent-manifest authorization lookup) — already
  tracked separately; this work produces `OkUnverifiedAgent` as today.
