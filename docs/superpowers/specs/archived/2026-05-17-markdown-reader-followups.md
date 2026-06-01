# Markdown reader/editor — v1 follow-ups

Deferred from the v1 ship (see `2026-05-17-markdown-reader-design.md` /
`plans/2026-05-17-markdown-reader.md`).

## Default-app onboarding card

v1 declares `.md`/`.markdown` as an Editor association with `Alternate` rank
so Secretariat appears in Finder's "Open With" list — but the principal must
opt in via Finder ("Get Info → Open With → Secretariat → Change All").

Follow-up: in-app card with two affordances:

1. Reveal the active `.md` handler.
2. Call `LSSetDefaultRoleHandlerForContentType("net.daringfireball.markdown", "tech.equanimi.secretariat", kLSRolesEditor)` via a small Rust helper (link `objc2` + `LaunchServices`).

Skipped from v1 because it adds objc-bridging weight for a one-time action the
principal can do once in Finder.

## Tabs / pinned files

Each markdown opens in its own window today. Tabbed view + a "recent files"
strip would help when bouncing across 4-5 related drafts. Additive — wait
until used in anger.

## Source-mode toggle

CodeMirror layer beside Crepe for power-edit (raw markdown). Reserve a hotkey
(Cmd+/) for the toggle. Crepe round-trip is good enough for v1; source-mode
is a polish, not a need.

## Image paste / drag-drop

Crepe handles inline images via URL but not local-file embedding. v2 should
wire paste/drop → write into a `_attachments/` sidecar dir → insert relative
link. Channel-scoped attachments folder is the natural anchor.

## Wiki-links / backlinks

Out of scope. Would compete with the file-tree-as-navigation primitive
(channel directory is the navigation surface). Revisit only if multi-file
linking inside a channel becomes load-bearing.

## "Is this stamped?" indicator

Stamp button is always shown. v1 doesn't detect whether the loaded file is
_already_ stamp-embedded. Detect via `sec verify --json` and render a small
"stamped on 2026-05-17 by did:..." badge in the titlebar when present.

## Pending-opens drain from markdown windows

Currently only the `main` window drains `PendingOpens`. If the user closes
`main` (hide-on-close, but possible) and only a markdown window is open, a
new `open -a` won't surface the file. Fix by also draining from the markdown
window mount.

## File watcher for external changes

If VS Code / `sec stamp` edits the file while a window is open, the user
sees stale content until they re-open. v2: watch the path, prompt to reload
on external change.
