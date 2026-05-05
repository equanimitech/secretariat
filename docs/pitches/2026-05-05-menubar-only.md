# Menubar-only Secretariat — out of the way by default

Pitch — 2026-05-05. Source: `/Users/rafa/Developer/equanimitech/secretariat/docs/ideas/menubar-only-no-main-window.md`.

**Hard dependency:** the v0.2.x onboarding wizard remains the first-launch path. The menubar-only behavior kicks in *after* onboarding completes.

## Boundaries

### Job to be done

When I'm working in Claude Code, my editor, or anywhere else throughout the day, I want Secretariat to live in the periphery — never a window competing for screen space, never a dock icon I need to manage. I want a small menubar icon that ambient-signals when there's something to review, that I summon a focused review surface from with one click, and that disappears the moment the session ends. The MCP, daemon, and signing primitives keep running out-of-process so my AI assistant works regardless of whether I've summoned the surface.

Baseline today: Tauri ships with an always-visible main window. The ReviewSurface (two-button home) sits in that window from launch until quit. There's a dock icon. The window-state plugin saves position. Everything about the chrome assumes a long-running window.

### Appetite

`medium` (overridden — set explicitly by user). A couple of focused days. A small/tiny would skip lifecycle hardening; a big would invite scope creep into menubar-app feature parity territory.

## Elements

Four elements, breadboarded.

### Place: menubar tray icon

