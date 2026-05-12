# Substrate layout for v0.3 — passport-rooted, key-as-proof

**Date:** 2026-05-12
**Status:** accepted (Rafa, 2026-05-12)
**Supersedes:** the v0.2.x flat `~/.secretariat/{key,did,inbox/,outbox/,queues/}` layout
**Predecessor docs:**
- `docs/ideas/2026-05-12-secretariat-as-autonomous-enterprise-substrate.md`
- `docs/ideas/2026-05-12-workspace-registry-and-repo-local-substrate.md` (deferred)
- `docs/ideas/2026-05-12-end-state-substrate-monoslice.md` (alternative path, rejected)

---

## Context

v0.3's channel-tree direction (`channel:dommage-corporel:paris-cohort` →
`<root>/channel/dommage-corporel/paris-cohort/envelopes/YYYY/MM/DD/`)
forces a layout decision: where does the principal's identity live
relative to the org/channel trees? Putting `key`/`did`/`profile.json`
at root next to `themia.pro/`, `marcelo/`, `rafa.equanimi.tech/` mixes
three categories (identity files + substrate work-dirs + per-org trees)
and won't scale once a principal subscribes to 5+ orgs or carries 2+
passports.

A v0.2.x → v0.3 migration is acceptable: the install base is small
(Rafa-only in production, plus one onboarding attempt with Marcelo).
Clean-slate is cheaper than designing around legacy.

## Decisions

### 1. Passport-rooted layout

Each principal-controlled identity (a "passport") is a self-contained
subtree directly under the substrate root. All identity-bearing state
lives inside it.

```
~/.secretariat/
├── <passport-handle>/                # one dir per passport — HAS a `key` file
│   ├── .identity                     # role: passport, canonical DID, handle binding
│   ├── key                           # ed25519 PKCS#8, mode 0600 — THE proof
│   ├── did                           # cross-checked against key on startup
│   ├── profile.json
│   ├── attention-envelope.md
│   ├── template.md
│   ├── contacts.json
│   ├── cognition.json
│   ├── CLAUDE.md
│   ├── .claude/{skills,agents,commands}/
│   ├── queues/                       # flat captures (inbox:triage, area:*)
│   └── channel/                      # channel-tree (channel:*)
│       └── <segs>/envelopes/YYYY/MM/DD/<ts>-<hash>.md
│
├── <other-passport-handle>/          # (multi-passport future — v0.4+)
│
├── <org-or-peer-handle>/             # subscriptions — NO `key` file
│   ├── .identity                     # role: org-subscription | peer-subscription
│   ├── CLAUDE.md                     # context from owner's _meta
│   ├── .claude/                      # skills/agents from owner's _meta
│   └── channel/<segs>/{envelopes,outbox,_meta,_ciphertext}/
│
├── peers/                            # substrate-global did doc cache
└── bin/                              # helper binaries
```

### 2. Identity detection: key-as-proof, no sidecar pointer

A passport is provable by possession of the private key, not by a
sidecar file. Startup:

1. Scan `~/.secretariat/*/key` (mode 0600).
2. v0.3 single-passport: exactly one match expected.
3. Load key → derive public key.
4. Read sibling `did` file.
5. **Cross-check:**
   - did:key → assert `did == Did::from_ed25519_public_key(pubkey)`
   - did:web → fetch the DID's `did.json`, assert `pubkey` appears in
     `verificationMethod` with `assertionMethod` capability.
6. Mismatch → refuse to start with explicit error
   ("`<handle>/did` does not derive from `<handle>/key`").
7. Multi-passport future: N matches; consult `~/.secretariat/current`
   (optional pointer for default-persona UX) or `--as <handle>` CLI
   override. Pointer is never load-bearing for identity claims —
   forging requires the private key regardless.

Rationale: a text-file pointer ("which dir is the principal?") has the
same blast radius as the key itself (anyone with write access can swap
either), but the key-as-proof rule makes the security property
cryptographically explicit rather than convention-based.

### 3. Handle derivation by DID method

