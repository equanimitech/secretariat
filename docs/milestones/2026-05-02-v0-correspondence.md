# v0 — first correspondence loop

Date opened: 2026-05-02.
Builds on: `docs/milestones/2026-04-30-first-signed-message.md` (Day 1, single-principal stamping).

## Thesis

Smallest demonstration that the *correspondence loop* — not just the stamp —
works end-to-end. Two principals, one transport (**self-hosted relay**:
WebSocket + HTTP, run by either principal or a trusted peer), MCP-first
surface with CLI alongside, body end-to-end encrypted, manual DID exchange.

**Why relay instead of email** (decision date 2026-05-02): email demanded
either (a) a dedicated mailbox per principal — 15–25 min of provider-UI
account creation + 2FA + app-password generation that even technical users
balk at, or (b) header-injection + server-side filter setup that's brittle
and silently fails on header-stripping providers. The relay path eliminates
all account-creation friction (`sec relay register --endpoint <url>` is one
command), keeps sovereignty intact (the relay never sees plaintext, and
"the relay" is just whoever you or a peer hosts), and gives us a wire
format we own end-to-end — no MIME, no provider quirks, no INBOX leak.

If v0 works, every later phase (menubar, MCP, invitation handshake,
bilateral contracts, more transports) is incremental on a proven substrate.

## Acceptance criteria

Rafa and Marcelo can complete this round trip without leaving the terminal:

1. Rafa: `sec compose --to marcelo --body "ch7 push-back"` → encrypted
   envelope appears in `~/.secretariat/outbox/<marcelo-did>/<utc>.md`
2. Rafa: `sec stamp <file>` → Touch ID prompt → stamped
3. Rafa: `sec send <file>` → Gmail API call → email leaves Rafa's outbox
4. Marcelo's daemon (already running) polls Gmail, picks up the message,
   decrypts the body locally, verifies the signature, places the cleartext
   stamped envelope in `~/.secretariat/inbox/<utc>-rafa.md`
5. Marcelo: `sec list inbox` → sees the entry
6. Marcelo: `sec read <file>` → decrypts + re-verifies + prints body
7. Marcelo: `sec compose --to rafa --reply-to <hash>` → drafts response
8. Marcelo: `sec stamp + send` → mirror of 2–3
9. Rafa's daemon receives, decrypts, verifies, places in inbox
10. Rafa: `sec read <reply>` → loop complete

All four envelopes (greeting + reply on each side) are signed by their
authors and end-to-end encrypted to the recipient's DID-derived x25519
key. Gmail sees ciphertext attachments only.

## Scope

### In

