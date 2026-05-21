# Quick-pane for capturing message ideas

Raw capture — 2026-05-05.

- "It would be nice to use the quick-pane for ideas of messages to send to people."
- The Tauri template ships a working quick-pane (`src-tauri/src/commands/quick_pane.rs` + `src/components/quick-pane/`) — global keyboard shortcut summons a floating NSPanel from anywhere, type into it, dismisses. Currently does generic "quick entry" stuff for the template.
- Repurpose: the principal hits the global shortcut → small pane appears → types a one-liner ("Tell dad chapter 3 needs more pressure" / "Christophe re: deal review") → optionally picks a recipient (or leaves unaddressed) → submits → the line lands in an "ideas" pool.
- The pool surfaces in the next outbox review session as proto-drafts the AI assistant can flesh out into actual envelopes. Or directly composable: the ideas pool is shown alongside the unstamped queue.
- This is the _capture_ end of the scribe-background-journaling idea (`docs/ideas/scribe-background-journaling.md`) but principal-initiated rather than scribe-initiated. Manual capture is simpler to ship; scribe-auto-capture lands later when the cognition ports are richer.
- Composes with menubar-only: the menubar dot tells you when there's pending review; the quick-pane is the "feed me" affordance from anywhere. Together: ambient state + ambient capture, no window required for either.
- Adjacent: the existing quick-pane shortcut is `Cmd+Shift+.` — fine, but Secretariat-context might justify a different default (e.g. `Cmd+Shift+S` for "say"). Configurable.
- Questions:
  - Where does the captured line live? New `~/.secretariat/ideas/` directory? Inside outbox as unstamped + unaddressed envelopes? Probably their own collection: ideas are pre-envelopes (no `to:` yet).
  - Recipient picker in the pane: dropdown of contacts? Optional? "to: <leave blank>" should be valid (unaddressed → AI assistant proposes recipient at review time).
  - When is the pane dismissed — Esc, click-outside, submit? Probably all three.
  - Does the AI assistant see the ideas pool via MCP? Yes — `list_ideas` MCP tool, paired with `compose_from_idea` that reads the line + drafts an envelope.
  - Voice input? Probably not v0.3 — typing is fine; voice is its own rabbit hole.
- Don't shape yet (already folded into the menubar-only pitch).
