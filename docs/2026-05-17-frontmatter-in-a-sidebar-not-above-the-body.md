---
migrated_from: equanimi.tech/project/secretariat/editor/ideas/20260517T192014Z-3f2274.md
---
# Frontmatter in a sidebar (not above the body)

Today: `FrontmatterPanel` renders horizontally between the titlebar and the Crepe body. Steals vertical real estate; visually competes with the document.

Better: move the FM fields into a collapsible right sidebar.

Components available (shadcn template):
- `sidebar.tsx` — full shadcn sidebar primitive (header/content/footer slots, collapsible)
- `resizable.tsx` — drag-to-resize splitter
- `sheet.tsx` — slide-over alternative if we want it modal
- `scroll-area.tsx` — for long FM lists
- `separator.tsx` — between field groups

Probable design: `Sidebar` on the right, `SidebarTrigger` in the titlebar (toggle), `resizable.tsx` only if we want manual width control (Notion-style — start without). FM fields stack vertically inside `SidebarContent`. Default state: open if doc has FM, collapsed if not.

Couples with TOC sidebar idea — should probably be the same rail with two tabs (Outline / Metadata), or two rails (TOC left, FM right). Lean two rails — fewer clicks, more obvious affordance.