- Body encryption: ed25519 → x25519 conversion + sealed-box AEAD (XChaCha20-Poly1305)
- Wire format extension: encrypted envelope variant, ciphertext is what gets hashed
- Contact book aggregate: `(did, display_name, relay_endpoint?)` in `~/.secretariat/contacts.json`. `relay_endpoint` is `None` for `did:web` peers (looked up live via the DID document's `serviceEndpoint`) and `Some(url)` for `did:key` peers (out-of-band exchange, like email used to be)
- **Self-hosted relay server** (`crates/relay`): tokio + axum + WebSocket. Open registration by default, allowlist mode via `--allowlist <did-list>` flag for hostile networks. Per-tenant in-memory queue with disk-backed durability and TTL (default 7 days). Verifies sender signatures on POST; recipient pulls authenticate via DID-signed nonce challenge
- **Railway deployment kit** in `crates/relay/`: `Dockerfile` (multi-stage Rust build → distroless runtime) + `railway.json` (persistent volume mount, healthcheck, restart policy) + README "Deploy on Railway" button. Custom domain via Railway dashboard → CNAME → automatic TLS. Alternatives documented: Render, Hetzner/DigitalOcean VPS, self-hosted on Mac via Tailscale
- **Relay client adapter** (`crates/core/src/infrastructure/transport/relay.rs`): implements the `Transport` trait. Connects via WebSocket (or HTTP polling fallback), registers on first use, polls inbox on cadence, POSTs outbound encrypted envelopes
- DID document `serviceEndpoint` extension for `did:web` users — relay URL advertised in `did.json` per DID Core spec. No new lexicon needed for discovery
- Daemon: long-running process — *cadence-respecting*, *attention-envelope-aware*, *notification-free* (see "Anti-compulsion rituals" below)
- MCP server (rmcp-based, stdio transport): `compose`, `list_outbox`, `list_inbox`, `read`, `verify`, `list_contacts`, `add_contact`. **`stamp` and `send` are deliberately not exposed** — stamp is principal-only (rule 4); send is daemon-only (auto-fires on stamped envelopes per recipient's window)
- CLI surface: `sec contact`, `sec relay {serve,register,status}`, `sec daemon`, `sec read`, `sec stamp`
- macOS only (relay binary cross-compiles to Linux for VPS hosting; client stays macOS-only in v0)

### Out (deferred)

- Menubar / Tauri shell — CLI is the v0 stamp surface (Touch ID still works via existing helper)
- Invitation / first-contact handshake — DID exchange is manual (text, signal)
- Bilateral contracts — both sides accept all inbound from known contacts
- Multiple transports — Gmail only; self-hosted relay and Iroh come later
- Slack adapter — deliberate workplace transport, deferred
- Cross-platform — Windows when GUI lands
- Local LLM substrate — no agent loop in v0; CLI does not call any LLM
- Group correspondence — pairwise only
- Forward / quote / cite primitives — basic compose only

## User flow (concrete)

### One-time setup (each principal)

```bash
# already works from Day 1
sec init                                    # rafa: derives did:key OR
sec init --did did:web:rafa.equanimi.tech   # rafa: did:web

# new in v0
sec contact add marcelo \
    --did did:key:z6MkMar... \
    --email marcelo@gmail.com
sec email connect                           # paste app password, stored in Keychain
                                            # works with any IMAP/SMTP provider
sec daemon start                            # backgrounds, logs to daemon.log
                                            # honors ~/.secretariat/attention-envelope.md
                                            # for both poll cadence + delivery windows
```

### Sending

```bash
sec compose --to marcelo --body-file draft.md
# → ~/.secretariat/outbox/did-key-zMar/2026-05-02T18-04-22Z-7K2QPL.md
sec stamp <file>          # Touch ID
sec send <file>           # Gmail API; on success, moves to outbox/sent/
```

### Receiving (passive, daemon-driven)

```bash
# daemon already running — polls Gmail, decrypts, verifies, files
sec list inbox            # see new arrivals
sec read <file>           # decrypts ciphertext if not already, prints body
```

### Replying

```bash
sec compose --to rafa --reply-to <hash-of-incoming>
sec stamp <new-draft>
sec send <new-draft>
```

## Components to build

| # | Component | Path | Estimate |
|---|---|---|---|
| 1 | Contact aggregate + JSON persistence + CLI | `crates/core/src/domain/contact.rs`, `crates/core/src/infrastructure/contact_store.rs`, `crates/cli/src/commands/contact.rs` | ~400 LoC |
| 2 | x25519 conversion + sealed-box encryption | `crates/core/src/infrastructure/crypto/sealed.rs` | ~400 LoC |
| 3 | Encrypted envelope wire format | extend `crates/core/src/infrastructure/markdown.rs`, add `EncryptedEnvelope` value object in domain | ~250 LoC |
| 4a | Relay server crate (axum + tokio + per-tenant queue + sig verification) | new crate `crates/relay`, binary `sec-relay` | ~600 LoC |
| 4b | Relay client adapter implementing `Transport` trait (WebSocket primary, HTTP polling fallback) | `crates/core/src/infrastructure/transport/{mod.rs,relay.rs}` + `RelayEndpoint` value object | ~400 LoC |
| 5 | Cadence policy: attention-envelope-aware delivery + polling | `crates/core/src/application/delivery_policy.rs` | ~200 LoC |
| 6 | Daemon loop (cadence-respecting, notification-free) | `crates/cli/src/commands/daemon.rs` | ~300 LoC |
| 7 | MCP server (rmcp, stdio transport) | new crate `crates/mcp` exposing 7 tools listed above | ~400 LoC |
| 8 | CLI command wiring | `crates/cli/src/commands/{contact,relay,daemon,read}.rs` | ~250 LoC |
| 9 | Round-trip integration test (two daemons + in-process relay) | `crates/core/tests/v0_correspondence.rs` | ~300 LoC |

Total: ~3.5k LoC. Roughly 2 weeks focused. Slightly larger than the email
path because we own the relay binary too — but no provider integrations,
no keychain, no MIME parsing, no header injection / filter setup, and no
account-creation friction at the user-facing edge.

## Wire format additions

Encrypted-body envelope variant (when body is sealed for a recipient):

```markdown
---
$envelope:
  $type: tech.equanimi.secretariat.envelope
  from: did:web:rafa.equanimi.tech
  to: did:key:z6MkMar...
  depth: subtle
  urgency: whenever
  encryption: x25519-xchacha20poly1305
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:web:rafa.equanimi.tech
  act: attest
  docHash: sha256:<hex over ciphertext>
  stampedAt: 2026-05-02T18:04:22Z
  signature: ed25519:<base64>
---

$encrypted: |
  {ephemeral_pubkey}:{nonce}:{ciphertext+tag}
  base64-of-binary-blob, multi-line ok
```

**Hash invariant change:** `docHash` is computed over the ciphertext bytes
(after canonicalization), not the plaintext. Reason: the recipient must
verify *what was signed* without needing to decrypt first; signature
authenticates the wire bytes. Plaintext is verified separately on
decryption.

**Body-only encryption** — frontmatter remains plaintext (routing must be
visible to the daemon for filing). This leaks `from`, `to`, `depth`,
`urgency` to the transport — same metadata email already leaks. Worse leak
prevented (body content sealed); same-tier metadata accepted as the cost
of using email at all.

## Anti-compulsion rituals

Substrate must *embody* equanimity, not require self-discipline against it.
The transport must not enable behaviors the principles forbid.

### Principal-side (Secretariat substrate)

- **Daemon poll cadence:** default hourly. Configurable in
  `~/.secretariat/cadence.toml`. Minimum 15 min. Honors
  `attention-envelope.md` for both poll cadence *and* delivery windows.
- **No native notifications:** no banner, badge, sound, dock bounce.
  Inbox state is latent — visible only on user request.
- **No counts:** envelope titles surfaced; "N unread" excluded.
  (BCT 10.4 — variable-ratio reinforcement breeds compulsion.)
- **Outbound delivery respects recipient's window:** envelopes stamped
  at 11pm don't transmit until the recipient's next inbound window.
  Substrate prevents late-night intrusion regardless of sender intent.
- **MCP `list_inbox` description:** "Use only when user explicitly asks
  for their inbox; do not call proactively." Encoded in tool spec so
  Claude doesn't auto-check.
- **Default reading mode:** morning digest (yesterday's verified
  inbound, summarized by Claude on principal's request). Newspaper, not
  feed.
- **No receipts:** delivered / read / typing all excluded at protocol
  level, not just absent in UI.
- **Reply latency norm:** v0 has no contracts yet, but UI/copy primes
  1–3 business days as default expectation.

### Transport-layer (no leak by construction)

The relay path eliminates the recipient-side leak problem entirely.
The Secretariat relay is a private wire — no Gmail / Apple Mail /
provider client is connected to it. Notifications cannot fire because
no other software is watching the queue. The daemon is the only reader.

This is a structural improvement over the email path: instead of
inventing mitigations to keep envelopes out of an INBOX that other
software watches (filter rules, dedicated mailboxes, custom headers),
the relay simply *isn't* an INBOX. It's a Secretariat-only queue.

## Auth strategy: DID-signed challenges, no passwords

The relay never holds a credential. Authentication is cryptographic:

- **Sender → relay (POST inbox):** sender signs the envelope with their
  ed25519 key. Relay verifies the signature against the sender's
  resolved DID document before queuing. No relay-side password.
- **Recipient → relay (GET inbox):** relay issues a random nonce.
  Recipient's daemon signs `(nonce, did)` with its ed25519 key.
  Relay verifies. Session token issued for short-lived polling
  (e.g. 1 hour) so we don't sign-per-request.
- **Registration:** `sec relay register --endpoint <url>` POSTs a
  signed registration message. Relay verifies the signature, records
  the DID + public key. Open registration by default; allowlist mode
  available via `--allowlist <did1,did2,...>` for hostile networks.

Zero password creation, zero account creation, zero provider UI navigation.
The principal's existing ed25519 key (already generated by `sec init`) is
the credential.

## Daemon, not agent

Secretariat the substrate is *plumbing* — post office, not editor. It
moves sealed bytes between principals according to stamped
instructions, respects cadence, files inbound. It does not read,
summarize, draft, or triage.

Cognition lives in Claude (or any MCP client of the principal's
choice). Claude calls MCP tools (`list_inbox`, `read`, `compose`);
Secretariat returns data. The agent loop runs in whatever the principal
points at the MCP server.

Why this separation matters:

- No LLM dependency in the daemon → cheap, deterministic, testable.
- BYOK / local LLM / vendor choice live in the MCP client, not us.
- The daemon is the trust-critical path (keys, biometrics, encryption);
  smaller is safer.
- Invariant #5 (cognition pluggable) is *satisfied by not having
  cognition in the daemon* — the principal plugs whichever brain they
  want into the MCP surface.

When Secretariat eventually gains cognition (autonomous triage,
schedule-aware drafting, contract counter-proposal): it goes behind a
`CognitionPort` adapter, principal-selectable, opt-in. Not v0.

### Process model on macOS

| Phase | Process |
|---|---|
| v0 | macOS LaunchAgent (`~/Library/LaunchAgents/tech.equanimi.secretariat.daemon.plist`) registered by `sec daemon install`. User-session scope, Keychain access, auto-restart on crash. |
| v0.1+ | Tauri menubar app subsumes the daemon — same process, adds UI. LaunchAgent points at the menubar binary instead of the headless one. |

## Risk register

| Risk | Severity | Mitigation |
|---|---|---|
| Gmail OAuth scope changes / token expiry | medium | Refresh-token loop + clear error on revoke; document re-auth flow |
| First-contact emails land in spam | medium (v0 sidesteps — both have whitelisted each other) | DKIM/SPF on did:web domain in v0.1 |
| Daemon crash leaves user blind to inbound | medium | Health check command (`sec daemon status`); LaunchAgent restart in v0.1 |
| Hash-over-ciphertext means recipients can't verify *content* before decrypting | low — by design | Decryption + signature check both required; failure of either rejects envelope |
| x25519 conversion subtleties (clamping, signature compatibility) | medium | Use `ed25519-dalek::SigningKey::to_x25519_static_secret` (or equivalent vetted lib); add property tests |
| Gmail send rate limits | low | v0 traffic is hand-driven; revisit at scale |
| User forgets `sec daemon start` after reboot | high (UX friction) | Document explicitly; LaunchAgent in v0.1 |

## Out-of-band setup (v0 only)

DID exchange is manual for v0. The two principals share their `did:key`
or `did:web` value via any side channel (text, signal, in person).

This intentionally defers the invitation lexicon + claim-URL flow to v0.1.
Reason: bootstrapping the first n=2 with manual exchange is fine because
both of us are technical; the viral handshake matters when reaching
non-technical users (Marcelo's dad, Christophe), which v0 doesn't yet
target.

## Validation tests run in parallel

- **Self-correspondence:** Rafa sends to Rafa (own DID, own email). Verifies
  the encryption + transport round-trip without needing two principals.
- **Tamper detection:** modify an in-flight email body — daemon rejects
  on hash mismatch.
- **Wrong-recipient detection:** Rafa encrypts to Marcelo, daemon on Rafa's
  side fails to decrypt (no x25519 secret for Marcelo's pubkey).
- **Replay:** same envelope arrives twice (forwarded by accident) — daemon
  dedupes by `docHash`.

## What v0 proves

- The sovereignty stack works on the wire, not just on disk.
- E2E encryption integrates cleanly with stamping (hash-over-ciphertext).
- Email is a viable bootstrap transport with the privacy properties we want.
- The inbound-watcher + outbound-send pattern generalizes — adding a second
  transport is now an adapter swap, not an architecture change.

## What v0 explicitly does *not* prove

- Non-technical user UX (deferred to menubar + invitation flow phase).
- Multi-agent compatibility (deferred to MCP phase).
- Bilateral attention bounds (deferred to contract phase).
- Cross-platform.
- Resilience under provider-account suspension.

## Build status (as of 2026-05-02 evening)

**Done (8 of 9 chunks, ~3.2k LoC, ~160 tests):**

- ✅ Chunk 1 — Contact aggregate + JSON store + `sec contact` CLI
- ✅ Chunk 2 — x25519 conversion + sealed-box AEAD (XChaCha20-Poly1305)
- ✅ Chunk 3 — Encrypted envelope wire format (`encryption` field on Envelope)
- ✅ Chunk 4a — Relay server crate (`crates/relay`) + Railway deploy kit
- ✅ Chunk 4b — Relay client adapter (`RelayClient` + `RelayState`)
- ✅ Chunk 5 — Cadence policy (hourly default, 15-min floor)
- ✅ Chunk 6 — Daemon loop (`sec daemon register/serve`)
- ✅ Chunk 8 — CLI command wiring (`sec contact`, `sec daemon`, `sec read`)
- ✅ Chunk 9 — Round-trip integration test (rafa → marcelo end-to-end:
  encrypt + stamp + relay-send + relay-poll + decrypt + hash-invariant
  verify, plus tamper-detection + wrong-recipient negative tests)

**Remaining: Chunk 7 — MCP server (rmcp).** Deferred to a follow-up
session due to context budget. Without it, the substrate is fully
working but Claude can only drive via `Bash` shelling out to `sec`
(which works for v0 use). With it, the MCP tool surface (`compose`,
`list_outbox`, `list_inbox`, `read`, `verify`, `list_contacts`,
`add_contact`) becomes the canonical Claude interface.

## Next milestones (sketched, not committed)

- **v0.1 — non-technical onboarding.** Menubar stamper. Invitation lexicon
  + claim URL handler. Static install page. Reaches Marcelo's dad.
- **v0.2 — agent surface.** MCP server. Claude (and any MCP client) drives
  compose/list/verify. CLI demoted to dev tool.
- **v0.3 — bilateral contracts.** `tech.equanimi.secretariat.contract`
  lexicon. Cadence + depth bounds enforced on inbound. Negotiation
  envelopes.
- **v0.4 — second transport.** Self-hosted relay (single-tenant, on user's
  own VPS). Removes Gmail metadata for users who care.

Each step shippable independently, each useful on its own.
