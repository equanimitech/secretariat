# Secretariat — architecture

This document describes the system as it exists today, after the **v0.12.0
git-native teardown** (see [`../../CHANGELOG.md`](../../CHANGELOG.md)). The
correspondence apparatus — federation, channels, orgs, contracts, compose,
capture, invite, the review queue — was cut. What remains is a **markdown
editor** over a plain filesystem tree plus the **Signet stamp / verify / read
core**, with `sec launch` opening a cognition CLI in a repo-bound `cwd`.

For the _why_ behind the cut, see
[`../ideas/2026-05-31-git-native-substrate.md`](../ideas/2026-05-31-git-native-substrate.md)
and the bounded-context decision
[`../decisions/2026-06-01-signet-secretariat-bounded-context-boundary.md`](../decisions/2026-06-01-signet-secretariat-bounded-context-boundary.md).
Pre-teardown docs describing channels/orgs/relay are **historical records**,
not current orientation.

## What the system does

A human opens a markdown document, reads it, and **seals it** with a
biometric-gated signature. AI (Claude, via MCP or `sec launch`) reads, drafts,
and _proposes_ a stamp; the principal is the only one who can stamp. The seal
is an `$attestation` block embedded in the document's frontmatter, verifiable
by anyone with no server in the middle.

Two verifiable layers, embedded in the document:

1. **Signature** — a detached ed25519 signature keyed to the author's DID
   (human principal or authorized agent). Proves _did this come from the
   claimed author?_
2. **Stamp** — Touch-ID attestation by the principal. **Selective.** Applied
   to documents the principal elects to elevate; the stamped subset is the
   authoritative record, the unstamped remainder is ambient context.
3. **Counter-stamp** — multi-principal stamp on the same document. **Reserved.**
   Design space defined in the lexicon; no record type ships yet.

The substrate is **git repositories**: documents are markdown files under a
repo's `docs/` (or anywhere); the identity + signing key live under
`~/.secretariat/`. There is no queue tree, no daemon poll loop, no relay.

## Repository layout

```
secretariat/
├── Cargo.toml                    workspace root
├── crates/
│   ├── core/                     library — all business logic
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── codec.rs          multibase / base32 helpers
│   │       ├── domain/           pure logic, no IO
│   │       ├── ports/            traits the domain depends on
│   │       ├── infrastructure/   concrete adapters
│   │       └── application/      use cases (orchestration)
│   ├── cli/                      `sec` binary
│   ├── mcp/                      `sec-mcp` MCP server (rmcp)
│   ├── daemon/                   `sec-daemon` — macOS LaunchAgent surface only
│   └── cognition-claude-sdk/     TS/Bun package — Claude Agent SDK bridge (bun-compiled to a sidecar binary; NOT a Cargo crate)
├── src-tauri/                    Tauri shell (markdown editor + tray + sidecar wiring)
├── tools/touchid-prompt/         Swift biometric helper
├── lexicons/                     AT-proto-shaped record schemas (truth)
└── docs/
    ├── developer/                ← you are here
    ├── decisions/                ADRs
    ├── ideas/                    raw captures
    ├── pitches/                  Shape Up pitches (many pre-teardown = historical)
    └── milestones/               historical milestones
```

The `relay/` crate was deleted in the teardown. The Cargo workspace is now
five members: `crates/core`, `crates/cli`, `crates/mcp`, `crates/daemon`, and
`src-tauri`. `crates/cognition-claude-sdk` is **not** a Cargo crate — it's a
TS/Bun package (`@anthropic-ai/claude-agent-sdk` wrapper) that `bun build
--compile`s into a sidecar binary, staged alongside `sec` / `sec-mcp` by
`src-tauri/scripts/build-sidecars.sh`.

## Layer dependencies (DDD)

```
crates/cli                  ──▶ application + infrastructure
crates/mcp                  ──▶ application + infrastructure
crates/daemon               ──▶ (LaunchAgent install/uninstall/status + keepalive serve)
src-tauri                   ──▶ (sidecar wiring + editor UI; no direct core dep)
crates/core::application    ──▶ ports + domain
crates/core::infrastructure ──▶ ports + domain
crates/core::ports          ──▶ domain
crates/core::domain         ──▶ codec (multibase only)
```

