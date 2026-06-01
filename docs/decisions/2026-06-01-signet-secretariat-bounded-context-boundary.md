---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:aab6ef8907186fa47771dc441261be9c3b95cf6e5f307ff4a4e472fccf0ecd85
  docFilename: 2026-06-01-signet-secretariat-bounded-context-boundary.md
  stampedAt: 2026-06-01T15:46:13.592385Z
  signature: ed25519:7GLKBwPjXkPlHnGohfrtfNUY7AlwQvctjh7vJfb/jB0S0leOl5KXfrzibQgjh/ZzRTJwVNKFNtQ1bpUXLQaVDA==
---
# Establish the Signet ↔ Secretariat bounded-context boundary

**Date:** 2026-06-01
**Context:** `equanimitech/signet` + `equanimitech/secretariat` (cross-repo architecture)

## Decision

Two bounded contexts, one dependency arrow:

- **Signet — the trust primitive.** One responsibility: `sign` · `stamp` · `verify`
  over a markdown doc and its `$attestation`. Pure. No IO opinions, no transports,
  no channels, no knowledge of where a doc goes. A reusable, transport-agnostic
  crypto core. *Signet seals.*
- **Secretariat — orchestration over the git-native substrate.** Everything that
  is not the crypto: walk the repos, surface stamp state, drive the review/stamp
  ceremony, decide what gets elevated, and forward sealed docs to transports
  (e.g. Slack). Secretariat **calls** Signet to seal; it never seals itself.

The dependency arrow points **down**: orchestration → primitive, never the
reverse. Signet does not know Secretariat, Slack, or any transport exists.

## Rationale

Clean DDD separation. It keeps Signet a small, reusable, auditable trust core
and lets every outward feature (Slack-forward, future email/relay adapters,
review walkers) attach to **Secretariat**, keyed off a Signet **seal event** —
without ever widening the primitive. Transports become orchestration concerns,
not crypto concerns; adding one cannot weaken the seal. This is the same
"transports are adapters, not authorities" invariant applied at the
crate/product boundary: the seal is the authority, the orchestration is the pipe.

Concretely, it answers where the Slack-forward feature lives: entirely in
Secretariat (global `[slack]` integration config + per-repo `forward.toml`
routing + a post-stamp forward step). Signet stays untouched.

## Consequences

- New outward capabilities (forwarding, routing, notification) land in
  Secretariat, never in Signet.
- Signet's public surface stays minimal — sign/stamp/verify value objects
  (`Seal`, `Attestation`, `DocHash`) and nothing transport-shaped.
- **Caveat — the split is aspirational today, not finished.** Stamp logic
  currently lives in **two places**: `sec`'s embedded stamp core and the
  `signet` crate. They diverge on:
  - **hash preimage** — body-only vs frontmatter+body
  - **record shape** — `$attestation` object vs `$signatures` array

  Plan: evolve `signet` into the **one protocol** that everything (sec,
  Secretariat orchestration) calls, **CI-gated on continuity of the 31 existing
  seals** (incl. the boat) so the unification breaks nothing. Until that merge
  lands, treat this document as the target state, and any new seal/verify code
  as a step toward it — not a second parallel implementation.

- Supersedes nothing; establishes the boundary as a first-class architectural
  invariant for both repos.
