# Post-teardown window model — main window, sidebar, quick captures

**Date:** 2026-06-09
**Status:** design (awaiting principal review → plan)
**Scope:** Tauri shell window architecture only. No domain/lexicon changes.

## Why

After the v0.12 git-native teardown, the Tauri shell still carries the skeleton
of the cut correspondence app. Three surfaces drifted out of sync with the
current bounded context (_markdown editor + stamp_):

1. **Quick pane** — a dead placeholder. The global hotkey still pops an NSPanel,
   but `QuickPaneApp.tsx` is a stub (`"Open Secretariat to edit and stamp…"`).
   Its capture/launch flow was cut and the backing commands are gone. A
   hotkey to nothing.
2. **Main window** — right function, fossil name. It is a tabbed markdown editor
   (`LeftSideBar` file tree + a tab strip of embedded `MarkdownWindow`s), but the
   tab manager is still `SessionTabs` in `components/sessions/` — naming from the
   cut Claude-sessions era.
3. **Dual editor surfaces** — the same `MarkdownWindow` renders two ways
   (embedded tab vs standalone window) with no rule, and a cold doc-open drags
   the full main shell along.

## Decisions (settled with the principal)

- **Window model = tabbed workspace + focused open.** Main is the deliberate
  workspace (explorer + tab strip). A cold "open this doc" always spawns a
  standalone focused doc window. Two render paths, each with a clear trigger.
- **Quick pane = retire it.** Remove the dead placeholder, the global shortcut,
  and the NSPanel wiring entirely — no hotkey-to-nothing. The real launch/
  dispatch mouth returns as its own pitch when its verbs exist (see Out of scope).
- **Cleanup is in scope.** Rename the `sessions` fossil, delete the orphan
  `RightSideBar`, widen editor tables.

---

## Part 1 — Window model + the "don't flash main" fix

### Behavior contract

- **Main starts hidden.** It is shown only when Secretariat is opened *itself*
  with no doc — dock click, tray "Show Secretariat", or a bare
  `open -a Secretariat`.
- **Open a doc → only the doc window.** `sec view`, Finder "Open With", argv,
  and any future deep-link spawn a standalone focused doc window. Main never
  tags along.
- **Inside main, sidebar-click → embedded tab.** That is the one place tabs live.

This gives the two render paths a clean rule (they are already one
`MarkdownWindow` component with an `embedded` flag):

| Trigger | Surface |
|--|--|
| `sec view` / Finder "Open With" / argv / deep-link | standalone doc window |
| sidebar-click inside main | embedded tab in main |

### Changes

- `src-tauri/tauri.conf.json` — main window `visible: false`.
- `src-tauri/src/lib.rs` `setup()` — main stays hidden at boot. Surface main
  only via the existing deliberate triggers (`RunEvent::Reopen`, tray
  `tray-show`, single-instance no-arg fallback) **plus** a new bare-launch path.

### Cold-start race (implementation note for the plan)

On macOS a doc-open arrives via `RunEvent::Opened` (Apple event), which fires
*after* `setup()` — and `open -a` passes no argv — so at `setup()` time we
cannot distinguish a bare launch from a doc launch synchronously.

Chosen approach: keep main hidden at `setup()`, then schedule a short deferred
check (~300–500 ms) that surfaces main **only if** no doc window was spawned in
the meantime (PendingOpens drained empty **and** no `md:*` webview window
exists). `RunEvent::Opened` fires near-immediately on cold start, so the doc
window wins the race; a genuinely bare launch sees nothing and falls through to
showing main. The plan firms up the exact mechanism (timer vs first-idle tick).

### Acceptance

- `sec view <doc>` on a cold app → exactly one window (the doc), main hidden.
- `sec view <doc>` while main is open → doc window spawns; main untouched.
- Bare `open -a Secretariat` (no file) / dock / tray → main shows.
- Quitting + relaunching never auto-resurrects a previously-visible main
  (VISIBLE already stripped from window-state) — unchanged.

---

## Part 2 — Honest cleanup

- **Rename the fossil.** `components/sessions/` → `components/workspace/`;
  `SessionTabs` → `WorkspaceTabs`. `types.ts` / `storage.ts` move with it.
  Update the import in `components/layout/MainWindowContent.tsx`. Keep the
  `OPEN_MARKDOWN_EVENT` constant name and the localStorage tab key unchanged so
  open tabs survive the rename (or accept a one-time reset — plan decides).
