# Secretariat as default macOS markdown reader/editor

**Status:** Draft (brainstorm output, pre-implementation)
**Date:** 2026-05-17
**Author:** Rafa + Claude
**Bounded context:** Secretariat Tauri shell

## Goal

Make Secretariat the principal's default app for opening and editing `.md` files
on macOS — replacing VS Code for the *read* path and providing enough editing
power to live in instead of Obsidian for ad-hoc markdown work. Each file opens
in its own fullscreenable window. Front-matter is rendered as a structured form
above the body. The stamp ceremony is one click away from any open file.

## Why now

- The Tauri shell already exists (v0.4.5) and ships a tray + sidecars + MCP wiring.
- Stamp ceremony (CLI + MCP) is shipped — UI just needs to invoke it.
- Reading `.md` files is the principal's highest-frequency action across the
  Secretariat / Themia / book corpus; VS Code is overkill and visually noisy.
- Channel directories already *are* markdown trees — a native reader closes the
  feedback loop between "navigate the channel" (forthcoming) and "read an envelope".

## Non-goals (v1)

- Vault navigator / file tree (each window is independent)
- Wiki-links / backlinks (Obsidian-style `[[link]]`)
- Image paste / drag-drop embedding
- Source-mode toggle (CodeMirror layer)
- Windows / Linux file association
- iCloud / Dropbox sync awareness
- Splitscreen / tabs
- Plugin system for editor extensions

## Editor library: Milkdown + Crepe

Rationale matrix:

| Lib | Fit | Decision |
|---|---|---|
| **Milkdown/Crepe** | Typora-like WYSIWYG out of box. Plugin-driven (ProseMirror + remark). Frontmatter plugin exists. React adapter. Precedent: MarkBun, Kuku. | **Pick.** |
| TipTap | Headless ProseMirror. Higher build cost for Typora-feel and FM UI. | Reject. |
| CodeMirror 6 | Source-only. | Reject for primary; reserve for future source-mode toggle. |
| Lexical | Powerful but markdown round-tripping is not first-class. | Reject. |
| @uiw/react-md-editor | Side-by-side preview, not WYSIWYG. | Reject. |

Crepe ships as `@milkdown/crepe` and renders into a DOM element; we mount it
inside a React effect with a `useRef`-bound div. Markdown round-trips via
Milkdown's remark serializer.

## Architecture

### Window topology

Three Tauri webview entry points:

1. **`main`** (existing) — review surface, navigator.
2. **`quick-pane`** (existing) — tray dropdown.
3. **`markdown`** (new) — one per opened file, label `md:<sha1(path)[..12]>`.

Each `markdown` window:

- Loads `markdown-window.html` (Vite multi-entry).
- Receives `file_path` via URL search param or initial Tauri event.
- Resizable, fullscreenable, has standard title bar.
- Minimum size 600×500; default 900×700.

### Frontend layout (per markdown window)

```
┌─────────────────────────────────────────────────────────────┐
│ ◀ ▶   Pretty Title (from FM / # H1 / filename)    Stamp ▸   │
├─────────────────────────────────────────────────────────────┤
│ ▾ Frontmatter                                                │
│    title:  [text input]                                      │
│    date:   [date picker]                                     │
│    tags:   [chip list]                                       │
│    draft:  [toggle]                                          │
│    <custom string>:  [text input / multiline]                │
├─────────────────────────────────────────────────────────────┤
│   (Crepe WYSIWYG editor — body only, FM stripped)            │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

Title resolution: `fm.title` → first `# H1` in body → file basename without extension.

### Frontmatter UI behaviour

- Parse on open with `gray-matter` (~2kb gzipped, mature, YAML).
- Render rows by inferred value type:
  - string (single line) → text input
  - string (multiline / contains `\n`) → textarea
  - ISO date → date picker
  - boolean → switch
  - array of scalars → chip list with `+ add`
  - nested object → JSON code block (read-only fallback)
- Unknown keys preserved verbatim on save.
- Collapsible (default: collapsed if FM has ≥ 5 keys, expanded otherwise).
- Edits debounce-save 800ms after last change.

### File I/O

