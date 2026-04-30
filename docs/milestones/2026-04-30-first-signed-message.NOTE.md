# Note on `2026-04-30-first-signed-message.md`

This is a historical artifact. **Do not modify it.**

## Namespace discrepancy (intentional)

The `$type` discriminators in this file's frontmatter are
`app.equanimi.secretariat.envelope` and `app.equanimi.secretariat.stamp`.

The current code uses `tech.equanimi.secretariat.*` — full reverse-DNS of
`equanimi.tech`. The Day 1 build used the AT-proto convention loosely and
prefixed with `app.` (the Bluesky-style `app.bsky.feed.post`). It was
corrected after the first signature.

**Consequence:** running `sec verify` on this file with current code will
fail with `expected $type tech.equanimi.secretariat.envelope, got
app.equanimi.secretariat.envelope`. That is correct historical truth —
this stamp was issued under the older naming.

The cryptographic content is fine: the signature is over the body hash,
which is unchanged. If you want to verify the actual ed25519 signature
against the body, do it manually:

```rust
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use secretariat_core::{codec::decode_ed25519_multibase, domain::canonical_body_hash};

// 1. Recover Rafa's verifying key from the did:web doc cached during Day 1
//    (the same key encoded as `did:key:z6MktG...` in any later artifact)
// 2. Compute canonical_body_hash(body_below_frontmatter)
// 3. Verify the signature on lines 16 of the file against that hash + key.
```

## Why preserved

The artifact captures a moment: ~75 minutes from plan-approval to the
first ed25519 stamp issued by the production code path, on a real Touch
ID, against a real ceremony surface. The naming bug is part of that
truth — it surfaced when Rafa pointed out that `equanimi.tech` reverses
to `tech.equanimi`, not `app.equanimi`, immediately after the milestone
was archived.
