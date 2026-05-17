# Secretariat — architecture

This document describes the system as it exists today (v0.3 — channels,
orgs, MCP-primary). It is the orientation read for Claude and for any
developer landing on the repo. For the *why* behind v0.3, see
[`../ideas/2026-05-12-secretariat-as-autonomous-enterprise-substrate.md`](../ideas/2026-05-12-secretariat-as-autonomous-enterprise-substrate.md);
for the substrate layout decision, see
[`../decisions/2026-05-12-substrate-layout-v03.md`](../decisions/2026-05-12-substrate-layout-v03.md).

## What the system does

Secretariat is the operating substrate for an autonomous enterprise. AI
agents (with their own DIDs) draft continuously into shared channels;
the human principal selectively stamps the envelopes that count
(decisions, commitments, external comms, contracts). Everything is
markdown on the local filesystem, signed by its author, optionally
sealed to a recipient, optionally elevated by a Touch-ID-gated stamp.

Three trust layers, composed over two records (envelope + stamp):

1. **Signed envelope** — every envelope carries a detached ed25519
   signature from its author (human passport or agent DID). **Mandatory.**
   Drives provenance: *did this come from the claimed author?*
2. **Stamp** — Touch-ID attestation by the principal. **Selective.**
   Applied to envelopes the principal elects to elevate; the stamped
   subset is the org's authoritative ledger, the unstamped remainder
   is ambient context.
3. **Counter-stamp** — multi-principal stamp on the same envelope
   (process-verbaux model). **Reserved** for v0.4+; design space defined
   in the lexicon, no record type ships in v0.3.

Composition layered on top:

- **Org** = `did:web` document advertising channels + member roster.
- **Channel** = an append-only envelope log identified by
  `(owner_did, handle)` where `handle` ∈ `inbox:*` | `area:*` | `channel:*`.
  Owner's relay is the canonical sequencer; subscribers read that
  sequence.
- **Contract** = roster + cadence + trust gate, accumulating along the
  channel tree (org-root → ancestors → leaf) the way `CLAUDE.md` walks
  up from a working directory.

## Repository layout

```
secretariat/
├── Cargo.toml                    workspace root (5 crates)
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
│   ├── daemon/                   `sec-daemon` background service
│   └── relay/                    `sec-relay` owner-as-sequencer server
├── src-tauri/                    Tauri shell (tray + sidecar wiring)
├── tools/touchid-prompt/         Swift biometric helper
├── lexicons/                     AT-proto-shaped record schemas (truth)
└── docs/
    ├── developer/                ← you are here
    ├── decisions/                ADRs (v0.3 substrate layout, etc.)
    ├── ideas/                    raw captures (incl. the v0.3 pivot)
    ├── pitches/                  Shape Up pitches for in-flight work
    └── milestones/               historical milestones
```

## Layer dependencies (DDD)

```
crates/cli                  ──▶ application + infrastructure
crates/mcp                  ──▶ application + infrastructure
crates/daemon               ──▶ application + infrastructure
crates/relay                ──▶ (independent — HTTP service, no domain coupling)
src-tauri                   ──▶ (sidecar wiring only; no direct core dep)
crates/core::application    ──▶ ports + domain
crates/core::infrastructure ──▶ ports + domain
crates/core::ports          ──▶ domain
crates/core::domain         ──▶ codec (multibase only)
```

**Hard rule:** the domain layer cannot use `std::fs`, `reqwest`,
`chrono::Utc::now()`, or any IO/clock. Time and randomness enter via
parameters or ports. The architectural guardrail keeps domain testable
as pure logic and makes illegal states unrepresentable at construction.

## Filesystem layout (passport-rooted, v0.3)

Each principal-controlled identity is a **passport** — a self-contained
subtree under `~/.secretariat/` (override via `SECRETARIAT_HOME`).
Possession of the private key is the identity proof; no sidecar pointer
file.

