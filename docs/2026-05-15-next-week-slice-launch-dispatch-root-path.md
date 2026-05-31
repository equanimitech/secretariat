---
migrated_from: equanimi.tech/project/secretariat/dev/20260515T105433Z-fvs4wr.md
---
# Next-week slice — `launch + dispatch + root_path`

Principal call 2026-05-15: this is the secretariat next-week scope. Pitch at `docs/pitches/2026-05-13-launch-dispatch-root-path.md`. Medium appetite (1 week).

## Why #1 in the backlog
Today substrate lives parallel to repos. `root_path` collapses the gap — channel-dir binds to working-dir, parent Claude session can dispatch into a channel's full `.claude/` context as subagent. Every other slice in the backlog (per-channel agents, daily reports, workspace registry) is downstream of this.

## Pre-spike before committing
Two rabbit holes flagged in the pitch — resolve before slice 1:
1. **`claude -p` auth from daemon-spawned subprocess** (no TTY, may run under launchd). Spike: `Command::new("claude").arg("-p").arg("hello").current_dir(...)` from daemon context, see if auth resolves cleanly.
2. **macOS notify/symlink edge case** on `fsevents` — prototype with one channel before generalizing the symlink direction (default-path → bound-path or reverse).

## Backlog ranking (post-root_path)
1. ✅ root_path (next week)
2. **Daemon v0.3 minimum subset** — IPC socket (Tauri+CLI race today), ScheduleTicker (duty registry, multiple duties), FS-notify on outbox/ (kill stamp→send latency). Push subscription + AgentSupervisor + RoutingEngine = bigger, defer.
3. **Channel governance artifact** (bare `contract.md`) — server-side stamp-required enforcement for assemblee_generale. No urgency until Christophe DID lands.
4. **`archive` MCP tool gap** — accept channel envelope paths, not just inbox. 1-day fix. Worth doing — workaround (raw rm) corrupts substrate-authoritative invariant.
5. **Workspace registry** (`.secretariat/` in repo, idea 2026-05-12). Big. Compose-on-top of root_path; precursor relationship is real.
6. **70-capture triage backlog from 2026-05-06+** — operational debt, roundtable session.

## Out of scope today
Daemon-IPC + ScheduleTicker would compose nicely with root_path next week but explicitly NOT in scope. One slice at a time.
