# Idea — left-hand TOC in the editor

Raw capture — 2026-06-04. Floated during the editor-reader redesign.

A table of contents on the **left-hand side** of the markdown editor, derived
from the document's headings (`h1`–`h3`). Click a heading → scroll to it. The
left side is currently empty space next to the centered 85ch column — a natural
home for navigation that doesn't crowd the writing surface.

## Why

- Long documents (contracts, decisions, the book chapters) are hard to navigate
  in a single scroll. A TOC makes structure legible and jumpable.
- The redesign moved trust to the footer and frontmatter above the body, leaving
  the left gutter free. A TOC fills it without re-introducing a heavy sidebar.

## Open questions

- **Source:** parse headings from the body markdown (cheap, already have
  `parse.ts`) vs. read from Crepe's doc model (live, reflects edits).
- **Sync:** highlight the active section on scroll (scroll-spy) — nice but more
  code. v1 could be a static jump-list.
- **Anti-compulsion fit:** must stay quiet/peripheral — a faint list that's
  ignorable, not a persistent panel demanding attention. Fade when not hovered?
- **Where exactly:** floating in the left margin of the centered column, or a
  thin fixed rail? On narrow windows it collapses / hides.
- **Empty/short docs:** hide entirely below N headings.

## Relation

Sibling to the editor-reader redesign (single Compose surface). Not part of that
PR — deferred here so the redesign can land first.

Don't shape yet.
