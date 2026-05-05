# Project — Menubar + quick-pane (v0.3)

Pairs with `docs/pitches/2026-05-05-menubar-only.md`. The pitch sets the
JBTD + boundaries; this doc sequences the work into vertical slices.

## Sequencing

Five slices. Each is independently shippable and demoable. Stop at any
slice if the appetite runs out — the partial release is still useful.

### Slice 1 — Tray icon + minimal dropdown (1/2 day)

The tray exists. Click → small popover with the same two-button home,
"Sync now", "Settings…". Main window keeps showing for now. No lifecycle
changes. Just adds the tray as a *parallel* affordance.

**What changes:**
- `src-tauri/Cargo.toml` — add `tauri = { features = ["tray-icon"] }`.
- `src-tauri/src/lib.rs` — `setup` hook builds a `TrayIconBuilder` with
  a static SVG icon. Click event opens an existing hidden window
  positioned under the tray icon.
- `src/components/secretariat/TrayPopover.tsx` — wraps `<ReviewSurface>`
  in a small popover layout (no header, condensed spacing).
- `src-tauri/tauri.conf.json` — new window definition `tray-popover`
  (transparent, frameless, NSPanel-style).
- `src-tauri/src/capabilities/default.json` — windows array gains
  `tray-popover`.

**Demo:** click menubar icon → popover slides down → Review-inbox /
Review-outbox buttons + Sync now → click outside dismisses. Main window
still opens normally on launch (slice 2 hides it).

### Slice 2 — Lifecycle: hide main window after onboarding (1/4 day)

The main window only shows during onboarding. Post-onboarding, the
app launches menubar-only.

**What changes:**
- `src-tauri/src/lib.rs` `setup` hook — check
  `current_identity` + `get_profile`; if both present, hide main
  window before showing it. If either missing, show normally (wizard
  flow runs).
- `src/components/layout/MainWindowContent.tsx` — when state is
  'ready', emit a `window:onboarding-complete` event. The Rust side
  listens, hides the main window, future opens go through tray
  popover.
- `lib.rs` close-requested handler — quit on tray-icon-only mode
  (Cmd+Q) instead of just hiding.

**Demo:** launch fresh → wizard appears (main window) → finish
onboarding → main window closes → only tray icon remains. Subsequent
launches: tray icon only, click to open popover.

### Slice 3 — Tray badge + ambient color dot (1/4 day)

Tray icon shows a colored status dot reflecting queue state. Updates
when sync runs or quick-pane captures land.

**What changes:**
- `src-tauri/src/commands/secretariat.rs` — new helper
  `compute_tray_state() -> TrayState { all_clear | pending }` reads
  inbox + queue counts.
- `src-tauri/src/lib.rs` — periodic call (15-min same as sync) updates
  the tray icon's image with a green or amber dot variant. Static
  dot-glyph SVGs in `icons/`.
- A Tauri event `tray:state-changed` so the React popover can also
  reflect (button border colors etc).

**Demo:** receive an envelope (sync brings it in) → tray icon dot
turns amber. Stamp the queue empty → goes green.

### Slice 4 — Ideas pool data model + capture infrastructure (1 day)

The ideas pool — pre-envelope captures, no recipient/body shape required.
Domain value object + storage + use cases. No UI yet.

**What changes:**
- `crates/core/src/domain/idea.rs` — `Idea { id, captured_at, body,
  suggested_to: Option<String> }` value object.
- `crates/core/src/infrastructure/idea_store.rs` — JSON-per-file
  storage at `~/.secretariat/ideas/<timestamp>.json`. Atomic write.
- `crates/core/src/application/idea_ops.rs` — use cases:
  `capture_idea`, `list_ideas`, `delete_idea`,
  `promote_idea_to_envelope` (creates an outbox draft from an idea +
  removes the idea from the pool). Tests for each.
- `crates/cli/src/commands/idea.rs` — `sec idea capture <body>`,
  `sec idea list`, `sec idea promote <id>`. Power-user surface.
- MCP tools: `idea_capture`, `idea_list`, `idea_promote`. Same
  shape, exposed to Claude.

**Demo:** `sec idea capture "tell dad chapter 3 needs more pressure"` →
file lands in `~/.secretariat/ideas/`. `sec idea list` shows it. Inside
Claude: `Use idea_promote to draft this as an envelope to dad` →
envelope appears in outbox queue.

### Slice 5 — Quick-pane wired to capture (1 day)

The template's existing quick-pane scaffolding gets repurposed as the
ideas-capture surface. Global shortcut summons it from anywhere.

**What changes:**
- `src/components/quick-pane/QuickPaneApp.tsx` — replace template
  content with a single text field + optional contact-picker
  dropdown + "Capture" button.
- New Tauri command `idea_capture_from_pane(body, suggested_to)` —
  thin wrapper over the application use case.
- `src-tauri/src/commands/quick_pane.rs` — keep the existing show /
  dismiss / shortcut machinery; just point at the new content.
- Default shortcut changed from `Cmd+Shift+.` to `Cmd+Shift+S`
  in `src-tauri/src/types.rs` (`DEFAULT_QUICK_PANE_SHORTCUT`).
- Tray-popover gains a "Capture an idea" button as alternative entry
  point.

**Demo:** any window focused → `Cmd+Shift+S` → quick-pane appears →
type "ping Christophe re: deal" → Enter → pane dismisses → the idea
is in the pool, surfaces in next outbox-review session.

## Out of slice scope

- The cadenced review walker (separate pitch — `two-buttons-cadenced-reviews`).
  Buttons in the popover keep their copy-prompt-to-clipboard behavior.
- Voice input on the quick-pane. Typing only.
- Scribe-auto-capture (the AI watching ambient context to drop ideas
  into the pool unprompted). v0.4+ when cognition ports mature.
- Windows / Linux tray support. macOS-only Day 1.
- Multi-monitor popover positioning edge cases.

## Decision log

- **Tray + quick-pane in one project, not two.** Both ride on
  existing template scaffolding (`tauri-nspanel`, `quick_pane.rs`),
  both serve the "stay out of the way" thesis, both are summoned
  surfaces. Splitting them would duplicate lifecycle work.
- **Ideas pool is a new domain value object, not an unstamped
  envelope variant.** Ideas have no `to:`, no body shape, no stamp —
  they're notes-to-the-AI-assistant, structurally distinct from
  envelopes. Forcing them into the envelope shape would corrupt that
  type's invariants.
- **Default shortcut `Cmd+Shift+S`** ('S' for "say"). Template's
  `Cmd+Shift+.` was generic; Secretariat-context wants something
  semantic.
- **Slice 4 before slice 5 strictly.** The ideas pool data model has
  to exist before the quick-pane has somewhere to write to.
  Otherwise we'd build UI against an empty backend.

## Success criteria

- Tray icon visible on every launch post-onboarding; main window
  invisible.
- Tray dot accurately reflects queue state within 15 min of any
  change (sync cadence).
- `Cmd+Shift+S` from any window summons the quick-pane in <200ms.
- Captured idea appears in the outbox-review session within one sync
  cycle.
- All five slices land within one focused week (the `big` appetite
  from the pitch).
