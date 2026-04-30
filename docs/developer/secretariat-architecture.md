# Secretariat — architecture

This document describes the system as it exists today (Day 1 — `0559dce`).
For aspirational design, see the pitch under `equanimitech/docs/pitches/`.

## What the system does

A principal (the human) instructs Claude (the scribe) to draft a document.
Claude writes a markdown file with AT-proto-shaped envelope frontmatter.
The principal opens the file, reads it, and runs `sec stamp <file>`.
The CLI prompts Touch ID; on success it computes a SHA-256 over the
canonical body, signs the hash with the principal's ed25519 key, and
embeds the signed `$attestation` block back into the file's frontmatter.

The file is now a self-contained attested artifact. Anyone with the
principal's public key (resolved from the DID) can verify it offline.

```
                                    ┌──────────────────┐
   AI scribe (Claude)               │   ~/.secretariat │
        │                           │                  │
        │ draft to outbox           │  outbox/         │
        ▼                           │   <recipient>/   │
   .md file with                    │     <utc>.md  ◀──┼── sec compose
   $envelope frontmatter            │                  │
        │                           │  inbox/          │
        │ principal opens, reads    │                  │
        ▼                           │  peers/          │
   ╔════════════════╗               │   <did>.json     │
   ║   sec stamp   ║                │                  │
   ║                ║               │  key             │
   ║  Touch ID  ────╫── biometric ──▶  did             │
   ║                ║   gate         │  template.md    │
   ║  ed25519 sign  ║                │  attention-     │
   ║                ║                │   envelope.md   │
   ╚════════════════╝                └──────────────────┘
        │
        ▼
   .md file with $envelope + $attestation
        │
        │ travels (Slack, email, iCloud, …)
        ▼
   ╔════════════════╗
   ║  sec verify   ║   ←   resolve signer's DID
   ╚════════════════╝       (did:web fetch + cache, or did:key decode)
        │
        ▼
   ✓ verified  /  ✗ tampered  /  ✗ unsigned  /  ✗ unresolvable  /  ✗ invalid sig
```

## Repository layout

```
secretariat/
├── Cargo.toml                    workspace root
├── crates/
│   ├── core/                     library
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── codec.rs          shared multibase encoding helpers
│   │       ├── domain/           pure business logic, no IO
│   │       ├── ports/            traits the domain depends on
│   │       ├── infrastructure/   concrete adapters
│   │       └── application/      use cases (orchestration)
│   └── cli/                      `sec` binary
│       ├── src/main.rs
│       ├── src/commands/
│       └── assets/               default ~/.secretariat/* contents
├── src-tauri/                    GUI shell (placeholder, Day 2+)
├── tools/touchid-prompt/         Swift biometric helper
└── docs/
    ├── developer/
    │   └── secretariat-architecture.md  ← you are here
    └── milestones/
        └── 2026-04-30-first-signed-message.md
```

## Layer dependencies (DDD)

```
crates/cli                  ──▶ application + infrastructure
src-tauri                   ──▶ application + infrastructure (when wired)
crates/core::application    ──▶ ports + domain
crates/core::infrastructure ──▶ ports + domain   (impls + types)
crates/core::ports          ──▶ domain           (trait inputs/outputs)
crates/core::codec          ──▶ (multibase only — pure function module)
crates/core::domain         ──▶ codec            (for did:key decoding only)
```

**The hard rule:** the domain layer cannot use `std::fs`, `reqwest`,
`chrono::Utc::now()`, or any IO/clock. Time and randomness enter via
parameters or ports. This is the architectural guardrail that keeps the
domain testable as pure logic.

## Domain (pure business logic)

`crates/core/src/domain/`

### Value objects (newtypes, parse-time validation)

- **`Did`** — wraps a DID string. Construction enforces `did:web:<host>[:<path>]`
  or `did:key:z<multibase>` shape. Once constructed, the value is well-formed.
  Methods: `parse`, `from_ed25519_public_key`, `as_str`, `method()` →
  `DidMethod::Web | Key`, `web_document_url()` (`Some` only for `did:web`),
  `embedded_ed25519_key()` (`Some` only for `did:key`).