- **Place:** macOS system menubar (NSStatusItem via Tauri's `TrayIconBuilder`).
- **Affordance:** static glyph + ambient color dot indicating queue state (green = both empty, amber = something pending in either).
- **Connection:** left-click → opens dropdown panel. Right-click → menu (Quit, Open Settings, About).

### Place: dropdown panel

- **Place:** small popover anchored under the tray icon. Reuses the
  template's NSPanel scaffolding from `src-tauri/src/commands/quick_pane.rs`
  (already wired with `tauri-nspanel`, currently unused).
- **Affordance:** the same two big buttons (Review inbox / Review outbox)
  + counts. Plus a "Settings…" link and "Sync now" link.
- **Connection:** click Review inbox → spawn focused review window
  (placeholder for cadenced walker — copies prompt + toast for now).
  Click Settings → spawn separate Preferences window. Click Sync now →
  fires `sync_now`, badge color refreshes.

### Place: focused review window (ephemeral)

- **Place:** standalone window that opens *only* when a review session
  is in progress. Currently routed by MainWindowContent; on session end
  the window closes (not just hides — closes).
- **Affordance:** the cadenced walker (separate pitch) lives here.
- **Connection:** opens from dropdown click; closes on session end →
  dropdown can be re-opened from the menubar.

### Lifecycle: hidden by default

- App starts → NO main window shown. Tray icon installed. Daemon (already
  out-of-process LaunchAgent) keeps polling.
- First-launch only → main window IS shown for the onboarding wizard.
  Wizard completes → window closes → menubar-only thereafter.
- Quit (Cmd+Q from tray menu) → app fully exits, tray icon removed.
  Daemon LaunchAgent keeps running.

## Risks

### 🐇 Rabbit holes

- **First-launch detection.** "Has onboarding completed?" — boolean
  derived from `current_identity() != null && get_profile() != null`
  (already the existing routing test). Show main window only when both
  are missing. Easy.
- **Tray badge updates on inbox arrival.** Today the daemon polls every
  15 min in-process *inside the Tauri app*; without a window, does the
  app process keep running? Answer: yes — Tauri tray apps stay
  resident. The background sync loop in `lib.rs` already runs
  independent of window state. Tray reads the same state.
- **NSPanel popover anchoring.** The template's quick-pane uses
  NSPanel for floating-popover behavior. Reusing it for the tray
  dropdown means anchoring to the tray icon's screen position — Tauri
  exposes this via `TrayIcon::rect()`. ~30min spike.
- **Menubar-only on Cmd+Q.** macOS convention: closing the last window
  doesn't quit a menubar app — only Cmd+Q from the tray menu does.
  Need to override the existing window-close behavior in
  `lib.rs:152-175` to allow the close (currently prevents on close,
  hides instead).

### 🏴 Off-sides called

- **Windows / Linux** menubar UX. Tauri's tray works cross-platform but
  conventions differ (Windows = system tray with right-click; Linux =
  varies by DE). Out of scope; macOS-only Day 1 per AGENTS.md.
- **Custom rendered tray icon glyph.** A simple monochrome icon
  template is fine. Custom SVG with state animations is fat.
- **Dock icon hide/show toggling.** Could expose as a setting later
  ("show in Dock when window is open"). v0.3+.

### 🥩 Fat cut

- **Right-click menu beyond Quit/Settings/About.** Tempting to add
  "Compose new envelope" / "Sync now" / etc. but those belong inside
  the dropdown popover, not the menu. Keep right-click minimal.
- **Tray icon animation while syncing.** Static color is enough.
  Animated spinners are noise; principal opens the dropdown if they
  want status.
- **Per-recipient counts in the tray dropdown.** Just totals. The
  cadenced walker is where per-item exists.
- **Custom popover styling beyond what NSPanel + the existing
  React shell provides.** The dropdown is a small render of
  `<TrayPopover>`, not a separately themed surface.

### 🧪 Domain knowledge

- **Tauri `TrayIcon` lifecycle on macOS.** Tray icons can be created
  in the `setup` hook and destroyed on app exit. Confirm clean
  installation/removal — the existing global-shortcut cleanup
  (`lib.rs:Exit` handler) is the pattern.
- **`tauri-plugin-window-state` interaction.** With no main window
  shown by default, the plugin's "restore on launch" behavior should
  still work for the ephemeral review window. But maybe excluding the
  main window from window-state entirely is cleaner. Check.
- **Single-instance plugin behavior** when the user clicks the tray
  icon while the app is already running. Should be a no-op — already
  resident. Confirm.

## Pitch

### Problem

Secretariat's vision is a tool that **stays out of the way**. The
review-session model says the principal opens the surface
intentionally, walks through queued correspondence, and leaves. The
equanimitech pyramid puts "peripheral presence" (principle 4) at the
core: the tool informs without demanding attention. But the v0.2.x
shell ships a persistent main window and a dock icon — both of which
violate that principle by sitting in the principal's visual field
unprompted.

The MCP and daemon already live out-of-process — the principal's AI
assistant works whether or not the app window is open. Daemon
syncs on its own. So the only thing pulling the app onto screen is
the chrome itself: the always-on main window, the always-visible
dock icon. Removing those is the smallest change that aligns the
shipped app with the stated philosophy.

### The bet

Two days of focused work. Add a `TrayIconBuilder`, anchor an NSPanel
popover under it (reusing the template's existing quick-pane
scaffolding), and rewrite the launch lifecycle: first-run shows
main window for onboarding, every subsequent run is menubar-only
with the focused review window only appearing when summoned. The
ReviewSurface component renders inside the popover; the cadenced
walker (separate pitch, future) renders inside the ephemeral
window. Daemon, MCP, and stamp ceremony all unchanged.

The bet pays off if Marcelo + Christophe can install the app and
genuinely *forget it's there* until they choose to look — at which
point the menubar dot tells them whether to bother, and one click
takes them straight into a focused review.

### No-gos

- No persistent main window after first-run onboarding. (Setting to
  re-enable is a v0.3+ ask if anyone misses it.)
- No tray icon animations, custom popover theming, or per-recipient
  badging. Static glyph + one color dot.
- No Windows / Linux tray support. macOS-only.
- No notifications added in this pitch. The tray dot IS the only new
  ambient signal; equanimitech red lines hold.
- No new commands or wire-format changes — purely Tauri shell
  rework.
- No replacement of the cadenced walker (still future work). The
  popover's Review-inbox / Review-outbox buttons keep their
  copy-prompt-to-clipboard behavior until the walker pitch lands.
