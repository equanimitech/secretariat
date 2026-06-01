---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T101314Z-it2wkz.md
---
# Realtime for channels?

Raw capture — 2026-05-12. Promoted to secretariat:dev from `_self/inbox/triage` 2026-05-18.

- Could we have realtime for channels?
- Adjacent angles:
  - Idea doc already calls this out as a future v0.4 wedge: humans = cadenced poll (15-min floor, anti-compulsion); agents = long-lived push subscription (WebSocket/SSE) over the same owner-as-sequencer log
  - Realtime ≠ notifications — `project_no_read_receipts` memory holds. No "X is typing", no presence dots, no unread badges. Realtime here = sub-second envelope appearance, not surveillance
  - For LOCAL substrate (single-user, multi-agent today): filesystem watch on `<channel>/envelopes/` would fire instantly when daemon-spawned agents drop envelopes. No network needed
  - For BILATERAL (sync across machines): owner's relay pushes via SSE to subscribed daemons; subscriber writes to local `_ciphertext/` then daemon decrypts to `envelopes/`. Filesystem watch picks up locally
- Questions:
  - Two distinct surfaces: (a) local filesystem watch for agent loops, (b) relay push for cross-machine sync. Both? Just (a) first?
  - Anti-compulsion: humans deliberately throttled to 15-min poll even when realtime is technically available — preserve this. Realtime is for *agents acting on their own clock*, not humans being reactive
  - MCP surface: a `subscribe_channel(handle)` long-poll tool? Or a separate notification primitive Claude Code can subscribe to?
  - How does this compose with attention routing (also v0.4)? Realtime drop → routing daemon decides surface vs defer vs digest → human still sees on their cadence
  - Battery / process count cost for N persistent subscriptions on one machine