| DID method | Handle (= on-disk dir name) | Source |
|---|---|---|
| `did:web:DOMAIN` | `DOMAIN` (e.g. `rafa.equanimi.tech`) | derived from DID, no user prompt |
| `did:key:KEY` | `slug(profile.display_name)` (e.g. `rafa`) | from profile at `sec init` |

Rejected alternatives for did:key:
- Full multibase tail (`z6MkrgSFp29uMmpaB28LZx3W5RpwGFydA3LcyjjyHysSqRWa`)
  — visually noisy.
- Truncated prefix (`z6MkrgSFp29uMmpa`) — still looks like a key.
- Magic `me/` — special-cases self, breaks multi-passport.
- Asked-at-init alias picker — extra UX surface for no gain.

For did:key, the handle ↔ DID binding is recorded in
`<handle>/.identity` so the alias is auditable. Collisions
(two principals both choosing `rafa`) are principal-local — they pick
a different slug at init time.

### 4. Top-level role taxonomy

Three role classes share a single detection rule (`key` file presence).

| Top-level dir | Role | `key` file | `.identity` `role:` |
|---|---|---|---|
| The principal's own | passport | **yes** (0600) | `passport` |
| Another principal (peer subscription) | peer subscription | no | `peer-subscription` |
| An org (org subscription) | org subscription | no | `org-subscription` |

Contacts are NOT top-level dirs. They live in
`<passport>/contacts.json` (cross-passport contact independence).

### 5. Multi-passport future, single-passport now

The layout natively accommodates N passports — each is just another
top-level dir with its own `key`. v0.3 ships single-passport invariant
(detection asserts exactly one `key` match). Multi-passport additions
arrive later:
- `~/.secretariat/current` — text file with active passport handle
  (UX preference only, not identity claim)
- `sec switch <handle>` CLI
- `--as <handle>` per-command override

### 6. Workspaces (`.secretariat/` in repos) — deferred

Project-local channel-tree extensions via repo-committed
`.secretariat/` dirs are designed but deferred — see
`docs/ideas/2026-05-12-workspace-registry-and-repo-local-substrate.md`.
Slice 1 ships passport-home only. Two structural moves now to avoid
future refactor:
- Substrate root resolution takes a single root (passport home), but
  the API shape (e.g. `Substrate::resolve_channel(handle) -> Path`)
  presupposes resolution could span multiple roots later.
- Naming: `KeyPaths` → `Substrate` (or `SubstrateRoot`) — the type
  models the substrate, not just keys.

### 7. Customizable substrate root

Default: `~/.secretariat/`. Override:
- `SECRETARIAT_HOME` environment variable
- `--home <path>` CLI flag (where applicable)

Cheap to add; useful for encrypted volumes, test isolation, and the
future "work vs personal split" use case.

## Migration

No migration code. Existing v0.2.x installs (Rafa only) wipe
`~/.secretariat/` and re-run `sec init`. Pre-v1 product, single user,
small blast radius.

## Consequences

**Positive:**
- Identity is provable from disk state alone (key + did cross-check).
- One passport = one tar-able sovereignty unit.
- Top-level taxonomy is recognizable at a glance.
- Multi-passport falls out without retrofit.
- Substrate-uniform across principals (Marcelo/Christophe/Rafa look
  structurally identical).
- Workspaces (deferred) bolt on cleanly to this base.

**Negative:**
- Every existing `KeyPaths` caller updates (~15-20 sites).
- `sec init` rewritten end-to-end.
- v0.2.x users lose their existing inbox/outbox/queues unless they
  manually copy under the new passport dir.
- did:web cross-check requires a network fetch at startup (cached
  thereafter under `peers/`).

## Open items for future slices

- Workspaces (idea doc cross-linked).
- The `_meta/` resolved-context format (channel skills/agents/roster
  as signed envelopes).
- View-only role enforcement (roster says `subscribe` only → daemon
  refuses to sign+send drafts from `outbox/`).
- `current` pointer + `sec switch` UX for multi-passport.
- Per-channel `contract.md` consumption format.
