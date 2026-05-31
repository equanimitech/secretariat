---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T101656Z-zxbmxb.md
---
# sec launch — time budget + focus level

Raw capture — 2026-05-17. Promoted to secretariat:dev from `_self/inbox/triage` 2026-05-18.

- Could `sec launch` carry a time limit, auto-ending the Claude Code session when the budget runs out (prevent massive sessions, force re-intention).
- Pair with a "focus level" knob that almost prevents starting other sessions concurrently — soft lock at the substrate level, not OS-level.
- "I like this very much."
- Questions:
  - Where does the timer live? Tauri daemon ticking against a session registry, or per-launched-process wrapper?
  - Soft vs hard cut at budget expiry — warn-then-grace vs SIGTERM the subprocess?
  - Focus level shape: int rung (0=open / 1=warn / 2=block) or named (`open` / `single-track` / `monk`)?
  - How does the lock interact with `dispatch` (parallel headless agents) once that ships?
  - Does the budget reset per-channel or pool across the day?
