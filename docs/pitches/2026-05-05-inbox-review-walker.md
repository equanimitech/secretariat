# Inbox review walker — Reply / Remind me later / Archive

Pitch — 2026-05-05. Source: free-text + composes three captured ideas:
- `docs/ideas/two-buttons-cadenced-reviews.md` (walker concept)
- `docs/ideas/bubble-up-like-hey.md` (defer / remind-later semantics)
- `docs/ideas/reply-directly-to-message.md` (reply as first-class)

**Hard dependency:** the two-button home (`<ReviewSurface>` shipped post-0.2.1) is the entry point. The walker is what those buttons launch.

## Boundaries

### Job to be done

When I open Secretariat — typically once or twice a day during a chosen review window — I want to walk through the envelopes my dad / Christophe / future correspondents have sent me, one at a time. For each, I should be able to *reply now*, *defer it for later*, *archive it as handled*, or *skip to next* without leaving the walker. The session should end naturally when the queue is exhausted, returning me to the two-button home; I should never feel like I'm in an inbox dashboard.

Baseline today: one envelope sits in my inbox unread since yesterday (Marcelo's "Muito bom mas como mando um file pra você?"). The home shows a count but clicking the button copies a Claude prompt — useful but indirect. There's no way to *act* on an envelope inside the app yet.

### Appetite

`medium` — a couple of focused days. Smaller would skip the action affordances; bigger would invite scope creep into reply-with-quoting, threading, scheduled-defer-to-specific-time. Those are next pitches.

## Elements

Four elements; no more.

### Place: ReviewSession (full-screen walker, replaces home for the session's duration)

- **Place:** the `<ReviewSurface>` component switches state from `home` to `session-inbox` when the principal clicks "Review inbox." Same window, no modal — full take-over.
- **Affordance:**
  - Header: avatar + name (left), "Envelope X of N" + Close button (right)
  - Body: the decrypted envelope contents (sender, body, encrypted-flag if relevant)
  - Action bar: four buttons in fixed order — **Reply** / **Remind me later** / **Archive** / **Next**
- **Connection:**
  - Reply → copies a Claude-ready prompt to clipboard, shows toast, advances to next
  - Remind me later → calls `defer_inbox_envelope`, advances
  - Archive → calls `archive_inbox_envelope`, advances
  - Next → no-op on the envelope, just advances
  - When the index hits N, session ends → return to home → home re-counts (the deferred / archived envelopes are no longer in the active queue)

### Place: Inbox file lifecycle (already wired, just expose the actions)

- `~/.secretariat/inbox/*.md` — active queue (what walker iterates)
- `~/.secretariat/inbox/deferred/*.md` — bubbled later, not in walker
- `~/.secretariat/inbox/archived/*.md` — handled, kept for history, not in walker
- The application use cases `defer_envelope` + `archive_envelope` (shipped 2026-05-05 in `crates/core/src/application/inbox_actions.rs`) move files between these.

### Affordance: Reply via Claude bridge

- The walker doesn't compose in-app (out of scope; aligns with the "drafting lives in the AI assistant" stance per `feedback_review_session_model.md`).
- Reply button copies a focused Claude prompt to clipboard:
  > *"Reply to this Secretariat envelope from <sender>: ‹body›. Use the compose MCP tool with to=<sender-DID> and the body I'll dictate next. Show me the body before you stamp."*
- Shows toast: *"Reply prompt copied — paste into Claude."* Walker advances.

### Connection: Session lifecycle

- Open: button click in home → set state `session-inbox`, fetch active inbox list, set index = 0, render walker with envelope[0].
- Acted (Reply / Defer / Archive / Next): increment index, fetch next or end.
- Empty queue at start: skip the walker, surface a toast "Inbox is clear" on home.
- Close (X / Esc): exit walker, return to home (envelopes not yet reached stay in active queue for next session).

## Risks

### 🐇 Rabbit holes

- **`list_inbox_files` already returns active + subfolder files (or just root?)** Check that the existing `list_inbox` Tauri command + `list_inbox_files` core function only walk one level (root of `inbox/`) and don't recurse into `deferred/` / `archived/` / `sent/`. If they do recurse, deferred envelopes will keep showing up. ~10min spike to verify, possible 20min fix if recursion is happening.
- **What happens when the current envelope was archived/deferred mid-session by another process?** The walker holds an in-memory list snapshot from session start; reading a file that's been moved fails with NotFound. Handle gracefully — skip + advance, log the error. Trivial.
- **Reply prompt body inclusion.** The full envelope body could be huge or include encrypted content. The clipboard prompt should include enough for Claude to know the context but not necessarily the full body. Use the headline + first 200 chars + sender DID. Verify Claude has enough.
- **Touch ID prompt copy reuse.** The `Reply` flow ends in Claude's `compose` tool, then Claude's `stamp` tool, which fires Touch ID. The existing broken Touch ID copy ("touchid-prompt is trying to Teste real" — `docs/ideas/touchid-prompt-copy.md`) shows during the stamp. Out of scope here but worth noting — fixing the Touch ID copy is a separate small pitch.

### 🏴 Off-sides called

- **In-app composer / textarea.** Out of scope; deliberately deferred. Reply goes through the Claude → MCP path, not an in-app form. (See review-session-model memory.)
- **Quote-reply / reply-to-specific-parts.** Captured in `docs/ideas/quote-reply-to-specific-parts.md`. Out of this pitch — we ship reply-as-bridge first, see if the lack hurts.
- **Defer-to-specific-time / bubble-up scheduling.** "Remind me later" v1 just moves to `inbox/deferred/`; principal manually re-surfaces by visiting that folder. Time-based bubble-up is a future pitch.
- **Outbox walker.** "Review outbox" still copies a Claude prompt for now. Outbox walker (with stamp + send affordances per envelope) is a sibling pitch, not this one.
- **Notifications.** No. Equanimitech red lines stand.

### 🥩 Fat cut

- **Action keyboard shortcuts** (R / D / A / Space). Tempting but every shortcut is a learning curve. Ship buttons-only first; shortcuts in v0.4+ if the principal asks.
- **"Mark as read" as a separate action.** Conflates with Archive. The walker treats viewing-the-body as enough acknowledgment; Archive is the explicit "I've handled this." Don't add a Read state.
- **Per-envelope notes / annotations.** Useful eventually but not for v1. The principal's notes go via reply (sent back) or a journal entry (separate scribe-background-journaling idea).
- **Animated transitions between envelopes.** Static replace is fine. Equanimitech "boring by design."

### 🧪 Domain knowledge

- **Toast library** — sonner is already imported in the template (used in the strip-dashboard commit's "Sync now" path). Reuse.
- **Tauri-specta serialization for inbox file lifecycle** — the new `defer_inbox_envelope` / `archive_inbox_envelope` Tauri commands return `String` (path); already wired to bindings via the regen test. Verify before frontend integration.
- **Empty-queue UX** — should the walker open at all if the inbox is empty? No: button click checks count first, falls back to a toast "Inbox is clear" on home. Cleaner than rendering a "0 of 0" walker.

## Pitch

### Problem

The home surface has two big buttons that do nothing useful — they copy prompts to Claude. That works as a placeholder but doesn't honor what the principal is actually trying to do: walk through correspondence one piece at a time, deciding what to act on, what to defer, what to forget. Today the only way to *do* something with an inbox envelope is to read it (CLI: `sec read <path>`), then manually move it (CLI: `mv` to wherever you keep handled mail), and remember the context for replying through your AI assistant.

The cadenced-review-walker idea has been floating in `docs/ideas/` for two days. Marcelo's first message ("Muito bom mas como mando um file pra você?") has been sitting in the inbox since yesterday because the surface to act on it doesn't exist yet. Shipping the walker turns the home buttons from "copy prompt" into a real review affordance — the smallest version of the daily ritual the v0.3 daily-routines idea will eventually build on.

### The bet

Two days, four buttons, one new surface state. Click "Review inbox" → enter the walker → see envelope[0] → choose Reply / Remind me later / Archive / Next → walker advances. Reuse the existing `<ReviewSurface>` component (add session state); reuse the application use cases `defer_envelope` + `archive_envelope` already shipped today; the Reply action stays as a Claude-bridge (clipboard + toast). No new wire format, no new domain primitives.

The bet pays off if I can clear the inbox with my dad's actual message via the app (not the CLI) by end of day tomorrow.

### No-gos

- No in-app composer. Reply = clipboard + Claude.
- No quote-reply / reply-to-parts. Future pitch.
- No defer-to-specific-time. v1 defer = move to `inbox/deferred/`, no scheduling.
- No keyboard shortcuts beyond default browser ones (Esc to close).
- No outbox walker yet — that's a sibling pitch.
- No notifications, no auto-open, no scheduled review prompts.
- No background processing of `inbox/deferred/` to bubble-up. Future pitch.
- No animated transitions, progress bars, or completion celebrations (equanimitech red lines).
