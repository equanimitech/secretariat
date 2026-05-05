# Daily review routines (morning inbox, EOD outbox)

Raw capture — 2026-05-05.

- "Should I have daily routines to view inbox (in the morning) and view outbox (EOD)? We could configure these at specific times of day. Could they be through the terminal or better through the app?"
- Concrete instance of the cadenced-reviews idea (`docs/ideas/two-buttons-cadenced-reviews.md`). Schedule = "8am: review inbox" + "6pm: review outbox" (configurable per principal).
- Three design tensions:
  1. **Notification vs ambient signal.** Equanimitech red lines forbid notifications (`equanimitech_principles.md` red lines, `feedback_review_session_model.md`). So the routine cannot pop a banner. It can only shift an ambient signal — the menubar dot color, for example.
  2. **Compulsion vs intention.** The whole point of the review-session model is that the principal *enters* the session; if the system says "it's review time now", that flips back into the compulsive mode. Solution: the time is a *self-set* anchor, like a meditation alarm — the principal sets it for themselves, the system reflects "you said you'd do this around now," nothing more.
  3. **In-app vs terminal.** Both can work. Terminal is good for power users + cron integration. In-app is more discoverable for non-developers. Given the wedge audience (Marcelo, Christophe — non-developers), in-app should be primary; terminal is the power-user surface.
- Proposed shape:
  - **Settings → Review routines** pane (in the menubar dropdown / Settings dialog):
    - "Morning review time: [picker, optional, e.g. 8:00]"
    - "Evening review time: [picker, optional, e.g. 18:00]"
    - Default: both empty (no routines)
  - **At each configured time:**
    - The menubar dot transitions to a distinct color (e.g. *blue* — "it's review time") regardless of queue state. Stays blue for ~30 min or until the principal opens the surface.
    - No notification, no popup, no sound.
    - Once the principal opens the menubar dropdown OR a review session, the dot returns to its normal state (green/amber).
  - **CLI mirror:** `sec routine set --morning 08:00 --evening 18:00`. `sec routine show`. For people who want to set it via terminal or sync via dotfiles.
- Adjacent: the bubble-up idea (`docs/ideas/bubble-up-like-hey.md`) — envelopes deferred to "tomorrow morning" automatically surface during the morning review. Routine + bubble-up compose naturally.
- Adjacent: timezones. The principal's wall-clock time (system locale). Don't try to be clever about peer timezones — that's a future shaping question.
- Questions:
  - Just two anchors (morning/EOD) or arbitrary list of times? Two is the simplest version of the user's mental model. Arbitrary is more flexible. Lean two for v1.
  - Day-of-week filtering — only weekdays? Skip weekends? Configurable, but defaults to "every day" for v1.
  - Does the app open a review session automatically at the configured time, or does it just signal? **Just signals** (no auto-open — that's a notification by another name).
  - Persistence — the routine config lives in `~/.secretariat/routines.toml` or similar, alongside `cadence.toml`.
  - What if the principal misses the time window? The dot stays blue until they enter a review or until the next cycle. They can visit at 9am for the 8am anchor; the system doesn't punish lateness.
- Don't shape yet.