**Hard rule:** the domain layer cannot use `std::fs`, `reqwest`,
`chrono::Utc::now()`, or any IO/clock. Time and randomness enter via
parameters or ports. The guardrail keeps domain testable as pure logic and
makes illegal states unrepresentable at construction.

## Filesystem layout

Two roots:

**1. Identity home — `~/.secretariat/`** (override via `SECRETARIAT_HOME`).
Holds the principal's key + identity, written by `sec init`. Possession of the
private key is the identity proof.

```
~/.secretariat/
├── identity.md                 # canonical DID, display name, authorized_agents
├── key                         # ed25519 PKCS#8, mode 0600 — THE proof
├── profile.json                # display name (presence)
├── cognition.toml / preferences.toml   # cognition launcher config
└── bin/                        # helper binaries (touchid-prompt, etc.)
```

**2. Document substrate — git repositories.** Documents are markdown files
with `$envelope` / `$attestation` frontmatter, living anywhere — typically a
repo's `docs/`. `sec stamp` / `sec verify` / `sec read` operate on any file
path; `/review-repos` walks a repo's doc surface by stamp state. There is no
canonical `~/.secretariat/` document tree anymore — the repo _is_ the
substrate (architectural invariant #8).

`sec launch` resolves a repo path (optionally via a `root_path` override) and
opens the configured cognition CLI there, so `cd <repo> && claude` activates
`CLAUDE.md`, `.claude/skills/`, and the doc history for free.

## Domain (pure business logic)

`crates/core/src/domain/` — the Signet trust core. Survives the teardown
intact.

### Value objects (newtypes, parse-time validation)

- **`Did`** — `did:web:<host>[:<path>]` or `did:key:z<multibase>`. `parse`,
  `from_ed25519_public_key`, `web_document_url`, `embedded_ed25519_key`.
- **`DocHash`** — sha256 over canonical body. Serializes `sha256:<hex>`.
- **`Signature`** — detached ed25519. Serializes `ed25519:<base64>`.
- **`StampAct`** — `Attest | Defer | Vouch | Dispute | Redirect`. Only
  `Attest` ships today; others reserved in the lexicon.

The `$envelope` frontmatter is parsed **opaquely** post-teardown — the crypto
core reads/embeds attestations without depending on the channel `Envelope`
shape. Legacy queue/channel/org value objects (`QueueHandle`, `OrgAlias`,
`TrustGate`, `EnvelopeDepth/Urgency`) were cut as live types.

### Aggregate

- **`AttestedDocument`** — `Stamp` + `body: String` (the envelope coupling was
  severed in teardown phase P1). Construction enforces the invariant
  `stamp.doc_hash == canonical_body_hash(body)`. Signature verification is
  _not_ in the aggregate — it requires IO (DID resolution) and is composed in
  the application layer.

### Pure helpers

- **`canonical_body_hash(body) -> DocHash`** — strip leading BOM, normalize
  CRLF→LF, strip trailing whitespace; preserve leading whitespace; SHA-256
  over UTF-8.

## Ports (traits)

`crates/core/src/ports/`

- **`Signer`** — `signer_did()`, `sign(doc_hash, reason) -> Signature`.
  Implementations gate signing on a humanness check (biometric).
- **`DidResolver`** — `resolve(did) -> ResolvedDid` returning ed25519 keys;
  implementations may cache.
- **`CognitionPort`** — `complete(messages, tools) -> Completion`. The agent
  loop talks to this, not a vendor SDK. Sovereignty over cognition parallels
  sovereignty over keys.

## Infrastructure (concrete adapters)

`crates/core/src/infrastructure/`

### Signing

- **`Ed25519Signer<B: BiometricGate>`** — signing key + pluggable biometric
  gate. The gate has no access to the key; it returns "verified yes/no".
  Signing happens in Rust _after_ the gate returns success.
- **`BiometricGate`** trait. Real impl: **`TouchIdGate`** shells out to
  `tools/touchid-prompt/` (Swift). Test impls: `AlwaysAllowGate`,
  `AlwaysDenyGate`.

### Resolution

- **`DidWebResolver`** — HTTPS fetch of `did.json`, cached as a peer doc.
  Trust-on-first-use; no TTL in MVP.
- **`DidKeyResolver`** — pure function over the embedded key.
- **`CompositeDidResolver`** — dispatches by `did.method()`.

### Markdown

- **`parse_document` / `embed_stamp`** — YAML frontmatter handling.
  `parse → embed → parse` round-trips byte-for-byte on body. Embeds the
  `$attestation` block in place.

### Persistence (`*_store.rs`)

- **`identity_store`** — read/write `identity.md` (DID, display name,
  `authorized_agents`).
- **`channel_def_store` / `binding_store` / `contract_store` / `org_store`** —
  retained **only as keepers** backing `sec launch` (channel binding +
  `root_path` resolution). They are not user-facing features post-teardown.
- Keys: PKCS#8 PEM, mode `0600`, refuse to overwrite.

### Codec (`crates/core/src/codec.rs`)

- **`encode_ed25519_multibase` / `decode_ed25519_multibase`** — z-prefixed
  base58btc with the `ed25519-pub` multicodec.

### Cognition (`crates/cognition-claude-sdk/` — TS/Bun sidecar)

- A private TS/Bun package wrapping `@anthropic-ai/claude-agent-sdk`,
  `bun build --compile`d into a sidecar binary. Drives a headless cognition
  session for `sec launch` / background work; the Rust side talks to it through
  the `CognitionPort`. Other substrates (Anthropic API, local models) wire
  through the same port per invariant #5.

## Application (use cases)

`crates/core/src/application/` — post-teardown the surviving use cases are:

| Use case           | What it does                                                                            |
| ------------------ | --------------------------------------------------------------------------------------- |
| `stamp_document`   | Hash + sign + embed `$attestation`; refuses re-stamp unless `force`                     |
| `verify_document`  | Layered result `{signature, stamp, counter_stamps}` against the signer's DID doc        |
| `read`/`inbox_ops` | Read + decrypt the body of a document (the read/decrypt path; list/draft cut)           |
| `launch_channel`   | Resolve a repo-bound `cwd` (via keeper stores + `root_path`) and exec the cognition CLI |
| `agent_ops`        | Add / list / remove / rotate authorized agents in `identity.md`                         |

Cut in teardown: `compose_envelope`, `send_envelope`, `capture_ops`,
`contextify_capture`, `review_queue`, `inbox_actions`, `channel_def_envelope`,
`contract_ops`, `invite_ops`, `accept_org_membership`,
`process_correspondence_claims`, `delivery_policy`, `sync`, `federation`.

## CLI (`sec`)

`crates/cli/src/`

```
sec init [--did did:web:<host>[:<path>]]   # generate key, write ~/.secretariat/*
sec agent {add | list | remove | rotate}   # manage authorized agents (scribes)
sec stamp <file> [--act attest] [--force] [--allow-test-biometrics]
sec verify <file> [--json]
sec read <file>                            # decrypt + print body
sec launch <handle> [--org <alias>]        # open cognition CLI in a channel-bound cwd (root_path → repo)
sec mcp install                            # wire sec-mcp into Claude Desktop / Code
sec daemon {install | uninstall | status}  # macOS LaunchAgent (keepalive serve)
sec profile {get | set}                    # display name (presence)
sec view <file>                            # open a markdown file in the desktop app
```

Cut subcommands: `compose`, `capture`, `channels`, `orgs`, `invite`, `list`,
and `daemon tick` / `daemon register`.

Exit codes: `0` ok, `1` generic error, `2` verify failed / already stamped,
`3` biometric refused.

Env vars: `SECRETARIAT_HOME`, `SECRETARIAT_TOUCHID_BINARY`,
`SECRETARIAT_BIOMETRIC` (`touchid` | `always_allow` | `always_deny` —
non-touchid honored only in debug builds or with `--allow-test-biometrics`).

## MCP (`sec-mcp`)

`crates/mcp/src/server.rs` exposes tools via `rmcp` `#[tool(...)]` attributes.

**Tools (the complete current set):** `stamp`, `read`, `verify`, `agent_add`,
`agent_list`, `agent_remove`, `agent_rotate`.

The server's `instructions` string carries the **stamp ceremony**: before
`stamp`, the client must `read` the file, render the full body verbatim,
obtain explicit same-turn consent, then call `stamp` (Touch ID gates
regardless). Claude proposes; the principal stamps. Cut in teardown:
`compose`, `capture`, `list_channels`, `read_channel`, `archive`,
`create_channel`/`delete_channel`, `create_org`/`list_orgs`/`delete_org`,
`invite`/`accept_invite`, all `*_contract` tools, `daemon_tick`/`daemon_status`,
and the `orgs` / `compositions` resources.

## Daemon (`sec-daemon`)

`crates/daemon/src/` — reduced in teardown to the **macOS LaunchAgent
surface**: `install` / `uninstall` / `status` plus a minimal keepalive `serve`
that the installed plist targets. The poll/send loop, `outbox_watcher`,
`relay_register`, and the IPC `TICK` path were all deleted. There is no
federation, no inbound-from-relay, no scheduled subscriber tick.

## Wire format

A stamped document = markdown with YAML frontmatter:

```markdown
---
$envelope: # parsed opaquely by the crypto core — routing metadata only
  $type: tech.equanimi.secretariat.envelope
  from: did:key:z6Mk... | did:web:rafa.equanimi.tech
  ...
$attestation: # absent if the document is signed-only / unstamped
  $type: tech.equanimi.secretariat.stamp
  signer: <did>
  act: attest
  docHash: sha256:<hex>
  stampedAt: 2026-05-13T16:01:35.220898Z
  signature: ed25519:<base64-of-64-bytes>
---

# Body

...
```

**Hashing rules** — strip leading BOM, normalize CRLF→LF, strip trailing
whitespace, preserve leading whitespace, SHA-256 over UTF-8. The hash covers
the **body only**; frontmatter is metadata. Editing the body after stamping
breaks the hash and `verify` reports `tampered`.

> Note: the stamp hash preimage (body-only vs frontmatter+body) and the
> record shape (`$attestation` object vs a `$signatures` array) are an active
> point of divergence between `sec`'s stamp core and the standalone Signet
> crate — see the bounded-context decision. Treat the above as `sec`'s current
> on-wire shape.

## Three-layer trust model in code

`sec verify --json` returns:

```json
{
  "signature": "ok | invalid | unresolvable | none | verifiedAgent | okUnverifiedAgent",
  "stamp": "none | verified | tampered",
  "counter_stamps": []
}
```

Recipient policy decides what's required. An unstamped-but-signed document is
_informational_ (the author wrote this); a stamped document is _authoritative_
(the principal vouches). Agents acting on a received document MUST treat
signed-only ≠ stamped.

Counter-stamps are designed in the lexicon but no record type ships yet —
deferred until a concrete driver.

## Threat model

### Defended

- **AI forging a stamp** — biometric gate blocks; signing happens in Rust
  after gate success, key never reachable from the AI surface.
- **Tampered body** — hash invariant breaks; aggregate refuses to construct;
  `verify` reports `tampered`.
- **Impersonator's stamp** — the signer's DID document is authoritative
  (`did:web` over HTTPS or `did:key` self-proving).

### Acknowledged (not defended)

- **Compromise of the principal's machine** — sudo + FS access exfiltrates the
  key. Mitigation later: Secure Enclave via WebAuthn.
- **DNS hijack of `did:web` host** — mitigated by HTTPS + first-fetch cache,
  but not absent attack-on-first-use.
- **Coercion** — biometric verifies presence, not free will.
- **Replay** — same body → same hash → same signature. Intentional.

## Architectural invariants (recap)

These are properties of the _system_, not rules of _behavior_. See
[`../../AGENTS.md`](../../AGENTS.md) for the full list. Summary:

1. No central server. 2. No telemetry. 3. Keys never leave device.
2. Cognition is pluggable. 5. Filesystem is authoritative; the git repo is the
   substrate. 6. No SaaS distribution.

(The pre-teardown invariants about transports-as-adapters, bilateral/
multi-party correspondence, and owner-as-sequencer channels lapsed with the
correspondence apparatus; they survive only as historical record.)

## What's not built yet

| Component                                            | Trigger                              |
| ---------------------------------------------------- | ------------------------------------ |
| Counter-stamp record + multi-party stamping ceremony | A concrete multi-principal driver    |
| Stamp-chain (each stamp signing the previous hash)   | When an audit trail demands it       |
| Signet-crate convergence (one stamp core)            | CI-gated on seal continuity          |
| Lexicon publication                                  | After self-use stabilizes the schema |
| Windows support                                      | When Christophe's workflow needs it  |
| `defer` / `vouch` / `dispute` / `redirect` acts      | As multi-party lands                 |

See [`../milestones/`](../milestones/) for the historical sequence.
