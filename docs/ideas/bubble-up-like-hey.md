# Bubble up like in Hey

Raw capture — 2026-05-04.

- Hey.com's "Bubble Up" lets the user defer a message and have it return to the top of the inbox at a chosen later time. Different from snooze (just hides until time T) — bubble-up is "show me again at T" with the original conversation context intact.
- Fits Secretariat's review-session model better than reactive notifications. The principal isn't pulled to a buzzing message; they tell the system "remind me about this when I'm next reviewing."
- Adjacent affordances Hey ships that might fit: Reply Later, Set Aside (longer-term staging), Imbox vs The Feed (separation of correspondence vs newsletters).
- For Secretariat: bubble-up is the natural "I want to act on this but not now" affordance during a review session. Cheaper than pushing notifications later; pulls when *the principal* re-enters review mode.
- Questions:
  - Does bubble-up work per-envelope or per-thread?
  - Where does the bubble-up state live? Local-only metadata (`~/.secretariat/inbox-state.json`) keeps it out of the wire format.
  - Defer to which trigger: "next time I open the app", "Monday morning", "in 3 days at 9am"?
  - Naming — "bubble up" is Hey's; we might call it "surface again" or just "follow up later".
- Don't shape yet.