```
~/.secretariat/
├── <passport-handle>/              # `did:web:DOMAIN` → DOMAIN; `did:key` → slug(display_name)
│   ├── .identity                   # role: passport, canonical DID, handle binding
│   ├── key                         # ed25519 PKCS#8, mode 0600 — THE proof
│   ├── did                         # cross-checked against key on startup
│   ├── profile.json
│   ├── template.md                 # global envelope template
│   ├── contacts.json
│   ├── cognition.json
│   ├── CLAUDE.md
│   ├── .claude/{skills,agents,commands}/
│   ├── queues/                     # flat captures: inbox:*, area:*
│   └── channel/                    # channel-tree: channel:*
│       └── <segs>/
│           ├── contract.local.md   # per-subscriber consumption contract
│           ├── envelopes/YYYY/MM/DD/<ts>-<hash>.md
│           └── outbox/             # drafts watched by the daemon
│
├── <org-or-peer-handle>/           # subscriptions — NO `key` file
│   ├── .identity                   # role: org-subscription | peer-subscription
│   ├── CLAUDE.md                   # context from owner's _meta
│   ├── .claude/                    # skills/agents from owner's _meta
│   └── channel/<segs>/
│       ├── contract.local.md
│       ├── envelopes/...
│       ├── outbox/
│       ├── _meta/                  # governance: roster, channel-wide policy
│       └── _ciphertext/            # sealed bodies awaiting decryption
│
├── peers/                          # global did-doc cache
└── bin/                            # helper binaries (touchid-prompt, etc.)
```

The channel directory IS a Claude Code project. `cd <channel-dir> && claude`
activates `CLAUDE.md`, `.claude/skills/`, the envelope history, the
consumption contract, and the outbox — same directory powers interactive
sessions and headless agents launched by the daemon via the Claude
Agent SDK.

**Two-tier file model in transit-bearing channels:** envelopes arrive
sealed under `_ciphertext/`; the daemon decrypts to `envelopes/` for
agent + grep access. The principal sees plaintext markdown; transports
only ever saw signed ciphertext.

## Domain (pure business logic)

`crates/core/src/domain/`

### Value objects (newtypes, parse-time validation)

- **`Did`** — `did:web:<host>[:<path>]` or `did:key:z<multibase>`.
  `parse`, `from_ed25519_public_key`, `web_document_url`,
  `embedded_ed25519_key`.
- **`DocHash`** — sha256 over canonical body. Serializes `sha256:<hex>`.
- **`Signature`** — detached ed25519. Serializes `ed25519:<base64>`.
- **`StampAct`** — `Attest | Defer | Vouch | Dispute | Redirect`. Only
  `Attest` ships in v0.3; others reserved in the lexicon.
- **`EnvelopeDepth`** — `Gross | Subtle`.
- **`EnvelopeUrgency`** — `Now | Soon | Whenever`.
- **`QueueHandle`** — `inbox:<seg>[:<seg>...]` |
  `area:<seg>[:<seg>...]` | `channel:<seg>[:<seg>...]`. Three sibling
  namespaces in the same grammar (depth = colon count).
- **`OrgAlias`** — kebab-case on-disk handle for an org subscription.
- **`TrustGate`** — `Signed | Stamped | CounterStamped` (minimum
  receiver requirement).

### Entities

- **`Stamp`** — signed human act. Immutable. Lexicon
  `tech.equanimi.secretariat.stamp`.
- **`Envelope`** — author's bid: from / to / handle / depth / urgency /
  `reply_to?: DocHash` (threading). Lexicon
  `tech.equanimi.secretariat.envelope`. The `(to, handle)` pair factors
  the queue URI `did:web:themia.pro#channel:dommage-corporel:paris-cohort`
  — one identity, many queues.
- **`Recipient`** — `Peer { did }` | `LocalQueue { handle }`. Routing
  discriminator; doesn't travel on-wire (recovered from envelope).
- **`Org`** — DID-rooted namespace; cached projection of the org's
  `did:web` DID document plus its advertised channels.
- **`ChannelDef`** — per-channel governance (display name, description,
  default contract knobs). Lexicon
  `tech.equanimi.secretariat.channelDef`.
- **`ChannelContract`** — receiver-side consumption contract: cadence
  floor, `min_trust: TrustGate`, notify policy, filters. Accumulates
  down the channel tree (org-root → ancestors → leaf); roster = UNION,
  cadence_floor / trust_gate = MAX-RESTRICTIVE. Lives in
  `contract.local.md` — `.local` is load-bearing (private, never sent,
  never shared).
- **`Contact`** — principal-local roster entry (DID + display + notes).

### Aggregate

- **`AttestedDocument`** — `Option<Envelope>`, `Stamp`, `body: String`.
  Construction enforces the invariant
  `stamp.doc_hash == canonical_body_hash(body)`. Signature verification
  is *not* in the aggregate — it requires IO (DID resolution) and is
  composed in the application layer.

