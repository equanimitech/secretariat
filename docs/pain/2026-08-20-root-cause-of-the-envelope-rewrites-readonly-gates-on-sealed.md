---
$signature:
  $type: tech.equanimi.secretariat.signature
  signer: did:key:z6MkpcX3mHt44yNEDPDWJic8ocJdagzERxx5u2Qh1dWcVRVN
  signerRole: agent
  docHash: sha256:906381c973f0dd77d44f79dc07ce733e0d4f76ca86871e074205ce01b245a9f6
  signedAt: 2026-08-20T16:51:04.966631Z
  signature: ed25519:IqVXD/dzpYI+e1J0aCAvFLelKr7OaH312DoJS5GbKMJZrJziooA6rXPOVtBle6bZRu1HO8CZrEyb3yXeycl+Cw==
created: 2026-08-20
severity: high
status: open
type: pain
---

# Root cause of the envelope rewrites

Extends `docs/pain/2026-08-20-something-rewrites-signed-envelopes-byte-for-byte-second-cor.md`, which recorded the cause as **inferred by artifact match**. It is now located, two lines.

Composed as a new document rather than an edit: the prior one is a signed envelope, and editing signed envelopes is the defect itself.

## The editor

`@milkdown/crepe` — ProseMirror core, **remark** serialisation. Every artifact in the prior report is a remark-stringify default: `*` bullets, `***` thematic breaks, escaped `[`, padded table columns. `| |` → `| <br /> |` is ProseMirror being unable to hold an empty table cell.

## Defect 1 — the read-only guard misses half the envelopes

`src/components/markdown/MarkdownWindow.tsx:321`

```tsx
readonly={verify.state === 'sealed'}
```

`src/lib/markdown/trust.ts:4`

```ts
export type TrustState = 'sealed' | 'signed' | 'unsigned' | 'tampered'
```

and from `trust.test.ts`:

```ts
deriveTrustState(result('ok', 'none')) === 'signed'   // signature ok, no stamp
```

**A `signed` envelope opens read-write, with the change-poll live.** But `$signature` carries a `docHash` over the body exactly as `$attestation` does. It is equally hash-covered and receives none of the protection.

That is precisely the file that broke — `themia/docs/docs/2026-06-13-jurimetria-trial-to-aha-…md` is signed-only, never stamped.

The guard is not wrong, it is scoped to the wrong predicate. The question is not *"is this sealed?"* but *"is this body covered by a hash?"* — true for `sealed` **and** `signed`.

## Defect 2 — an edit writes remark's normalisation, not the edit

`src/components/markdown/CrepeEditor.tsx:108–121`. The comment already states the mechanism:

> *"on load (whitespace, list markers, etc.), so `getMarkdown()` differs from the on-disk text even with zero edits. Without this baseline the first poll fires a phantom onChange — rewriting the file on open and, on a sealed doc, looping the break-seal dialog."*

```ts
lastSeen = crepe.getMarkdown()          // baseline = the NORMALISED text
pollTimer = window.setInterval(() => {
  const md = attached.getMarkdown()
  if (md !== lastSeen) { lastSeen = md; onChangeRef.current(md) }
}, 500)
```

Baselining to the normalised output correctly suppresses the zero-edit phantom write. But the baseline **is** the normalisation, so the moment a real edit lands, the value written is the fully re-serialised document. **One keystroke reformats the whole file** and breaks the hash — the user's change and remark's rewrite are indistinguishable at the save boundary.

## Fixes, smallest first

1. **Widen the guard.** `readonly={verify.state === 'sealed' || verify.state === 'signed'}`. One line, stops the observed case immediately. Costs the ability to edit a signed draft in the app — which is arguably correct: an envelope with a hash over its body is not a scratchpad.
2. **Save a diff, not a re-serialisation.** If editing signed drafts must stay possible, apply the user's change to the *original bytes* rather than writing ProseMirror's view of the document. Harder, and the right long-term shape.
3. **Re-sign on write.** Any path that legitimately rewrites a body must produce a new signature in the same operation. A rewritten body under a stale signature is a silent lie.
4. **`verify` in a pre-commit hook** on modified envelopes — catches it at commit rather than weeks later.

## On "we can always re-stamp"

True mechanically, and worth stating carefully, because it is the most dangerous repair available.

Re-stamping a corrupted envelope **does** restore a valid seal — over the corrupted body. The hash then attests to remark's normalisation rather than to what the author wrote. Nothing downstream can tell the difference, and the corruption becomes permanent and *attested*.

That is acceptable only when the body has been read and confirmed to be what was meant. For formatting-only damage that check is cheap. But if re-stamping becomes the reflex repair, the seal stops detecting change — which is its only function. A seal that is re-applied whenever it complains is a seal that says nothing.

**The rule worth holding:** re-stamp only after diffing against the last known-good version and confirming no prose moved. Prefer `git checkout` where the prior bytes still exist — it restores the original signature rather than minting a new one over damage.

## Immediate repair, unchanged

`themia/docs/docs/2026-06-13-jurimetria-trial-to-aha-to-conversion-strategy-brief-goals-t.md` — prose unchanged, so `git checkout HEAD -- <file>` loses nothing and restores the original valid signature. Preferred over re-stamping, for the reason above.
