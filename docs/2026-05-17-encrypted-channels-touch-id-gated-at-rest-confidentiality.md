---
migrated_from: equanimi.tech/project/secretariat/dev/20260517T183855Z-me2gbm.md
---
# Encrypted channels — Touch ID-gated at-rest confidentiality

> **Re-anchored.** Original capture lived in the misplaced personal-tree
> `channel:secretariat` envelope `~/.secretariat/channels/secretariat/envelopes/2026/05/17/20260517T183548Z-bkikcq.md`.
> That channel scheduled for deletion (wrong namespace, no org scope, wrong segmentation).
> Source channel for Secretariat product ideas = `channel:dev:secretariat` under `equanimi.tech` — matching the existing `channel:dev:{equanimi,zenborg,penceive}` convention.

Source: discussion during the `channel:journal:therapy` Slice A — wondering whether the stamp ceremony could also lock/unlock the capture content.

## The hook

For private channels (therapy, medical, financial), can captures be encrypted at rest such that decrypt requires Touch ID — reusing the principal's existing ceremony? Stamp already gates writes (authorship); should it also gate reads (confidentiality)?

## Crypto reality

Stamp ≠ encryption. Two distinct primitives:

- **Stamp** — ed25519 signature over body bytes. Proves authorship + integrity. Body stays plaintext on disk.
- **At-rest encryption** — X25519 ECDH + chacha20poly1305 AEAD. Confidentiality. Different keypair from signing.

Both primitives already in tree (`x25519-dalek`, `chacha20poly1305` in `Cargo.toml`). Bilateral DM encryption uses them. Reuse, don't reinvent.

Standard composition: sign-then-encrypt (sign plaintext, then encrypt). Verifier decrypts, then checks signature.

## Three real edges

1. **Backup story.** Touch ID-gated key in Secure Enclave → lose device = data unreadable forever. README invariant "Keys never leave the device" pushes toward paper recovery phrase or principal-encrypted iCloud Keychain backup. Pick one explicitly; "we'll figure it out" is how people lose decades of journals.

2. **Tantivy index breaks.** Search today indexes plaintext bodies. Encrypted bodies → three options, all bad: (a) skip index for encrypted channels (lose `sec read --search`), (b) searchable encryption (research-grade, defer indefinitely), (c) plaintext sidecar index gated by same Touch ID (complex, leaky surface). v1 = (a), accept the tradeoff for the channels that opt in.

3. **LLM review pipeline needs plaintext.** `channel:journal:therapy/bin/review.py` reads journal markdown and posts to LM Studio. If at-rest encrypted, pipeline becomes Touch ID → decrypt to memory → feed to LM Studio → discard. Once-per-run is fine; once-per-envelope is hellish. Need session-window unlocked-key caching with explicit TTL (e.g. 15 min, configurable).

## Threat model split — pick the right defense

| Threat | Right defense |
|---|---|
| Another Claude session on same machine reads via `mcp__secretariat__read` | **Slice B cognition policy** — `cognition.allowed: [local]` on channel contract, `sec-mcp` 403s on class mismatch. No crypto needed. |
| Disk theft / Time Machine backup / iCloud sync exfiltration | **Filevault first** (already 80% of the answer). At-rest channel encryption adds defense-in-depth for the "logged-in-attacker-browsing-files" case. |
| Subpoena / legal compulsion | Encryption helps only if the principal can credibly refuse to Touch ID. Adversarial scenarios outside MVP scope. |

Conclusion: ship Slice B first (cheaper, covers the realistic in-scope threat). Encryption-at-rest is real but a separate pitch with its own appetite.

## Minimum cut, when it's time

New envelope shape — extension to existing `tech.equanimi.secretariat.envelope` lexicon, NOT a new kind:

```yaml
---
$type: tech.equanimi.secretariat.envelope
from: did:key:...
to: did:key:...               # = self for private captures
handle: channel:journal:therapy
kind: therapy-review
encryption:
  scheme: chacha20poly1305-ietf
  recipient_pubkey: <x25519-multibase>
  ephemeral_pubkey: <x25519-multibase>
  nonce: <base64-12>
body_ciphertext_b64: <...>
---
```

Plaintext body lives nowhere on disk after write. New CLI:
- `sec read --decrypt <path>` — Touch ID → unwrap → decrypt → stdout
- `sec capture --encrypt --queue <handle>` — encrypt-before-write
- Decision: encrypt-by-default for channels whose contract carries `at_rest_encryption: required`

Channel-contract field (when `channelContract` lexicon ships):
```yaml
at_rest_encryption: required | optional | forbidden
```

Merge rule: max-restrictive (intersection). `required` at ancestor → all descendants required. Matches `trust_gate` semantics.

MCP impact:
- `mcp__secretariat__read` / `read_channel` Touch ID prompts on decrypt; refuse if caller can't auth.
- Encrypted captures opt out of remote relay sync until per-channel encryption is reconciled with channel keypair (org-channels need different key model — DMs are point-to-point, channel-encryption is broadcast-to-roster, harder).

## Lexicon work

- Extend `tech.equanimi.secretariat.envelope` with optional `encryption` block + `body_ciphertext_b64` (mutually exclusive with current plaintext body field).
- Add `at_rest_encryption` field to the (future) `channelContract` lexicon.
- New `tech.equanimi.secretariat.recoveryShare` lexicon for principal-encrypted backup material.

Per `[[feedback-lexicons-as-ground-truth]]`: lexicons drafted first, code follows.

## Appetite

`medium` — extension of existing primitives, not net-new crypto. Backup story is the real cost driver; design that explicitly before shipping or the feature is a footgun.

## Order of operations

1. **Slice B (cognition policy enforcement)** — covers same-machine cross-agent threat. Already on the roadmap.
2. **Threat-model doc** — decide which adversaries Secretariat aims to defend against. Channels at this layer of intimacy (therapy, medical) deserve an explicit treatment.
3. **Backup story decision** — paper recovery phrase vs. principal-encrypted Keychain backup vs. accept-loss. Pick one.
4. **This pitch** — extension of envelope lexicon + sign-then-encrypt path + Touch ID unlock flow + `at_rest_encryption` contract field + cached-unlock TTL.
5. **Optional follow-on** — multi-recipient channel encryption (broadcast to roster) for org-owned encrypted channels. Not in this slice; bilateral-style point-to-point sufficient for personal channels.

## Adjacent ideas

- **Per-envelope encryption opt-out.** A channel can be `at_rest_encryption: optional` and individual captures choose. Useful when most captures are mundane but some are sensitive.
- **Selective field encryption.** Encrypt body but leave frontmatter (`kind`, `run_at`, `model`) plaintext for indexing on metadata. Tantivy can still discover envelopes; only bodies are sealed.
- **Counter-signature → counter-decrypt.** Multi-principal channels (assemblee_generale model) → both principals must Touch ID to decrypt jointly-stamped envelopes. Real cryptographic threshold or simple OR-gate? Out of scope; flag for future.
- **Memory of unlocked keys across the daemon.** Daemon (already long-running) is the natural cache holder. Session TTL configurable in `preferences.toml` (`encryption.unlock_ttl_minutes`).
