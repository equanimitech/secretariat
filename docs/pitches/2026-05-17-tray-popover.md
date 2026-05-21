# tray popover — primary verbs at glance, native menu as fallback

Pitch — 2026-05-17. Source: principal feedback on the v0.4.3 tray:

> "Right now we right-click and see Show and Quit. When we click on it
> it just shows the page. I wonder if we shouldn't think of an
> intermediary experience a bit like LM Studio or TomatoBar (in form,
> not necessarily in content)."

Today's tray (shipped in v0.4.3) is a native macOS menu — fine for
"Show / Quit," boring for everything else. Native menus can't render
a prominent action button, can't show status, can't host a typeahead.
The principal wants the menubar icon to be **the primary surface** for
the daily verbs, not a window-summoner.

## Boundaries

### Job to be done

As a principal whose Secretariat runs as a background daemon (no Dock
icon, no Cmd+Tab presence per v0.4.3's Accessory mode), I want the
menubar icon to be **my interactive control panel** — one-click access
to the three motions I make most (review, launch, surface the
window), plus a quiet status block telling me the daemon is alive —
so I never have to fully "open the app" just to dispatch a verb.

_When_: many times a day. The tray is _the_ surface I have constant
access to; the main window is something I open deliberately.
Baseline today: right-click → "Show Secretariat" → window opens →
click the Review-themia.pro button → terminal opens. That's three
gestures for one action.

### Appetite

`medium`

NSPanel positioning + animation + React popover content + status
polling. Each piece is small but five surfaces interact (Rust tray
positioning, React popover, status command, dismiss-on-blur, native
menu fallback). Cuts can land slice by slice if appetite tightens
(see fat-cut below).

## Elements

Four primary elements. The popover replaces the native menu's
_left-click_ behavior; right-click keeps the native menu as a fallback
(macOS convention + reliability under launcher edge cases).

### 1. NSPanel popover anchored to the tray icon

Reuses `tauri-nspanel` (already a dep — the quick-pane uses it).
Window label: `tray-popover`. Created hidden on app startup, shown
on tray left-click, dismissed on blur (same contract as the
quick-pane).

Positioning: anchor to the tray icon's screen rect via Tauri's
`get_position()` + a small downward offset. Initial dimensions:
~320×280px. Resize if status block grows.

### 2. Primary verb — "Review (everything)"

Big button at the top of the popover. Single click runs `claude
--agent review` in the principal's chosen terminal with cwd =
`~/.secretariat/` (substrate root, not per-vault — per the principal's
"review on everything" directive). Reuses the
`review_org` Tauri command with alias `_self` (the existing private
path resolves to `paths.root`, which IS the substrate root).

Visual: matches the OrgPicker row style (big, full-width, hover
highlight). Cmd+R while popover is focused fires it.

### 3. Status block

Ambient, read-only. Two lines:

```
Daemon · running (last poll 12 min ago)
3 envelopes pending stamp
```

The daemon-running line reads `~/.secretariat/.daemon-pid` (LaunchAgent
writes this) and a fresh-enough mtime on a sentinel file the daemon
touches on each poll tick. The pending-stamps line counts
`list_review_queue().filter(!stamped)`. Both refresh on every popover
show (no background polling — the popover is short-lived).

Anti-compulsion: no last-sync timestamp displayed when it's < 1 min
old (no real-time gloss). No notification badges on the tray icon.

### 4. Secondary verbs

Two rows beneath the status block:

- **Launch Secretariat** — opens the quick-pane (the cmdk launcher
  shipped in v0.4.8). Same `show_quick_pane` Tauri command as the
  capslock shortcut. Discoverable backup for principals who don't
  remember the global hotkey.
- **Show window** — surfaces the main window's OrgPicker. Same
  `surface_main_window` helper as the existing native menu.

Both render as cmdk `<CommandItem>` rows (no input — they're a static
two-item list). Keyboard: ↑↓ to move, Enter to fire, ⌘ shortcuts as
captioned.

Footer (smaller, muted):

```
Preferences…    ⌘,
Quit Secretariat ⌘Q
```

### Right-click fallback

The current native menu (`Show Secretariat` / `Quit Secretariat`)
stays bound to right-click. Macs users expect it; if NSPanel
positioning misbehaves, it's the durable escape hatch.

## Risks

### 🐇 Rabbit holes

- **NSPanel positioning across multiple displays / DPI changes.** The
  tray icon's screen rect changes when the principal moves between
  laptop screen and external monitor. Re-resolve on every show, not
  once at startup. macOS's `NSScreen.main` flips when popover takes
  focus — capture the position pre-focus.
- **Click-outside dismissal vs focus-loss dismissal.** NSPanel's
  `becomesKey` event fires on the popover when it shows; we want to
  dismiss on losing key. The quick-pane has the same pattern — port
  its blur handler verbatim.
- **Status block timing.** Reading `~/.secretariat/.daemon-pid` is
  fast; counting pending-stamps requires a substrate walk. Cap the
  walk at 5000 envelopes for the count, surface "5000+" if hit.
  Better: cache the count in the daemon's poll loop and read the
  cached value.
- **Animation jank.** TomatoBar slides the popover in; LM Studio
  fades. NSPanel doesn't give us a free transition. Ship without
  animation v1; iterate if it bothers the principal.

### 🏴 Off-sides called

- Tray-icon badge with unread count. Anti-compulsion (AGENTS.md "no
  notifications / counts surfaced as score"). Status block has the
  number; the tray icon stays template-tinted.
- Drag-and-drop into the popover. Useful for compose-with-attachment
  later but compose is MCP-primary now; defer.
- Sync-now button. Removed by principal directive (see Reference). The
  daemon's 15-min cadence floor is the governor.
- Per-vault status. Principal said "review on everything" — the
  status block doesn't fragment by vault. Future review-cursor work
  (separate pitch) may add a "X unread since last review" line.

### 🥩 Fat cut

- Status block. Ship slice 1 (NSPanel + Review + secondary verbs)
  without the status; add the status block in slice 2 once the daemon
  exposes a `.daemon-pid` + counter.
- Animation. Ship v1 without; reassess if the popover-snap feels rude.
- Cmd+R / Cmd+L hotkeys while popover is focused. Slice 3 if at all.

### 🧪 Domain knowledge

- Confirm NSPanel can re-anchor to a moving tray icon. Tauri-nspanel
  uses native NSPanel under the hood; the position API is
  reset-on-show. Should be fine but verify.
- Confirm `tauri_plugin_global_shortcut` doesn't conflict with the
  popover's keyboard handling (it shouldn't — different scope).

## Pitch

### Problem

The tray exists as a daemon-mode artifact: app runs background, tray
is the only persistent surface. But today's tray is a one-trick
menu — left-click toggles the window, right-click shows two items.
Every action requires opening the window first. The popover-as-control-
panel pattern (TomatoBar, LM Studio, Things' menu bar) treats the
tray as _the_ surface; the window becomes the exception.

The three verbs the principal actually uses from the tray are: review,
launch, surface. All three exist as Tauri commands already. The work
is wiring them into a popover, plus a status block to keep the
principal oriented without opening anything.

### The bet

Build the popover on top of NSPanel (already a dep). Reuse the
quick-pane's blur-to-dismiss pattern. Hardcode three rows (Review,
Launch, Show) in slice 1, add the status block in slice 2. Leave the
native right-click menu in place — durability over uniformity.

The bet pays off when the principal hovers the menubar, sees "Daemon ·
running. 3 pending stamp" without clicking, decides "okay, I'll do
those now" — and one click is enough to start.

### No-gos

- No badge on the tray icon. Status lives inside the popover only.
- No notification surfacing. The daemon doesn't ping; the tray
  doesn't bleat. Status is pull, not push.
- No sync-now button. Daemon owns cadence.
- No "open last vault" shortcut. The principal goes through Review
  (which now operates on everything) — single canonical entry.

## Reference

- v0.4.3 ship note (tray icon + native menu)
- v0.4.6 ship note (OrgPicker + `review_org` Tauri command)
- v0.4.8 ship note (quick-pane cmdk launcher + `show_quick_pane`)
- `docs/pitches/2026-05-17-review-orchestration.md` — review cursor &
  agent work that the status block will eventually consume
- AGENTS.md rule on anti-compulsion / cadence floors
