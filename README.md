# Secretariat

**A markdown editor that lets a human stamp what an AI wrote — cryptographically, with a fingerprint.**

AI drafts at volume. Secretariat gives the human a way to vouch for the
moments that count: open a markdown document, read it, and seal it with a
biometric-gated signature. The stamp is verifiable by anyone, forever, with
no server in the middle. The substrate is plain git repositories — every
document is a markdown file you can `tar`, fork, or walk away with.

## The primitive

One trust act, two verifiable layers, embedded in the document itself:

1. **Signature** — a detached ed25519 signature keyed to a DID, proving _who
   authored this body_. The author may be a human principal or an authorized
   agent (a scribe).
2. **Stamp** — a Touch ID attestation from the human principal, embedded as an
   `$attestation` block in the file's frontmatter. **Selective, not
   mandatory** — applied to the documents the principal elects to elevate
   (decisions, commitments, contracts). The stamped subset is the
   authoritative record; everything else is just context.
3. **Counter-stamp** — multi-principal stamp on the same document. Reserved;
   designed in the lexicon, no record type ships yet.

Stamping embeds the attestation **in place** — no rename, no path change. The
hash covers the body; editing the body after sealing breaks the stamp, and
`sec verify` reports it as tampered.

## What ships today (v0.12, macOS)

- **`sec` CLI** — `init` / `stamp` / `verify` / `read` / `launch` / `agent` /
  `profile` / `daemon` / `mcp` / `view`. End-to-end working.
- **`sec-mcp` server** — Claude (or any MCP client) reads, verifies, and
  _proposes_ a stamp; the human signs with Touch ID. Tools: `stamp`, `read`,
  `verify`, `agent_*`. Claude never stamps — it shows you the body, you seal it.
- **Markdown editor (Tauri app)** — read/edit markdown, frontmatter sidebar,
  the stamp ceremony UI, command palette, quick pane. No notifications, no
  push — anti-compulsion by design.
- **`sec launch`** — opens Claude Code (or any configured cognition CLI) with
  `cwd` set to a repo, so `cd <repo> && claude` activates the full project
  context for free.

The substrate is **git repositories**: documents live as markdown under a
repo's `docs/`, the identity + signing key live under `~/.secretariat/`.
Review and seal documents directly on top of git with the `/review-repos`
walker (`git` + `sec verify` + `sec stamp`).

## Architectural invariants

These are properties of the system, not rules of behavior. Violating one means
we shipped the wrong thing.

- **No central server.** Identity is direct DID resolution (`did:web` over
  HTTPS, or self-proving `did:key`). No broker, registry, marketplace.
- **No telemetry.** Nothing phones home. Verification is self-contained.
- **Keys never leave the device.** No vendor keystore. Backups are
  user-encrypted only.
- **Cognition is pluggable.** Claude Code, Anthropic API, local models
  (Ollama / llama.cpp / MLX), Bedrock. The principal owns the brain.
- **Filesystem is authoritative.** Every document, identity, and instruction
  is a markdown file on disk — in a git repo or under `~/.secretariat/`. No
  database-as-truth.
- **No SaaS distribution.** A hosted Secretariat collapses the primitive the
  moment a server holds keys. Local app + CLI plus optional self-hosted
  `did:web` only.

## Quick start

```bash
# Prerequisites: Rust (latest stable), pnpm, Node 18+
# See docs/developer/ for platform-specific notes

git clone <repo> secretariat
cd secretariat
pnpm install
pnpm tauri:build

# Initialize identity (writes the signing key under ~/.secretariat/)
sec init                    # generates did:key
# or
sec init --did did:web:you.example.com

# Wire up MCP for Claude Code
sec mcp install

# Read, then stamp a document
sec read   <file>           # show the body
sec stamp  <file>           # Touch ID prompt — seals in place
sec verify <file> --json    # {signature, stamp, counter_stamps}
```

Full setup: see [`docs/developer/`](docs/developer/).

## Status

**Alpha. Pre-1.0.** Breaking changes per minor version. The correspondence
apparatus (federation, channels, orgs, compose) was deliberately cut in
v0.12.0 — see [`CHANGELOG.md`](CHANGELOG.md) and
[`docs/ideas/2026-05-31-git-native-substrate.md`](docs/ideas/2026-05-31-git-native-substrate.md).
What remains is the editor + the Signet stamp/verify/read core over a
git-native substrate. Schemas under [`lexicons/`](lexicons/) mirror the
on-wire shape but are not yet runtime-validated.

Built in the open because the only honest way to ship a sovereignty product is
to make every piece inspectable.

## Docs

- [`AGENTS.md`](AGENTS.md) — orientation for Claude Code and coding agents
- [`docs/developer/`](docs/developer/) — architecture, wire format, threat model
- [`docs/decisions/`](docs/decisions/) — architectural decisions
- [`docs/pitches/`](docs/pitches/) — Shape Up pitches (note: many pre-teardown pitches are historical records)
- [`lexicons/`](lexicons/) — AT-proto-shaped record schemas
- [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md) · [`docs/SECURITY.md`](docs/SECURITY.md)

## License

MIT. See [`LICENSE.md`](LICENSE.md).