- **`DocHash`** — 32-byte sha256 digest. Serializes as `sha256:<hex>`.
- **`Signature`** — 64-byte detached ed25519 signature. Serializes as
  `ed25519:<base64>`.
- **`StampAct`** — enum: `Attest | Defer | Vouch | Dispute | Redirect`.
  MVP only ships `Attest`; the rest are reserved.
- **`EnvelopeDepth`** — `Gross | Subtle`.
- **`EnvelopeUrgency`** — `Now | Soon | Whenever`.

### Entities

- **`Stamp`** — the signed human act. Once issued, immutable. Lexicon:
  `app.equanimi.secretariat.stamp`.
- **`Envelope`** — bid for the receiver's attention. Composed by the scribe.
  Lexicon: `app.equanimi.secretariat.envelope`.
- **`AttentionEnvelope`** — the principal's published bounds. Has an
  `admits(envelope) -> bool` predicate. Lexicon:
  `app.equanimi.secretariat.attentionEnvelope`.

### Aggregate

- **`AttestedDocument`** — root of the consistency boundary. Owns
  `Option<Envelope>`, `Stamp`, `body: String`. Construction enforces the
  hash invariant: `stamp.doc_hash == canonical_body_hash(body)`.
  Signature verification is **not** in the aggregate — that requires
  resolving the signer's DID, which is IO-bound. The application layer
  composes the two checks.

### Pure helpers

- **`canonical_body_hash(body: &str) -> DocHash`** — applies the
  canonicalization rules (strip BOM, normalize CRLF→LF, strip trailing
  whitespace; do NOT strip leading whitespace), then SHA-256.

## Ports (traits)

`crates/core/src/ports/mod.rs`

- **`Signer`** — `signer_did()` and `sign(doc_hash, reason) -> Signature`.
  Implementations gate signing behind a humanness check.
- **`DidResolver`** — `resolve(did) -> ResolvedDid` containing one or more
  ed25519 verifying keys. Implementations may cache.

## Infrastructure (concrete adapters)

`crates/core/src/infrastructure/`

### Signing

- **`Ed25519Signer<B: BiometricGate>`** — composes a signing key with a
  pluggable biometric gate. The gate has no access to the signing key; it
  only returns "verified yes/no." Signing happens in Rust *after* the gate
  returns success.
- **`BiometricGate` trait** — single `prompt(reason) -> Result<()>`.
- **`AlwaysAllowGate`, `AlwaysDenyGate`** — test gates.
- **`TouchIdGate`** — shells out to the Swift helper at
  `tools/touchid-prompt/`. Discovers the binary via
  `SECRETARIAT_TOUCHID_BINARY`, `SECRETARIAT_TARGET_DIR`,
  `~/.secretariat/bin/touchid-prompt`, or `$PATH`.

### Resolution

- **`DidWebResolver`** — fetches `did.json` over HTTPS, caches at
  `~/.secretariat/peers/<sanitized-did>.json`. Trust-on-first-use; no TTL
  in MVP. Returns `Err(Malformed(_))` if asked to resolve a non-`did:web`
  value.
- **`DidKeyResolver`** — pure function. Decodes the embedded key from the
  DID string itself. Zero IO.
- **`CompositeDidResolver`** — dispatches to the right resolver based on
  `did.method()`. Wired up by the CLI; tests prefer the per-method resolvers
  directly.

### Persistence

- **`KeyPaths`** — discovers `~/.secretariat/` (or honors
  `SECRETARIAT_HOME` for tests). Owns paths for key, did.json, peers cache,
  inbox, outbox, template, attention-envelope.
- **`generate_keypair`, `save_signing_key`, `load_signing_key`** — PKCS#8
  PEM IO with `0600` permissions. Refuses to overwrite existing keys.
- **`write_did_document`** — emits the `did:web` JSON document scaffold
  (`Ed25519VerificationKey2020` with `publicKeyMultibase`).

### Markdown frontmatter