### Pure helpers

- **`canonical_body_hash(body) -> DocHash`** — strip leading BOM,
  normalize CRLF→LF, strip trailing whitespace; preserve leading
  whitespace; SHA-256 over UTF-8.

## Ports (traits)

`crates/core/src/ports/`

- **`Signer`** — `signer_did()`, `sign(doc_hash, reason) -> Signature`.
  Implementations gate signing on a humanness check (biometric).
- **`DidResolver`** — `resolve(did) -> ResolvedDid` returning ed25519
  keys; implementations may cache.
- **`CognitionPort`** — `complete(messages, tools) -> Completion`. The
  agent loop talks to this, not a vendor SDK. Adapters: Claude Code
  (subscription), OpenAI-compatible (BYOK), local (deferred). Sovereignty
  over cognition parallels sovereignty over keys.

## Infrastructure (concrete adapters)

`crates/core/src/infrastructure/`

### Signing

- **`Ed25519Signer<B: BiometricGate>`** — signing key + pluggable
  biometric gate. Gate has no access to the key; it returns "verified
  yes/no". Signing happens in Rust *after* the gate returns success.
- **`BiometricGate`** trait. Real impl: **`TouchIdGate`** shells out to
  `tools/touchid-prompt/` (Swift). Test impls: `AlwaysAllowGate`,
  `AlwaysDenyGate`.

### Resolution

- **`DidWebResolver`** — HTTPS fetch of `did.json`, cached at
  `peers/<sanitized-did>.json`. Trust-on-first-use; no TTL in MVP.
- **`DidKeyResolver`** — pure function over the embedded key.
- **`CompositeDidResolver`** — dispatches by `did.method()`.

### Crypto (`crypto/`)

- **`sealed`** — sealed-box body encryption: ed25519 → x25519
  conversion + XChaCha20-Poly1305 to recipient's DID-derived key.
  Transports see signed ciphertext, never plaintext.

### Persistence (`*_store.rs`)

- **`Substrate`** (formerly `KeyPaths`) — substrate root resolution;
  ready for multi-passport API even though v0.3 enforces single.
- **`ProfileStore`, `ContactStore`, `OrgStore`, `ChannelDefStore`,
  `ContractStore`, `QueueDir`** — filesystem-backed stores. Each is the
  read+write boundary for one aggregate.
- Keys: PKCS#8 PEM, mode `0600`, refuse to overwrite.

### Transport (`transport/`)

- **`relay`** — HTTP client against `sec-relay`. Owner-as-sequencer per
  channel; subscribers poll (humans, ≥15-min floor — anti-compulsion)
  or push-subscribe (agents, sub-second).

### Cognition (`cognition/`)

- **`claude`** — Claude Code adapter (uses the user's existing
  subscription).
- **`openai_compat`** — generic adapter for OpenAI-compatible endpoints
  (Anthropic API, Ollama, etc.).
- **`ledger`** — per-conversation transcript persistence.

### Markdown

- **`parse_document` / `embed_stamp`** — YAML frontmatter handling.
  `parse → embed → parse` round-trips byte-for-byte on body.

### Codec (`crates/core/src/codec.rs`)

- **`encode_ed25519_multibase` / `decode_ed25519_multibase`** —
  z-prefixed base58btc with `ed25519-pub` multicodec.

## Application (use cases)

`crates/core/src/application/` — every principal-facing primitive
ships parallel use case + CLI command + MCP tool.

| Use case | What it does |
|---|---|
| `compose_envelope` | Read template, prepend `$envelope`, write to recipient's outbox |
| `stamp_document` | Hash + sign + embed; refuses re-stamp unless `force` |
| `verify_document` | Returns `Verified / Tampered / Unsigned / SignerUnresolvable / SignatureInvalid` |
| `send_envelope` | Seal body to recipient, hand to relay client |
| `capture_ops` | `capture(queue, body)` → write to `<passport>/queues/<handle>/...` |
| `contextify_capture` | Enrich raw capture with org/channel context for review |
| `inbox_ops`, `inbox_actions` | Read / archive / route inbound envelopes |
| `review_queue` | Cross-channel walker — collates inbox + outbox drafts + captures |
| `channels_ops` | Create / list / delete channels under a passport-owned org |
| `org_ops` | Create / list / delete orgs (did:web-rooted) |
| `contract_ops` | Get / set consumption contracts; resolver accumulates org-root → leaf |
| `contact_ops` | Add / list / remove contacts (passport-local roster) |
| `invite_ops` | Create + claim bilateral correspondence invites |
| `process_correspondence_claims` | Daemon-side: process accepted invites, install peer subscription |
| `delivery_policy` | Resolve effective contract to decide queue-vs-surface |
| `sync` | Pull from relay, decrypt `_ciphertext/` → `envelopes/`, write index |

