---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T101816Z-ju4akj.md
---
# Channels can have emojis

Raw capture — 2026-05-17. Promoted to secretariat:dev from `_self/inbox/triage` 2026-05-18.

- Channels can have emojis.
- Questions:
  - Field on `.channelDef` (`emoji: "🧪"`)? Or derived from channel name?
  - Surface where: quick-pane Launch rows, OrgPicker buttons, tray menu, both?
  - One emoji per channel, or stack (channel emoji + org emoji)?
  - Inheritance from org-level default vs explicit per-channel override?
  - Picker UX on `sec channels create` — suggest from name (à la zenborg's `suggestEmojiForAreaName`) or require explicit?