- **`parse_document(content) -> ParsedDocument`** — extracts `$envelope`
  and `$attestation` from YAML frontmatter; returns body untouched.
- **`embed_stamp(body, envelope, stamp) -> String`** — emits a markdown
  document with frontmatter rebuilt deterministically. Body is preserved
  byte-for-byte, so `parse → embed → parse` round-trips equal.

### Codec

`crates/core/src/codec.rs`

- **`encode_ed25519_multibase(&[u8; 32]) -> String`** — `z`-prefixed
  base58btc with the `ed25519-pub` multicodec prefix (`0xed 0x01`).
- **`decode_ed25519_multibase(&str) -> Result<[u8; 32]>`** — inverse,
  validates length and prefix.

## Application (use cases)

`crates/core/src/application/`

- **`stamp_document(file, signer, act, force, now)`** — reads, computes
  hash, asks the `Signer` (which gates on biometric), embeds the stamp,
  writes back. Refuses if a stamp is already present unless `force = true`.
- **`verify_document(file, resolver)`** — returns one of:
  `Verified | Tampered | Unsigned | SignerUnresolvable | SignatureInvalid`.
  Each variant carries the data needed to explain itself.
- **`compose_envelope(request, template_path, outbox, now)`** — reads the
  user-customizable AG template, prepends an `$envelope` block, writes to
  `outbox/<sanitized-recipient>/<utc>-<6-char-base32>.md`.

## CLI (`sec`)

`crates/cli/src/`

```
sec init [--did did:web:<host>[:<path>]] [--force-seed]
sec compose --to <did> [--from <did>] [--depth gross|subtle]
                       [--urgency now|soon|whenever] [--source <s>]
                       [--cadence-hint <s>]
sec stamp <file> [--act attest|defer|vouch|dispute|redirect] [--force]
                 [--allow-test-biometrics]
sec verify <file> [--json]
sec list [inbox|outbox|peers]
```

Exit codes:

- `0` — success
- `1` — generic error
- `2` — verify failed (any non-`Verified` outcome) or already-stamped
- `3` — biometric refused

Environment variables:

- `SECRETARIAT_HOME` — overrides `~/.secretariat/` (used by tests)
- `SECRETARIAT_TARGET_DIR` — where to find `touchid-prompt` binary
- `SECRETARIAT_TOUCHID_BINARY` — explicit path to the biometric helper
- `SECRETARIAT_BIOMETRIC` — `touchid` (default), `always_allow`, `always_deny`.
  The non-touchid options are honored only in debug builds or when
  `--allow-test-biometrics` is set.

## Wire format

Stamped envelope = markdown with YAML frontmatter:

```markdown
---
$envelope:
  $type: app.equanimi.secretariat.envelope
  from: did:key:z6Mk... | did:web:rafa.equanimi.tech
  to: did:key:z6Mk...                # optional; absent = self-addressed
  depth: gross | subtle
  urgency: now | soon | whenever
  source: <free-form>
  cadenceHint: <optional>
$attestation:
  $type: app.equanimi.secretariat.stamp
  signer: <did>
  act: attest
  docHash: sha256:<hex>
  docFilename: <basename>             # advisory; hash is authoritative
  stampedAt: 2026-04-30T16:01:35.220898Z
  signature: ed25519:<base64-of-64-bytes>
---
# Body
...
```