- Tauri command `read_markdown(path) -> { frontmatter, body, sha256, mtime }`.
- Tauri command `write_markdown(path, frontmatter, body, expected_sha256)` —
  pessimistic concurrency: refuse if disk sha256 ≠ expected.
- All file IO lives in `src-tauri/src/markdown/` (new module). Frontend never
  hits `tauri-plugin-fs` directly for markdown bodies — single source of truth.

### macOS file association

Three pieces:

1. **`tauri.macos.conf.json`** — `bundle.fileAssociations`:
   ```json
   {
     "fileAssociations": [
       {
         "ext": ["md", "markdown", "mdown", "mkd"],
         "name": "Markdown",
         "description": "Markdown document",
         "role": "Editor",
         "rank": "Owner",
         "mimeType": "text/markdown"
       }
     ]
   }
   ```
   This populates `CFBundleDocumentTypes` in the bundled `Info.plist`.

2. **Rust `RunEvent::Opened` handler** — `src-tauri/src/main.rs`:
   - On `RunEvent::Opened { urls }`, parse file URLs, append to
     `AppState::pending_opens: Mutex<Vec<PathBuf>>`.
   - On main window ready (or on next single-instance invocation), drain the
     queue and spawn a markdown window per path.
   - The event-order trap (Opened before Ready) is documented;
     `AppState::pending_opens` is the buffer that bridges it.

3. **Single-instance plugin** — already a Tauri-standard pattern. New-instance
   callback receives argv, parses paths, calls the same `open_markdown_window`
   command.

