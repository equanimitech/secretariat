# Stamping over a stale signature

Why `docs/2026-05-26-bloat-audit-framing-recommit.md` verifies as
`signature=tampered, stamp=verified`. Investigated 2026-08-22.

Related: [[hister-integration]] (the spike that surfaced it) ·
`docs/pain/2026-08-20-root-cause-of-the-envelope-rewrites-readonly-gates-on-sealed.md`

## The verdict

The body was modified **between signing and stamping**. The stamp sealed the modified body;
the signature still refers to the body as it was before. `sec stamp` did not notice, because
it never checks the existing signature before sealing.

The document on disk is therefore **stamped over content its named signer never signed**,
and `sec verify` correctly reports the contradiction — but only after the fact.

## How it was established

Both blocks use the same preimage, `sha256(body.rstrip("\n"))`. Confirmed against two
healthy controls where the signature verifies:

| doc | computed | `$signature` | `$attestation` |
|---|---|---|---|
| `ideas/2026-06-14-finish-the-git-native-teardown…` | `0199aba2…` | MATCH | MATCH |
| `ideas/2026-06-14-agent-dispatch-substrate…` | `909f3889…` | MATCH | (absent) |
| **`2026-05-26-bloat-audit-framing-recommit`** | `988355d7…` | **`213c2868…` MISMATCH** | **MATCH** |

Because the preimage is identical for both blocks, a divergence can only mean the body
changed between the two operations. The attestation matching the *current* body fixes the
direction: the stamp came after the change.

Frontmatter is outside the preimage, so the `migrated_from:` key added during the
2026-05-31 git-native migration is **not** the cause.

## Window and fingerprint

```
signedAt   2026-05-26T19:32:02.714Z
stampedAt  2026-05-26T19:34:44.872791Z   → 2m 42s
```

The body carries remark-stringify artifacts — 4 escaped tildes (`\~3k LOC`), 19 `*` bullets
— which is the normalisation signature the root-cause pain doc attributes to
`@milkdown/crepe`. So the sequence was: compose + sign → open in the editor, which
normalised on load and wrote back via the phantom `onChange` (defect 2 in that doc) →
stamp.

## Why git history is not the culprit

```
git log --follow -- docs/2026-05-26-bloat-audit-framing-recommit.md
  b1232ee docs: migrate Secretariat envelopes into repo docs (git-native)
```

One commit. `git rev-parse b1232ee:<path>` and `git hash-object <working tree>` both give
`14ad8b7ea3408d7a4221e64802424b5dbc554f11` — byte-identical. The file has not been touched
since migration. The damage predates it, in `~/.secretariat`, on 2026-05-26.

## The gap in the ceremony

`crates/core/src/application/stamp_document.rs:59-99`:

```rust
if parsed.stamp.is_some() && !force { return Err(StampError::AlreadyStamped); }
let hash = canonical_body_hash(&parsed.body);      // fresh hash of the CURRENT body
…
let signature = signer.sign(&hash, &reason)?;
…
let new_content = embed_frontmatter(
    &parsed.body,
    parsed.envelope.as_ref(),
    parsed.signature.as_ref(),                     // ← carried over verbatim, unchecked
    Some(&stamp),
)?;
```

The only pre-stamp guard is `AlreadyStamped`. The existing `$signature` is preserved
verbatim — the comment says *"stamping attests to an already-signed envelope; it does not
replace the author's signature"* — but nothing verifies that signature still covers the body
being sealed.

This inverts the intended trust gradient. Hard rule #4 makes the stamp the *stronger* claim
("the stamped subset **is** the authoritative record"), so a stamp applied over a broken
signature lends the principal's Touch-ID authority to content the agent never attested. The
Touch ID reason string does guard display-vs-bytes (headline + short hash), but it says
nothing about the signature's validity, so the principal has no way to see the problem
during the ceremony.

**Proposed guard:** `stamp_document` should verify `parsed.signature` against
`canonical_body_hash(&parsed.body)` before signing, and refuse with a distinct error
(`StampError::SignatureStale` or similar) unless `--force`. Cheap — the hash is already
computed on the line above.

## Scope

From `sec verify --json` across all 221 docs in the repo:

| signature | stamp | count |
|---|---|---|
| none | none | 193 |
| none | verified | 14 |
| okUnverifiedAgent | none | 8 |
| okUnverifiedAgent | verified | 4 |
| **tampered** | **verified** | **1** |
| *(parse error)* | — | 1 |

Only 5 documents are both signed and stamped, and 1 of those 5 is broken. The 14
`none/verified` docs predate compose-signs-at-birth and carry no signature to contradict.

## What remains unknown

- The pre-normalisation body is not recoverable from this repo — the doc entered git already
  damaged, and `~/.secretariat` reportedly holds 0 envelopes now. If a Time Machine or
  pre-2026-05-31 backup of `~/.secretariat` exists, the original body is in it, and the
  signature could be re-validated rather than re-signed.
- Whether other repos (themia, minerva, leggia, penceive) carry the same pattern. This sweep
  covered only the secretariat repo. The prior pain doc names a themia file, so at minimum
  that one deserves the same preimage check.
- Whether the remaining 4 signed+stamped docs were also editor-opened between sign and stamp
  and simply happened not to be normalised (no artifacts to trigger a rewrite). If so they
  are correct by luck, not by construction.
