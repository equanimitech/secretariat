---
$signature:
  $type: tech.equanimi.secretariat.signature
  signer: did:key:z6MkpcX3mHt44yNEDPDWJic8ocJdagzERxx5u2Qh1dWcVRVN
  signerRole: agent
  docHash: sha256:0199aba247ce34469ddac3c5a8b9999b49a8f3a602fd57eda584a62152a50122
  signedAt: 2026-06-14T19:34:40.132957Z
  signature: ed25519:TLeNAieZxmS8avyJQgBS0zynbyyTDWTJhqwctTgMIbf5KM9WxMhVNDUgpTfDv++uTHOrhwwC/b1Dat5f5vrtDQ==
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:0199aba247ce34469ddac3c5a8b9999b49a8f3a602fd57eda584a62152a50122
  docFilename: 2026-06-14-finish-the-git-native-teardown-then-converge-on-signet.md
  stampedAt: 2026-06-14T19:34:51.345138Z
  signature: ed25519:bYtQmgZ5R20XImM2n25v/OP1EKQfkAm4OQY1yZqE6J8duaSNV0NavKNz/wz1xBKLTvGEaIjGJJd/IZna3fjCAA==
---
> Surfaced by a ponytail over-engineering audit (2026-06-14). Two structural cuts, sequenced. Findings only; nothing applied. The first is pure deletion; the second is the documented signet convergence, quantified.

## The shape of it

The core crate carries weight from a richer past it no longer ships. Two cuts roughly halve it (16,767 lines today):

1. **Finish the v0.12 teardown.** ~5,100 lines of correspondence apparatus declared cut but still compiled.
2. **Converge on signet.** ~3,300 lines of embedded seal core that should be a dependency on `signet-core`, per the 2026-06-01 boundary decision.

Do them in that order. The first is safe deletion with no protocol risk; the second is gated on seal continuity.

## Cut 1: finish the teardown (do first)

AGENTS.md states the correspondence apparatus (federation, relay, channels, orgs, contracts, compose-apparatus, queue) was cut in v0.12. In practice only the user-facing callers were removed. The ops, stores, and domain types still compile in `crates/core/`.

Evidence of orphaning: the live verbs reference almost none of it. Across `crates/cli`, `crates/mcp`, and `src-tauri/src`, these show zero references:

- `channels_ops`, `org_ops`, `contract_store`, `inbox_ops`, `queue_dir`, `create_channel`, `create_org`.

The live MCP tool set is exactly: stamp, read, compose, verify, agent_add/list/remove/rotate, repo_add/list/remove, timeline. No channel, org, contract, or inbox tool exists. The CLI matches.

Approximate size of the still-compiled apparatus: ~5,100 lines across `domain/{channel_binding, channel_contract, channel_def, org, org_alias, recipient, queue_handle, contact, scope_intent, root}`, `application/{channels_ops, org_ops, inbox_ops}`, and `infrastructure/{channel_def_store, contract_store, org_store, queue_dir, binding_store, transport}`.

What must stay (caller-trace per module before deleting, not a blanket removal):

- `channels_ops::read_channel` and `ChannelEnvelope`. The Tauri timeline keeper projects a channel-dir `envelopes/` tree and still reads them. A comment in `application/mod.rs` flags this explicitly.
- `launch_channel`. Live via `sec launch` (3 references).
- `workflow` and `workflow_ops`. Live via the `sec workflow` command (1 reference).

Also verify, do not assume: the post-teardown encryption layer (`infrastructure/crypto/sealed.rs`, plus `x25519-dalek` and `chacha20poly1305`). `chacha20poly1305` and `ciphertext_dir` show zero live-surface references; `sealed`, `x25519`, and `Envelope` retain a few. This looks like residue of the relay ciphertext queue. Trace it before cutting; if dead, two crypto dependencies go with it.

## Cut 2: converge on signet (do second, gated)

The 2026-06-01 boundary decision is already stamped: signet seals, secretariat orchestrates and calls signet to seal, the dependency arrow points down. The architecture is correct on paper. Reality diverges exactly as that decision's own caveat predicted:

- **No arrow exists yet.** Secretariat has no `signet` dependency in any Cargo.toml or Rust file (only doc references). It carries a parallel ~3,300-line seal core: `infrastructure/{markdown, keys, ed25519_signer, did_key_resolver, did_web_resolver, composite_did_resolver, crypto}` and `domain/{identity, signature, stamp, acts, attested_document, envelope}`. signet-core is ~2,788 lines covering the same sign/stamp/verify primitive.
- **They diverge on wire format.** Hash preimage is body-only here vs frontmatter-plus-body in signet; the record is an `$attestation` object here vs signet's `$signatures` array. So a dependency swap is not drop-in.

This is the single largest structural simplification available in the equanimitech tree, and it is already the documented plan, deferred. The blocker is one thing: CI continuity of the existing seals (the 31 seals, including the boat) so the merge breaks nothing. That gate is the precondition, not a reason to defer indefinitely.

## Sequencing and payoff

- Cut 1 first: pure deletion, no seal-continuity concern, the bigger and safer win (~5,000 lines).
- Cut 2 second: behind the seal-continuity CI gate the boundary decision specifies (~3,000 lines net of the signet dep added back).
- Together: close to halving the core crate, and collapsing the seal logic to one implementation that both `sec` and any future orchestration call.

## Open questions

- Is the `read_channel` and `ChannelEnvelope` timeline dependency worth keeping, or should the timeline keeper read the envelope tree directly so the whole channels module can go?
- Does the encryption layer have any surviving git-native use (sealed at-rest documents), or is it pure relay residue?
- What is the smallest CI fixture that proves seal continuity across the preimage and record-shape change, so cut 2 can start?