User flow to set as default: first-launch onboarding card says "Make Secretariat
your default markdown app", with a button that uses macOS `LSSetDefaultRoleHandlerForContentType`
via a tiny Rust helper (or falls back to a "Finder → Get Info → Open With →
Change All" tooltip).

### Window spawn flow

Frontend Tauri command:

```rust
#[tauri::command]
async fn open_markdown_window(app: AppHandle, path: PathBuf) -> Result<String> {
    let label = format!("md:{}", sha1_hex(&path)[..12]);
    if let Some(existing) = app.get_webview_window(&label) {
        existing.set_focus()?;
        return Ok(label);
    }
    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(
        format!("markdown-window.html?path={}", urlencoded(&path)).into()
    ))
    .title(&pretty_title(&path))
    .inner_size(900.0, 700.0)
    .min_inner_size(600.0, 500.0)
    .build()?;
    Ok(label)
}
```

### Stamp integration

Toolbar button in window titlebar area:

1. Click → modal opens.
2. Modal renders full body verbatim (per AGENTS rule #4).
3. Principal types/confirms — explicit consent in same turn.
4. Frontend calls `sec stamp <path>` via the existing CLI sidecar
   (`tauri-plugin-shell` or direct `Command::new`).
5. Touch ID dialog (existing flow) — reason string carries first-line headline +
   short hash prefix.
6. On success: window re-reads file (stamp now embedded), shows "Stamped" toast.
7. On abort/mismatch: toast error, file unchanged.

Stamp button visibility: shown for all `.md` files. v1 doesn't try to detect
"is this already stamped" — that's a v1.1 polish.

## Component breakdown

New files:

```
src/
  markdown-window-main.tsx           # React entry for markdown windows
  markdown-window.html               # Vite multi-entry HTML
  components/
    markdown/
      MarkdownWindow.tsx             # Top-level layout
      FrontmatterPanel.tsx           # Structured FM form
      FrontmatterField.tsx           # Per-key row, type-dispatched
      CrepeEditor.tsx                # Milkdown/Crepe React wrapper
      MarkdownTitlebar.tsx           # Pretty title + stamp button
      StampDialog.tsx                # Verbatim-body confirmation modal
  lib/
    markdown/
      parse.ts                       # gray-matter wrapper
      title.ts                       # title resolution rules
      stamp.ts                       # IPC to sec stamp

src-tauri/src/
  markdown/
    mod.rs                           # module root
    commands.rs                      # read_markdown, write_markdown, open_markdown_window
    pending.rs                       # AppState::pending_opens buffer
    open_event.rs                    # RunEvent::Opened handler

src-tauri/
  tauri.macos.conf.json              # fileAssociations addition
  capabilities/markdown.json         # capability scoped to md windows
```

Touched files:

```
src-tauri/Cargo.toml                 # add sha1, urlencoding (likely already in)
src-tauri/src/main.rs                # wire RunEvent + single-instance
src-tauri/src/lib.rs                 # register markdown module commands
src-tauri/tauri.conf.json            # add markdown-window.html to multi-window list
vite.config.ts                       # multi-entry build
package.json                         # add @milkdown/crepe, @milkdown/react, gray-matter
```

## Data flow

```
.md on disk
    │
    ▼ read_markdown (Rust → JS)
{ frontmatter: object, body: string, sha256, mtime }
    │
    ├─▶ FrontmatterPanel (form state)
    │       │
    │       └─▶ debounced merge ─┐
    │                            ▼
    └─▶ CrepeEditor (body state) ▶ write_markdown(path, fm, body, sha256)
                                       │
                                       ▼ atomic rename, fsync
                                   disk + new sha256 → state.sha256

stamp click ──▶ StampDialog (verbatim) ──▶ sec stamp <path> ──▶ Touch ID
                                                                 │
                                                                 ▼
                                                         file re-read, toast
```

## Error handling

- **File doesn't exist on open** → toast, close window.
- **YAML parse error** → render FM as raw text in a textarea with "Couldn't parse
  frontmatter — edit raw" banner; body still loads.
- **Concurrent disk change** (sha mismatch on save) → modal "File changed on
  disk: reload / overwrite / cancel".
- **Stamp failure** → toast with error, no file change.
- **Window-spawn failure** → fall back to opening file in `main` window with
  warning (post-MVP — v1 just toasts).

## Testing

- **Unit:** `parse.ts`, `title.ts`, frontmatter type inference (Vitest).
- **Rust unit:** `markdown/commands.rs` round-trip (`tempfile`), `pending.rs`
  buffer drain semantics.
- **Integration:** open Tauri dev, simulate `RunEvent::Opened` via test harness,
  assert window spawn. (Tauri integration tests are limited — manual smoke
  + a thin e2e using the existing Playwright setup if present.)
- **Manual:** open `.md` from Finder; from `open` CLI; from drag onto Dock icon;
  full-screen toggle; stamp on a file in a Secretariat outbox.

## Rollout

- Ship behind no feature flag — additive surface, doesn't change existing
  flows.
- v0.5.0 release line.
- Onboarding card on first launch post-update offering "Set Secretariat as
  default markdown app".

## Open trade-offs resolved with defaults

| Question | Default | Reversible? |
|---|---|---|
| One window per file vs tabs | One window | Yes (tabs are additive later) |
| Autosave vs save-on-blur | Autosave debounced 800ms | Yes (config later) |
| FM placement | Collapsible above body | Yes (sidebar variant later) |
| Read-only vs always editable | Always editable; stamp gates durability | Yes |
| Source-mode toggle | Not v1 | Yes (CodeMirror layer additive) |

## Risks

- **Milkdown/Crepe stability with frontmatter** — the FM plugin is community-
  maintained. Mitigation: parse FM in JS (gray-matter), keep Crepe focused on
  body only. FM is *not* round-tripped through Crepe.
- **macOS "Open With" event-ordering** — covered by `AppState::pending_opens`.
- **`sec stamp` non-existence of file in Secretariat path** — v1 assumes any
  `.md` path is stampable; the CLI's existing checks handle envelope-shape
  validation and error if not. UI surfaces the error.
- **Default-app land grab** — we declare `Owner` rank but explicitly do NOT
  auto-set ourselves as default. User opts in via onboarding card.

## References

- Milkdown Crepe — https://milkdown.dev/docs/guide/using-crepe
- MarkBun (FM precedent) — https://github.com/xiaochong/markbun
- Tauri file associations — https://v2.tauri.app/distribute/macos-application-bundle/
- macOS UTI for markdown — `net.daringfireball.markdown`
- AGENTS.md rule #4 (stamp ceremony) — show body verbatim, explicit consent
