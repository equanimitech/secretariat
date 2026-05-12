# Secretariat

**Ambient context for AI, stamped by humans.**

Most tools treat AI as a consumer of static context — RAG, system prompts, one-shot retrieval. Secretariat inverts that: AI lives *in* the context stream, reads and drafts continuously, and the human only enters to stamp the moments that count. It's the operating layer for an autonomous enterprise — where agents draft at volume, humans vouch for what matters, and no vendor sits in the middle.

## The primitive

Two records, three trust layers:

1. **Signed envelope** — every message carries a detached signature from its author (human or AI agent), keyed to a DID. Mandatory. Drives provenance: *did this come from the claimed author?*
2. **Stamp** — Touch ID attestation from the human principal. **Selective, not mandatory.** Applied to envelopes the principal elects to elevate: decisions, commitments, external messages, contracts. The stamped subset is the org's authoritative record. Everything else is ambient context.
3. **Counter-stamp** — multi-principal stamp on the same envelope (process-verbaux model). Reserved for later.

That's it. Everything else — channels, contracts, agents — is composition over those primitives.

## What ships today (v0.2, macOS)

- **`sec` CLI** — `init` / `compose` / `stamp` / `verify` / `list` / `daemon install` / `mcp install`. End-to-end working.
- **`sec-mcp` server** — Claude (or any MCP client) drafts, verifies, manages contacts. Stamping still requires Touch ID — Claude proposes, the human signs.
- **Tray app** — quick-pane capture, daemon wiring, review surface. No notifications, no compose UI — deliberate. Anti-compulsion by design.
- **Bilateral correspondence** — one-to-one envelopes over any transport (Gmail today, more later). End-to-end encrypted to the recipient's DID-derived key.

## Where it's going

- **v0.3 — channels.** Multi-subscriber threads with their own contracts and history. AI agents draft into channels with their own keys. Selective stamping marks decisions; the rest flows signed-only.
- **v0.4+** — attention routing, multi-device key migration, optional self-hosted relays, channel ownership transfer.

## Architectural invariants

These are properties of the system, not rules of behavior. Violating one means we shipped the wrong thing.

- **No central server.** Federation is direct DID resolution. No broker, registry, marketplace.
- **No telemetry.** The daemon never phones home.
- **Keys never leave the device.** No vendor keystore. Backups are user-encrypted only.
- **Transports are adapters, not authorities.** Gmail, Slack, IMAP — dumb pipes carrying signed ciphertext. The substrate doesn't trust them.
- **Cognition is pluggable.** Claude Code, Anthropic API, local models (Ollama / llama.cpp / MLX), Bedrock. The principal owns the brain.
- **Filesystem authoritative.** Every envelope, contract, instruction is a markdown file on disk. No database-as-truth. `tar` it, fork it, walk away with it.
- **Owner-as-sequencer per channel.** Strong consistency emerges from each channel's owner, not from consensus. Cross-channel global order is explicitly not provided.
- **No SaaS distribution.** Hosted Secretariat collapses the primitive. Local daemon plus optional self-hosted `did:web` only.

## Quick start

```bash
# Prerequisites: Rust (latest stable), pnpm, Node 18+
# See docs/developer/ for platform-specific notes

git clone <repo> secretariat
cd secretariat
pnpm install
pnpm tauri:build

# Initialize identity
sec init                    # generates did:key
# or
sec init --did did:web:you.example.com

# Wire up MCP for Claude Code
sec mcp install

# Compose + stamp + send
sec compose <recipient-did>
sec stamp <draft-path>      # Touch ID prompt
```

Full setup: see [`docs/developer/`](docs/developer/).

## Status

**Alpha. Pre-1.0.** Breaking changes per minor version. Two real users (one is the author). Schemas under [`lexicons/`](lexicons/) mirror the on-wire shape but are not yet runtime-validated.

Built in the open because the only honest way to ship a sovereignty product is to make every piece inspectable. If the design resonates, the most useful thing you can do is try it and tell us what broke.

## Docs

- [`AGENTS.md`](AGENTS.md) — orientation for Claude Code and coding agents
- [`docs/developer/`](docs/developer/) — architecture, wire format, threat model
- [`docs/specs/`](docs/specs/) — record types and protocol
- [`docs/decisions/`](docs/decisions/) — architectural decisions
- [`docs/pitches/`](docs/pitches/) — Shape Up pitches for in-flight work
- [`lexicons/`](lexicons/) — AT-proto-shaped record schemas
- [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md) · [`docs/SECURITY.md`](docs/SECURITY.md)

## License

MIT. See [`LICENSE.md`](LICENSE.md).
