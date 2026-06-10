---
$signature:
  $type: tech.equanimi.secretariat.signature
  signer: did:key:z6MkpcX3mHt44yNEDPDWJic8ocJdagzERxx5u2Qh1dWcVRVN
  signerRole: agent
  docHash: sha256:1d6fbef006aa75d0ac320033e076437b66d60e7bba5505376e09b3068f57b9c4
  signedAt: 2026-06-10T14:36:57.957001Z
  signature: ed25519:zCs5xvMFQiQBEVAOfpiZEdMgTnOZ2iTDxWmuy3DaTC/HrQnhNH/6MeIdR1Ibn2YTsXLRc8EOQVAXlNfRbdGyCw==
type: pain
---
# Never CLI-swap a binary inside a signed .app

**What happened (2026-06-10).** While releasing a `sec-mcp` fix, I tried to hot-swap the binary inside `/Applications/Secretariat.app/Contents/MacOS/sec-mcp`. Two compounding failures:

1. `install`'s sandbox-fallback **unlinked the destination before** failing to rewrite it → the app's `sec-mcp` was deleted, not replaced.
2. macOS **App-Management protection** blocks *any* CLI write into a Team-ID-signed `.app` (`Operation not permitted`), even with the shell sandbox disabled — so it couldn't be restored from the CLI either.

**Lesson.** Binaries inside a signed `.app` are immutable from the CLI. Don't try. The only durable path is the **release pipeline**: bump → tag → push → CI builds + signs the bundle → auto-update replaces the whole `.app`. That also self-heals a damaged bundle.

**Also:** `~/.local/bin/sec-mcp` (what Claude Code launches) *is* freely writable, which is why this session recovered while Claude Desktop stayed broken until auto-update.

**Resolution:** released v0.16.1; the auto-update restores the bundle.
