# Secretariat: path-based links + move-watcher to auto-update edges on rename

Captured 2026-06-08 from Things inbox (weekly review). **Don't shape yet.**

Link envelopes via filesystem paths (URL-style), and run a filesystem watcher that detects
renames/moves and rewrites all existing edges/backlinks so wiki links never break.

**Motivating pain (2026-06-03):** renamed a zenborg idea doc (circular-day-brief →
calendar-zoom-ladder) and had to manually stub the old file with a `[[wiki]]` pointer.
Path-based edges + move-watching would auto-update every reference.

Relates to the `[[name]]` wiki-link convention already used in Claude auto-memory.