## CLI (`sec`)

`crates/cli/src/`

```
sec init [--did did:web:<host>[:<path>]]
sec compose --to <did> [--handle <queue-handle>] [--depth ...] [--urgency ...]
sec capture --queue <handle> [--body <text>]
sec channels {create | list | delete | get-contract | set-contract | resolve-contract}
sec orgs {create | list | delete | get-contract | set-contract}
sec stamp <file> [--act attest] [--force] [--allow-test-biometrics]
sec verify <file> [--json]
sec list {inbox | outbox | peers}
sec contact {add | list | remove}
sec read <envelope>
sec invite {create | accept}
sec daemon {install | uninstall | status | tick}
sec mcp install
sec profile {get | set}
```

Exit codes: `0` ok, `1` generic error, `2` verify failed / already
stamped, `3` biometric refused.

Env vars: `SECRETARIAT_HOME`, `SECRETARIAT_TOUCHID_BINARY`,
`SECRETARIAT_BIOMETRIC` (`touchid` | `always_allow` | `always_deny` —
non-touchid honored only in debug builds or with
`--allow-test-biometrics`).

## MCP (`sec-mcp`)

`crates/mcp/src/server.rs` exposes tools via `rmcp` `#[tool(...)]`
attributes. Same parameter shapes as the CLI flags, same return shapes
as the use cases' output structs.

**Tools:** `compose`, `capture`, `list_channels`, `read_channel`,
`stamp`, `archive`, `read`, `verify`, `invite`, `accept_invite`,
`create_org`, `list_orgs`, `delete_org`, `create_channel`,
`delete_channel`, `get_channel_contract`, `set_channel_contract`,
`resolve_channel_contract`, `get_org_contract`, `set_org_contract`,
`daemon_tick`, `daemon_status`.

**Prompts:** `idea`, `pain`, `review`, `compose`, `onboard`, `stamp`.

MCP is the **primary interface** per
[`memory/project_mcp_is_primary_interface.md`](../../CLAUDE.md). UI
navigates; MCP handles all CRUD.

## Daemon (`sec-daemon`)

`crates/daemon/src/` — local nervous system, installed as a macOS
LaunchAgent (`sec daemon install`).

Subsystems:

- **`outbox_watcher`** — watches `<passport>/.../<channel>/outbox/`,
  surfaces drafts for review and (on stamp) hands to relay client.
- **`serve`** — main loop; ticks scheduled subscribers, processes
  inbound from relays.
- **`relay_register`** — registers the passport's own owned channels
  with its relay.
- **`ipc`** — local socket for `sec daemon tick` / `status`.

Push for agents (sub-second), poll for humans (≥15-min floor). The
anti-compulsion floor is a property of the human subscription, not the
substrate — agents on the same channel get push.

## Relay (`sec-relay`)

`crates/relay/src/` — minimal HTTP service implementing
owner-as-sequencer for a single passport's channels. **Independent
crate; no domain coupling.**

Routes (`routes/`):

