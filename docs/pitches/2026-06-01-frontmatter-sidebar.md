---
tag: pitch
appetite: small
status: draft
source: docs/ideas/2026-05-31-frontmatter-sidebar.md
supersedes: []
slice_id: A
---

# Pitch — Frontmatter sidebar as a real property editor

**Bet:** Rework the right-hand frontmatter sidebar into a stacked, fully-editable property panel — labels above fields, add/remove keys, type-aware inputs — and make it open by default so frontmatter is reachable from any open document.

**Why it matters:** Docs carry their `type`/`tags`/`status` metadata in frontmatter, but today it's view-mostly, cramped, and hidden behind a toggle. The principal can't reliably reach or edit it from the main window. The sidebar is the place metadata gets curated without polluting the prose.

---

## Boundaries

**JBTD:** As the principal editing a doc in the markdown window, I want to read, edit, add, and remove every frontmatter field from a clear sidebar, so I can curate `type`/`tags`/`status` without hand-editing YAML or cluttering the prose. Baseline today: the panel exists (`FrontmatterPanel.tsx`) but labels sit cramped to the left of a narrow control, there's no add/remove, and the sidebar is `defaultOpen={false}` + offcanvas — so from the main window frontmatter reads as "not there" (the access bug).

**Out:**
- A frontmatter schema/lexicon registry with validation. Type *inference* stays; declared per-`$type` schemas are a later slice.
- Editing `$`-prefixed protocol blocks (`$envelope`, `$attestation`). They stay read-only — the lexicon is authoritative.
- Any change to parse/serialize (`parse.ts`) or the wire format.

## Elements

- **Stacked field layout** (`FrontmatterField.tsx:28-38`). Drop the `w-32` left label; put the key label *above* the control, full-width. Each field becomes a labeled block, readable at sidebar width.
- **Add field** (`FrontmatterPanel.tsx:25-36`). A footer "+ Add field" row: key input + type picker (text/list/boolean/date/number) → inserts `{ [key]: emptyForType }` into frontmatter, fires existing `onChange`. Reuse `FieldType` from `field-type.ts`.
- **Remove + rename field** (`FrontmatterField.tsx`). Per-row delete (drops the key) and inline key rename (re-key the object, preserve value). Block both on `$`-prefixed keys.
- **Type override per field** (`FrontmatterField.tsx:27`, `field-type.ts`). Small per-field type selector so an inferred-`text` value can be forced to `list`/`date`/etc. Inference stays the default; override wins.
- **Sidebar open + reachable** (`MarkdownWindow.tsx:262-315`). Flip `defaultOpen` to true (or persist last state); keep the `PanelRight` toggle in `MarkdownTitlebar.tsx:107-114`. Closes the access-bug capture.

## Risks

**🐇 Rabbit holes:**
- Rename-as-rekey reorders object keys → YAML field order churns on serialize. Decide: preserve insertion order, or accept reorder. Keep it simple — don't build an ordered-map layer.
- Type override changing `list`↔`text` must coerce the value cleanly (string ↔ array), not drop data.
- Add-field with a duplicate or empty key — guard at the input, don't let it silently overwrite.

**🏴 Off-sides:** Per-`$type` schema-driven fields (required keys, enum dropdowns for `status`). Tempting once the type picker exists — defer to the schema slice.

**🥩 Fat cut:** Drag-to-reorder fields. Nice, not load-bearing for "edit + add + remove."

**🧪 Domain knowledge:** Confirm the access bug is discoverability (`defaultOpen={false}`), not a mount failure in tab mode — both `markdown-window-main.tsx` and `SessionTabs.tsx:203` render the same `SidebarProvider`, so opening by default should fix it. Verify in the embedded tab path.

## Acceptance

1. Opening any doc shows the frontmatter sidebar without clicking the toggle (or restores the last-used open state).
2. Each non-`$` field renders its key as a label *above* a full-width control.
3. "+ Add field" with a key + type inserts a new editable field; it persists to disk on autosave.
4. A per-row delete removes the field from frontmatter and from disk on autosave.
5. Renaming a field key preserves its value; `$`-prefixed keys cannot be renamed, removed, or edited.
6. A per-field type override switches the control (e.g. text→list) and coerces the value without data loss.
7. `$envelope`/`$attestation` blocks remain collapsed and read-only.

---

_Drafted by Claude (scribe). Subsumes the `2026-05-31-frontmatter-access-bug.md` capture._
