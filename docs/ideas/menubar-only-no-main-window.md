# Menubar-only Secretariat — no main window by default

Raw capture — 2026-05-05.

- "Secretariat needs to be running for the MCP to work right? What if we don't even have a window at the moment? What if it's all a menu item?"
- The MCP doesn't need the Tauri app — `sec-mcp` runs as a Claude
  Code/Desktop subprocess, talking to `~/.secretariat/` directly. The
  LaunchAgent daemon polls the relay regardless. The Tauri app is
  purely a review surface.
- This means **the app could ship as a menubar/tray-icon-only utility**
  with no persistent main window. Click the menubar icon → small
  popover with the two big buttons (Review inbox / Review outbox) →
  click → spawn a focused window for that review session → window
  closes when the session ends → back to just the menubar icon.
- Aligned with equanimitech "peripheral presence" (principle 4): the
  app lives in the periphery (menubar) rather than competing for
  desktop space. The principal *summons* the review surface; it
  doesn't sit there asking to be looked at.
- Tauri 2 supports tray icons natively (`TrayIconBuilder`). Existing
  Tauri-template scaffolding would need to swap the auto-show main
  window for a hidden window that's only shown by tray clicks or
  review-session triggers.
- Adjacent: the menubar icon can carry an ambient status indicator —
  small color dot when the inbox or outbox has items pending. Per
  the two-buttons-home idea, color > number.
- Adjacent: menubar dropdowns are perfect for the "tap to glance
  count" use case. Dropdown shows "3 to review · 1 to stamp · sync
  status: 2 min ago"; click either count → opens a focused review
  session.
- Adjacent: the dropdown could also house the "Sync now" affordance,
  removing it from the main review surface entirely. Even less in the
  primary view.
- Questions:
  - Does the main window go away entirely or stay hidden by default?
    Option A: tray-only, all surfaces summoned. Option B: tray +
    optional main window for principals who want to keep it open.
    Lean A — the bet is principals don't want it open.
  - On first run, does the menubar icon onboard or does it pop a
    window? Probably one window for onboarding (the wizard), then
    drops to menubar-only.
  - Onboarding should still feel intentional, but daily life is
    invisible. The wizard is one-shot; menubar is always.
  - Tray icon click on macOS: left-click opens dropdown, right-click
    opens menu. What's the dropdown vs menu split?
  - Where do Settings live in a menubar-only world? Drawer from the
    dropdown, or a separate window summoned by "Settings…"
    menu item.
  - Notifications? Still no — menubar status dot is the ambient
    signal; the principal looks when they choose. Aligned with
    review-session model.
- Implementation lift: medium-ish. Existing Tauri shell has window
  management; needs a TrayIconBuilder, a popover-style window or
  panel attached to the tray, and lifecycle changes (don't auto-show
  main window on launch). All the React components stay; what
  changes is the chrome around them.
- Don't shape yet.