- **Delete the orphan.** `components/layout/RightSideBar.tsx` (defined, never
  mounted; frontmatter lives inside `MarkdownWindow`). Remove its `index.ts`
  export.
- **Widen editor tables.** Bump the table max-width in the editor CSS
  (`src/components/markdown/markdown-editor.css`) per the
  `docs/2026-06-08-ui-pains-backlog.md` note.

### Acceptance

- App builds; main window opens a doc in a tab from the sidebar as before.
- No dangling imports of `SessionTabs` / `RightSideBar`.
- A wide table in a doc uses the new max-width.

---

## Part 3 — Retire the quick pane

Remove the pane and all its wiring. Surfaces to delete or unwire:

**Frontend**
- `src/quick-pane-main.tsx`, `src/quick-pane.css`,
  `src/components/quick-pane/QuickPaneApp.tsx`, `quick-pane.html`.
- `vite.config.ts` — drop the `quick-pane` rollup input.
- `src/components/preferences/panes/ShortcutPane.tsx` — the pane existed to bind
  the quick-pane shortcut; remove it and its registration in the preferences
  pane list / `GeneralPane` reference.
- `src/lib/bindings.ts`, `src/services/preferences.ts`,
  `src/store/ui-store.ts`, `src/hooks/useMainWindowEventListeners.ts`,
  `src/components/ThemeProvider.tsx`, `src/test/setup.ts` — strip quick-pane
  references (toggle command, shortcut state, test stubs).

**Rust**
- `src-tauri/src/commands/quick_pane.rs` — delete.
- `src-tauri/src/commands/mod.rs` — drop `pub mod quick_pane;`.
- `src-tauri/src/bindings.rs` — drop the five quick-pane command registrations.
- `src-tauri/src/lib.rs` — remove `init_quick_pane`, shortcut registration, the
  tray quick-pane toggle branch, the window-state `quick-pane` denylist entry,
  and the `RunEvent::Exit` panel-hide block.
- `src-tauri/src/types.rs` — drop `quick_pane_shortcut`.
- `src-tauri/src/commands/preferences.rs` — drop `load_quick_pane_shortcut`.
- `src-tauri/src/commands/secretariat.rs` — strip any quick-pane reference.
- `src-tauri/capabilities/quick-pane.json` — delete.

> Keep the global-shortcut plugin itself (used elsewhere / future), but
> unregister the quick-pane binding.

### Acceptance

- No `quick_pane` / `QuickPane` references remain (grep clean).
- The former hotkey does nothing; app launches without creating an NSPanel.
- `cargo build` + `pnpm build` succeed; bindings regenerate without the
  removed commands.

---

## Out of scope (future pitch) — the real capture mouth

The launch/dispatch mouth from `docs/ideas/2026-05-31-improve-quick-capture.md`
is **not** in this slice. It is gated on two unbuilt verbs:

- **`sec launch` repo-registry rewire.** `crates/cli/src/commands/launch.rs` is
  still channel-era (resolves `root_path` from
  `~/.secretariat/<alias>/channel/.../contract.local.md`). It must resolve from
  the `[[repos]]` registry.
- **Headless repo-dispatch verb.** The existing `dispatch_compose`/
  `dispatch_send` commands are **Slack-forward** (seal → Slack message from a
  doc), not the "fire background work into a repo" verb. That verb is unbuilt.

When both land, the pane returns as a repo picker → launch/dispatch mouth. Its
own spec.

## Risks

- **Bare-launch race** (Part 1): if the deferred check is too short, a slow
  cold start could flash main before the doc window registers. Mitigate with a
  conservative delay + the doc-window-exists guard.
- **Double-open** (Part 1): `sec view`-ing a doc already open as a tab in main
  yields two live editors on one file. Write path has sha256 conflict
  detection, so it is safe, not pretty. Acceptable for now; dedupe later.
- **Tab persistence** (Part 2): renaming the storage key would drop open tabs;
  keep the key to avoid a reset.

## Quality gates

`cargo test --workspace`, `cargo clippy -- -D warnings`, `pnpm build`,
`pnpm test` all green before claiming done.
