---
$signature:
  $type: tech.equanimi.secretariat.signature
  signer: did:key:z6MkpcX3mHt44yNEDPDWJic8ocJdagzERxx5u2Qh1dWcVRVN
  signerRole: agent
  docHash: sha256:1fd27fb81811ecba54e065a61f18dd2677bcf8c217ec5ce27eed8901ad4374e1
  signedAt: 2026-08-20T16:44:24.014878Z
  signature: ed25519:wYSCzUZGzens7Q/73UU+BYaEiUAZQWA4LzTm2nTamKS+so3RO9W+n5mCk7mvMd9hEK1AOiy2vzuzb5JPEg+wAQ==
created: 2026-08-20
severity: high
status: open
type: pain
---

# Something rewrites signed envelopes byte-for-byte

A signed document in the `themia/docs` repo was rewritten at the byte level without a single line of prose changing. `verify` reports **`tampered`**. This is the **second** occurrence, and both file-level guards were already in place.

## Observed

`themia/docs/docs/2026-06-13-jurimetria-trial-to-aha-to-conversion-strategy-brief-goals-t.md`

```
claimed:  sha256:c8d4c6196b3dc6596f3876cf434c20360446f1db8c6ae9533a8d463333144c40
computed: sha256:f61827d9187b696b3d2f3c4cf3c02b737c636e2e2a64bf3ebb0c73368ea026f8
outcome:  tampered
```

59 insertions / 43 deletions. After normalising escapes, bullets, thematic breaks and whitespace: **zero prose difference.** Every delta is a formatting artifact.

## The damage signature

| before | after |
|---|---|
| `~$0.15/turn` | `\~\$0.15/turn` |
| `---` (thematic break) | `***` |
| `- item` | `* item` |
| `[Confirmer]` | `\[Confirmer]` |
| `\| \|` (empty cell) | `\| <br /> \|` |
| `\|---\|---\|` | padded, column-aligned |

And the decisive one — **inside the `$signature` block itself**:

```diff
-  signedAt: 2026-06-13T10:55:26.970613Z
+  signedAt: 2026-06-13T10:55:26.970Z
```

Microseconds truncated. Something deserialised and re-serialised the signature YAML. That delta alone breaks the hash.

## Why it is not prettier and not VS Code

Both guards added 2026-08-11 in the consuming repo are at maximum and stopped nothing:

- `.prettierignore` ignores **`*.md`** wholesale — *"Never reformat markdown in this repo"*
- `.vscode/settings.json` disables `formatOnSave` / `formatOnPaste` / `formatOnType`, including the `[markdown]` block, **and** markdown-all-in-one's `tableFormatter`, `trimTrailingWhitespace`, `insertFinalNewline`

That settings file already names the suspect:

> *"this is defence-in-depth, not the root cause. The 2026-08-12 corruption of `2026-08-12-principes-themia.md` was traced to **secretariat's own write path**, not to VS Code."*

`| <br /> |` and escaped brackets are not prettier output — that is Slate/ProseMirror-style serialisation, i.e. **a rich-text editor round-trip**.

## Established vs inferred

**Established** — two corrupted envelopes (2026-08-12, 2026-08-20), prose intact in both, file guards at maximum, matching artifacts, and the signature YAML itself rewritten.

**Not established** — today's exact cause. The August 12 case was traced to this repo's write path; today's is **inferred** by artifact match. Nobody has instrumented the write.

## Why this is severe

The whole attestation layer rests on byte-exactness.

1. **A `tampered` verdict on a document whose prose never moved is indistinguishable, to a reader, from real tampering.** The seal loses its signal exactly where it should carry most.
2. **It trains people to ignore `tampered`.** An alarm that fires on reformatting becomes noise, and the next genuine alteration walks through.
3. **The repair is destructive by construction.** The only fix is `git checkout` on the file — the manoeuvre Themia's root CLAUDE.md forbids precisely because it destroyed in-progress work on 2026-05-26. Every corruption forces the dangerous gesture.

Principle 1.2 requires traceability **structurally, not by instruction**. Two file-level instructions are in place and failed; the defect is upstream of both.

## Leads

1. **Instrument before fixing.** Nothing today knows *who* writes. A before/after hash on the write path, logged, would settle it in one occurrence.
2. **Never rewrite a body you did not author.** If the UI opens an envelope, it renders it verbatim or refuses to save.
3. **If a round-trip is unavoidable, re-sign after it.** A rewritten body with a stale signature is a silent lie; re-signed, it is an honest new version.
4. **A `verify` pre-commit hook** on any modified envelope would catch this at commit rather than weeks later.

## Immediate repair

Prose unchanged, so `git checkout HEAD -- <file>` loses nothing and restores a valid signature. Verify the diff first; the gesture is reserved to the human.

---

*Captured 2026-08-20 during an audit of `themia/docs`. Filed here rather than in Themia's Linear: the consumer feels it, but the write path that must change is this repo's.*
