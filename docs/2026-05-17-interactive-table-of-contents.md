---
migrated_from: equanimi.tech/project/secretariat/editor/ideas/20260517T192001Z-hafvps.md
---

# Interactive Table of Contents

When viewing/editing a markdown doc in Secretariat's editor, show a navigable TOC sidebar — clickable headings, current section highlighted as the viewport scrolls, collapsible nested headings.

ProseMirror gets us section boundaries cheaply (Crepe is ProseMirror-based). TOC is a side panel that mirrors the document outline; click-to-scroll, scroll-to-highlight.

Open questions:

* Side panel left or right? (Right matches Typora; left matches IDE muscle memory.)

* Toggle vs always-on? (Toggle — many docs are short.)

* Persist toggle per-window or globally?

Implementation: shadcn `sidebar.tsx` + `scroll-area.tsx`. Same layout primitive as the frontmatter sidebar idea — likely both share a `EditorRail` shell.
