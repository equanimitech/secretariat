# Idea — markdown fidelity: Crepe normalizes on serialize

Raw capture — 2026-06-04. Surfaced during the editor-reader redesign.

## The problem

Crepe (Milkdown WYSIWYG) round-trips markdown through its own document model:
`markdown → ProseMirror doc → markdown`. The output is **normalized** —
whitespace, list markers (`-` vs `*`), heading styles, blank-line counts, etc.
get rewritten to Crepe's canonical form. The author's exact source formatting is
not preserved.

The change-detection poll (`getMarkdown()` every 500ms) surfaced this: on load,
Crepe's serialization differs from the on-disk text even with zero edits, so the
first poll fired a phantom `onChange` → save → **the file got rewritten just by
opening it**, and on a sealed doc that looped the break-seal dialog.

## Mitigation already shipped

The poll now baselines `lastSeen = crepe.getMarkdown()` right after create, so a
no-edit open never triggers a save. Sealed docs are read-only (no poll at all).

## The residual concern (this idea)

A **genuine** edit still re-serializes the whole document, normalizing
everything — not just the edited span. Consequences:

- **Signature breakage.** A signed (`$signature`) doc whose body is normalized
  on the first keystroke no longer matches its signed hash → verify flips to
  tampered. The author intended a small edit, not a re-hash of the whole body.
- **Noisy git diffs.** Opening + lightly editing a doc produces a diff full of
  formatting churn unrelated to the change — bad for a git-native substrate.
- **Author intent.** Markdown source *is* the artifact here; reformatting it is
  a side effect the principal didn't ask for.

## Directions (not yet chosen)

1. **Source-mode editor** (CodeMirror) instead of WYSIWYG — edits the raw
   markdown text directly, zero normalization. Loses the rich editing UX.
2. **Dual mode** — CodeMirror source by default for structured/signed envelopes,
   Crepe WYSIWYG opt-in for prose drafting.
3. **Minimal-diff serializer** — teach the save path to only rewrite changed
   regions / preserve original formatting where untouched. Hard; Crepe has no
   source-span mapping.
4. **Accept normalization** but make it explicit — a one-time "reformat on first
   edit?" prompt, and re-sign automatically.

## Relation

The `verify_envelope` keystone + the read-only-on-sealed policy already protect
the *sealed* subset. This idea is about the *signed-but-editable* and *draft*
cases. Pairs with the editor-reader redesign.

Don't shape yet.