**Hashing rules** (decision log #5 in the plan):

- Strip a single leading BOM (`U+FEFF`) if present.
- Normalize line endings: CRLF → LF.
- Strip trailing whitespace.
- Leading whitespace inside the body is preserved (heading position matters).
- SHA-256 over the resulting UTF-8 bytes.

The hash covers the **body only** — the `$envelope` frontmatter is routing
metadata, not protected by the signature. v2 may add envelope signing for
bilateral bound enforcement; for now, content authenticity is the contract.

## Threat model

### Defended

- **AI forging a stamp on the principal's behalf** — the signing key is
  gated by biometric; AI can call the helper but only gets a "yes/no", not
  a signature. The actual signing happens in Rust *after* the gate returns
  success. Without the principal's physical presence, no stamp can be
  produced.
- **Recipient verifying a tampered document** — any byte change to the
  body (after BOM/CRLF/trailing-whitespace canonicalization) breaks the
  hash invariant; the aggregate refuses to construct.
- **Recipient verifying an impersonator's stamp** — the signer's DID
  document is authoritative. Impersonators can't publish under someone
  else's domain (`did:web`) or fake a `did:key` (the DID *is* the key).
- **Lost integrity in transit** — signature invalidation surfaces as
  `VerifyOutcome::Tampered` or `SignatureInvalid`.

### Not defended in MVP

- **Compromise of the principal's machine** — sudo + filesystem access
  exfiltrates the signing key. (Mitigation later: Secure Enclave-backed
  keys via WebAuthn / passkey, when the GUI lands.)
- **DNS hijacking of the signer's domain** — mitigated by HTTPS + cache
  on first fetch, but not absent attack-on-first-use.
- **Coercion of the principal** — biometric verifies presence, not free
  will.
- **Replay** — same body → same hash → same signature. Intentional. The
  envelope `source` field can carry a session ID for app-level dedupe.
- **Forward secrecy** — if a key leaks, all past stamps are forgeable in
  retrospect. Mitigation later: key rotation in the `did:web`
  `assertionMethod` history.
- **Side channels in the Swift helper** — a malicious app shelling out
  to the helper gets a "yes/no", not a signature. The signing key + gate
  are co-located in the principal's CLI, not the helper.

## Test layout

61 unit tests cover:

| Layer | What |
|---|---|
| `domain::identity` | Did parse (web + key + invalid), URL/key roundtrips, DocHash + Signature serde |
| `domain::acts` | enum serde renames |
| `domain::stamp` / `envelope` / `attention_envelope` | YAML round-trip, `$type` discriminator enforcement |
| `domain::attested_document` | hash idempotence (CRLF/BOM/trailing-whitespace), aggregate invariant |
| `codec` | multibase round-trip + rejection cases |
| `infrastructure::markdown` | parse with/without/malformed frontmatter, embed→parse round-trip |
| `infrastructure::keys` | gen/save/load round-trip, `0600` perms, did.json shape |
| `infrastructure::ed25519_signer` | sign + verify with `AlwaysAllowGate` / `AlwaysDenyGate` |
| `infrastructure::did_web_resolver` | resolve from local cache fixture, reject non-ed25519 docs |
| `infrastructure::did_key_resolver` | resolve a `did:key`, reject `did:web` |
| `infrastructure::composite_did_resolver` | dispatch by method |
| `application::stamp_document` | stamp raw / refuse re-stamp / force re-stamp / preserve envelope |
| `application::verify_document` | all five `VerifyOutcome` variants with stub resolver |
| `application::compose_envelope` | recipient dir / self-addressed / template frontmatter strip |

Test biometric strategy: `Ed25519Signer<AlwaysAllowGate>` for unit tests;
`SECRETARIAT_BIOMETRIC=always_allow` env override + `--allow-test-biometrics`
flag for CLI smoke tests.

## What's not built yet

| Component | Where it'll live | Trigger |
|---|---|---|
| Tauri ceremony GUI | `src-tauri/`, `src/` (React) | After self-use validates the primitive |
| MCP server | new crate `crates/mcp` | When Claude orchestration outgrows Bash |
| Bilateral transport | new crate, server-side | After `did:key` flows are exercised at n=2 |
| Real PDS migration | replaces `infrastructure/did_web_resolver` | Multi-correspondent phase |
| Cross-platform Touch ID | WebAuthn via Tauri webview | When GUI lands |
| Lexicon publication | `lexicons/` directory becomes public | After self-use stabilizes the schema |
| `defer` / `vouch` / `dispute` / `redirect` acts | already in `StampAct`, untyped at CLI | As cadence + bilateral land |

See `~/.claude/plans/wait-you-have-a-zazzy-aurora.md` for the full
sequencing plan and the validation tests run in parallel.