- `POST /channels/:handle/envelopes` — append (auth: passport's own key)
- `GET /channels/:handle/envelopes?cursor=` — read sequence
- `POST /channels/:handle/subscribe` — agent push subscription
- Invite endpoints.

Per-channel strong consistency emerges from the owner's relay. Cross-
channel global ordering is explicitly NOT provided — channels are
independent logs; cross-channel causality is expressed via envelope-
hash references in `reply_to`.

Self-hosted (the passport's own infrastructure) or run locally for
single-user setups. No central broker, registry, or marketplace.

## Wire format

Stamped envelope = markdown with YAML frontmatter:

```markdown
---
$envelope:
  $type: tech.equanimi.secretariat.envelope
  from: did:key:z6Mk... | did:web:rafa.equanimi.tech
  to: did:web:themia.pro                    # owner DID of the queue
  handle: channel:dommage-corporel:paris    # queue handle under that owner
  depth: gross | subtle
  urgency: now | soon | whenever
  reply_to: sha256:<hex>                    # optional — threading
  source: <free-form>
$attestation:                               # absent if envelope is signed-only
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

Queue URI assembled: `<to>#<handle>` →
`did:web:themia.pro#channel:dommage-corporel:paris`. W3C DID URL
fragment semantics ("sub-resource of identity"); wire format keeps
`(to, handle)` factored for efficient routing.

**Hashing rules** — strip leading BOM, normalize CRLF→LF, strip
trailing whitespace, preserve leading whitespace, SHA-256 over UTF-8.
The hash covers the **body only**; envelope frontmatter is routing
metadata.

**Body encryption** — sealed-box to recipient's x25519 (from ed25519);
transports see signed ciphertext. Bodies under `_ciphertext/` until
the daemon decrypts to `envelopes/`. Decrypted plaintext never leaves
the device.

## Three-layer trust model in code

`sec verify --json` returns:

```json
{
  "signature": "ok | invalid | unresolvable",
  "stamp": "none | ok | invalid",
  "counter_stamps": []
}
```

Recipient policy decides what's required. An unstamped-but-signed
envelope is *informational* (the author wrote this); a stamped envelope
is *authoritative* (the principal vouches). UI surfaces this
distinction; agents acting on received envelopes MUST treat
signed-only ≠ stamped.

Counter-stamps are designed in the lexicon but no record type ships in
v0.3 — deferred until concrete driver (Themia's annual `assemblee_generale`
process-verbaux).

## Threat model

### Defended

- **AI forging a stamp** — biometric gate blocks; signing happens in
  Rust after gate success, key never reachable from AI surface.
- **Tampered body** — hash invariant breaks; aggregate refuses to
  construct.
- **Impersonator's stamp** — signer's DID document is authoritative
  (`did:web` over HTTPS or `did:key` self-proving).
- **Transport leak of content** — bodies sealed end-to-end before they
  reach the wire.

### Acknowledged (not defended)

- **Compromise of the principal's machine** — sudo + FS access
  exfiltrates the key. Mitigation later: Secure Enclave via WebAuthn.
- **Metadata leakage to transports** — Gmail/Slack see
  who-to-whom-and-when. Acknowledged in invariant #4; bilateral
  contracts may negotiate stronger transports for steady state.
- **DNS hijack of `did:web` host** — mitigated by HTTPS + first-fetch
  cache, but not absent attack-on-first-use.
- **Coercion** — biometric verifies presence, not free will.
- **Replay** — same body → same hash → same signature. Intentional.
  Envelope `source` carries app-level dedupe IDs.

## Architectural invariants (recap)

These are properties of the *system*, not rules of *behavior*. See
[`../../AGENTS.md`](../../AGENTS.md) for the full list. Summary:

1. No central server. 2. No telemetry. 3. Keys never leave device.
4. Transports are adapters, not authorities. 5. Cognition is pluggable.
6. Correspondence is bilateral or multi-party; always local.
7. No SaaS distribution. 8. Filesystem authoritative; channel dir is
the activation surface. 9. Owner-as-sequencer per channel; cross-
channel order not provided.

## What's not built yet

| Component | Trigger |
|---|---|
| Counter-stamp record + multi-party stamping ceremony | Themia `assemblee_generale` driver — v0.4 |
| Attention routing daemon (compose from `depth`/`urgency`/contract) | 2–3 weeks of real channel traffic — v0.4 |
| SQLite read-cache for cross-channel queries | When query latency demands — v0.4+ |
| Multi-passport same-device sync (key migration UX) | v0.4 wedge |
| Channel ownership transfer (`rosterUpdate.op = transfer_ownership`) | Concrete driver |
| Lexicon publication | After self-use stabilizes the schema |
| Windows support | When Christophe's brief workflow needs it |
| `defer` / `vouch` / `dispute` / `redirect` stamp acts | As cadence + multi-party land |
| Webhook adapter for external sources | DID-keyed external services — v0.4 wedge |

See [`../milestones/`](../milestones/) for the historical sequence and
the v0.3 substrate decision in [`../decisions/2026-05-12-substrate-layout-v03.md`](../decisions/2026-05-12-substrate-layout-v03.md).
