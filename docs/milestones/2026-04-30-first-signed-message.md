---
$envelope:
  $type: app.equanimi.secretariat.envelope
  from: did:web:rafa.equanimi.tech
  to: did:web:marcelo.ballestiero.com
  depth: subtle
  urgency: soon
  source: claude-code-secretariat-day1
$attestation:
  $type: app.equanimi.secretariat.stamp
  signer: did:web:rafa.equanimi.tech
  act: attest
  docHash: sha256:c35f1d465d91998ef16ba8ac921d36aee3ebe003efd9947906e7f443a38f4d34
  docFilename: 20260430T155833Z-eaxh7v.md
  stampedAt: 2026-04-30T16:01:35.220898Z
  signature: ed25519:TENFy3k2MYKI1XsVDy1h9eo16U6bEKieoS1wZneH6VuMz3lHnm6O0sUXiJ9u+Kk/swaLVA4Bljd5kGVIZRQtDQ==
---
# Secretariat — first signed message

**Lede:** Day 1 of Secretariat is functionally end-to-end. This file proves it.

**Why it matters:** the architectural lineage in *Autonomous Enterprise* — humans govern, machines operate (p131); bounds propagate, goals don't cascade (p67); signals, not reports (p88) — now has a working operational artifact, scoped to a single principal. The wedge is real, in code, hours after planning ended.

## What works today

- ed25519 signing key generated locally; private key never leaves my Mac
- Touch ID gates every stamp via `LocalAuthentication.framework` — biometric proof of physical presence at the moment of attestation
- AT-proto-lexicon-shaped envelope and stamp records; v2 PDS migration is mechanical (no schema translation)
- did:web identity (a static `.well-known/did.json` to host on rafa.equanimi.tech)
- YAML frontmatter embedding, byte-preserving round-trip, hash invariant enforced by the `AttestedDocument` aggregate
- Offline verification — the recipient never depends on a Secretariat server, only on the signer's static DID document
- Tamper detection — any byte change to the body breaks the stamp's hash invariant
- 52 unit tests passing across domain, infrastructure, and application layers
- All five `verify_document` outcomes covered (verified, tampered, unsigned, signer unresolvable, signature invalid)

## What this stamp is

This very file. The `$attestation` block in this document's frontmatter is a real ed25519 signature over the canonical body hash, produced by my Mac's Secure Enclave after I touched the sensor. Anyone with my public key (published as the `Ed25519VerificationKey2020` in my did:web document) can verify it offline.

If you change a single byte of the body below this line and re-run `sec verify`, the verification will fail with `✗ tampered`. If you remove the `$attestation` block, it will fail with `✗ unsigned`. If you replace my public key, it will fail with `✗ signature does not verify`.

## Go deeper

- Repo: `github.com/equanimitech/secretariat`
- Plan: `~/.claude/plans/wait-you-have-a-zazzy-aurora.md`
- Pitch: `equanimitech/docs/pitches/2026-04-30-secretariat-stamping-client-mvp.md`
- Primer with the *Autonomous Enterprise* page-by-page mapping: `equanimitech/docs/share/2026-04-30-primer-for-marcelo.md`

---

_Drafted by AI, reviewed by a human._
