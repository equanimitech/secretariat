# Unified review walker over a hierarchical queue tree

Pitch — 2026-05-06. Source: `~/.secretariat/queues/equanimitech/secretariat/20260506T163511Z-7tz5xz.md` (substrate spec capture)

**Hard dependency:** the unified `list_review_queue` core function shipped 2026-05-05 (`crates/core/src/application/review_queue.rs:99`). This pitch is the MCP-side surfacing of what the Rust core already produces.

## Boundaries

### Job to be done

When I run `/review` in Claude Code at the end of a working block, I want one walker that surfaces every envelope addressed to me — peer mail in `~/.secretariat/inbox/`, drafts in `~/.secretariat/outbox/`, AND captures in `~/.secretariat/queues/**` — grouped by area so I can descend into one project at a time (e.g. "themia → data → dommage_corporel") rather than wading through chronological soup. Skipping an area is a valid outcome; the next session re-surfaces what I didn't touch.

Baseline today: `/review` fetches `secretariat://inbox` + `secretariat://outbox` resources, walks them. Captures via `mcp__secretariat__capture` land in `queues/<ns>/<slug>/` — invisible to the walker. Today's session captured 5 ideas across 2 namespaces; none surfaced when I ran `/review` immediately after. The Rust core function `list_review_queue` already unions all three sources; the MCP layer just doesn't expose it.

### Appetite

`medium` — three slices, each a focused day:

1. New `secretariat://review` resource that returns the unified `list_review_queue` output, grouped by area.
2. `QueueHandle` grammar extension to allow nested namespaces (`themia:data:dommage_corporel`).
3. `/review` prompt rewrite: tree descent → leaf walk, with per-envelope action menu varying by kind.

Smaller would skip the grammar work and force flat namespaces, defeating the area-tree framing. Bigger would invite cross-org write ACL (Christophe → `themia:christophe`), which is a separate auth pitch.

## Elements

Four elements; no more.

### Place: `secretariat://review` resource

- **Place:** new MCP resource registered in `crates/mcp/src/server.rs` alongside the existing `RESOURCE_INBOX_URI` and `RESOURCE_OUTBOX_URI` (server.rs:774-775).
- **Affordance:** `read_resource("secretariat://review")` returns markdown grouped by area:

  ```
  # Review

  ## inbox (1)
  - <path> · from <did> · stamped ✓

  ## themia:data:dommage_corporel (3)
  - <path> · capture · 2026-05-06
  - <path> · capture · 2026-05-06
  - ...

  ## equanimitech:secretariat (1)
  - <path> · capture · 2026-05-06
  ```

- **Connection:** backed by `list_review_queue(outbox_root, queues_root)` (already shipped). Add `list_inbox_files(inbox_root)` to the union — currently `list_review_queue` only does outbox + queues, not peer inbox. Either extend the core function or compose at the MCP layer.

### Place: nested `QueueHandle` grammar

- **Place:** `crates/core/src/domain/queue_handle.rs` — relax the slug rule.
- **Affordance:** allow `:` as level separator in the slug component. Grammar becomes `<ns>:<slug>(:<slug>)*`, each segment matching `[a-z][a-z0-9-]*`. Max length stays 64.
- **Connection:** `as_path_segment()` already replaces `:` → `/`, so `themia:data:dommage_corporel` → `themia/data/dommage_corporel/` on disk. No filesystem migration needed for existing flat handles. New helpers: `top_level_namespace()` (first segment), `path_segments()` (vec of all segments).

### Affordance: tree projection helper

- **Place:** `crates/core/src/application/review_queue.rs` — new `group_by_area(envelopes: Vec<ListedEnvelope>) -> AreaTree`.
- **Affordance:** `AreaTree` is a recursive `{ name, count, children: Vec<AreaTree>, leaves: Vec<ListedEnvelope> }`. Inbox + outbox become synthetic top-level nodes (`inbox`, `outbox`); queue captures group by their namespace path.
- **Connection:** the `secretariat://review` resource handler calls `list_review_queue` + `list_inbox_files`, feeds into `group_by_area`, renders to markdown. The walker prompt parses the markdown back into a tree (or — better — the resource emits structured JSON; markdown is for humans).

### Connection: `/review` prompt rewrite — descend then walk

- **Place:** `crates/mcp/src/prompts/review.md`.
- **Affordance:** new flow:
  1. Fetch `secretariat://review`.
  2. Render top-level area counts. Ask principal which area to enter (or `all` to walk flat).
  3. If picked area has sub-areas, recurse — show child counts, ask again.
  4. At a leaf area, walk envelopes one-per-turn (current cadence — verify, render, ask, act).
  5. Action menu varies by envelope kind:
     - peer inbox envelope → `archive` / `skip` / `compose reply`
     - outbox draft (unstamped) → `stamp` / `skip` / `discard`
     - self-capture → `archive` / `skip` / `shape` (delegates to `/shaping <path>`)
- **Connection:** ends naturally when the chosen subtree is exhausted. Other subtrees stay untouched and re-surface next session. One-line summary: _"Reviewed N envelopes across <area> — A archived, S stamped, K skipped."_

## Risks

### 🐇 Rabbit holes

- **Backward compatibility of `QueueHandle` grammar.** Existing parsed handles (`inbox:triage`, `equanimitech:secretariat`) must keep parsing. The new grammar is a strict superset — all current handles match the relaxed rule. But: serialized handles in older envelope frontmatter need to round-trip identically. Spike: write a property test that asserts `parse(s).to_string() == s` for every fixture in the repo. ~30min.
- **Inbox path inclusion in `list_review_queue`.** The core function currently does outbox + queues; peer inbox is fetched separately. Decision: extend the core to take an optional `inbox_root`, or compose at the MCP layer. Compose-at-MCP is less invasive. Pick that.
- **Resource size for large queue trees.** A principal with 100+ captures across 20 areas produces a fat markdown blob. Acceptable for v1 (the principal-side count is small today). If it grows, paginate by top-level area later — `secretariat://review/themia` as a sub-resource. Don't pre-build that now.
- **Walker re-entry mid-session.** If the principal exits at a sub-area and re-runs `/review`, do they resume in that area or restart at top? Restart at top. Simpler. Re-surfacing the area takes one extra prompt turn.

### 🏴 Off-sides called

- **Cross-org peer writes (Christophe → `themia:christophe`).** Out. Needs auth model — invite already grants trust edge, but per-queue ACL is a new domain concept. Separate pitch.
- **Scheduled / time-based bubble-up of captures.** Out. Skip + leave-in-place is the v1 "remind me later." Time-based defer was already off-sides in the inbox-walker pitch (2026-05-05) and stays off-sides here.
- **Tauri app UI walker.** Out. The 2026-05-05 inbox-walker pitch covers the in-app surface. This pitch is MCP-side only — the Claude Code `/review` flow.
- **Replacing `/roundtable` with this walker.** Partially. The walker subsumes the _capture-triage_ part (read each capture, decide action). Roundtable's _bucketing_ (now/later/never) and _shaping dispatch_ are not in this pitch — `shape` action delegates to existing `/shaping`, not to a bucketing system. Roundtable can later become "review with `--mode=triage` and dispatch shaping in parallel."
- **Migration of existing `inbox:triage` captures into nested namespaces.** Out. Existing flat handles keep working; new captures use whatever namespace the principal chooses.
- **Notifications / unread badges.** No. Equanimitech red lines stand.

### 🥩 Fat cut

- **Per-area "mark all archived" bulk action.** No. Per-envelope consent is a substrate invariant from the existing `/review` prompt — keep it.
- **Pretty tree rendering with box-drawing characters.** Plain markdown headings + indented bullets. The walker is a Claude conversation, not a TUI.
- **Storing principal's last-visited area as a cursor for resume.** Restart at top is simpler. Add a cursor only if the principal asks twice.
- **Renaming `secretariat://inbox` and `secretariat://outbox` to deprecate them.** Keep both. The new `secretariat://review` is additive. Other prompts (`/compose`, `/onboard`) already read the originals; don't churn them.

### 🧪 Domain knowledge

- **`list_inbox_files` recursion behavior.** Verify it walks only `inbox/` root, not `inbox/archived/` or `inbox/deferred/` — same risk flagged in the 2026-05-05 inbox-walker pitch. If it recurses, the walker shows already-archived envelopes. ~10min spike against `crates/core/src/application/inbox_ops.rs:92`.
- **`as_path_segment()` interaction with new grammar.** Already does `replace(':', "/")` (queue_handle.rs:101) so nested handles produce nested dirs for free. Verified.
- **The "show body before acting" invariant.** Holds for the new self-capture action menu — `archive`, `shape`, `skip` all happen _after_ the body has been rendered verbatim. No new ceremony work.
- **Lexicon impact.** None. `tech.equanimi.secretariat.envelope` schema is untouched; this is a projection / surface change. The `kind` field stays as-is.

## Pitch

### Problem

The Rust core already produces a unified review queue (`list_review_queue` shipped 2026-05-05). The MCP layer hasn't caught up: `secretariat://inbox` shows peer mail, `secretariat://outbox` shows drafts, captures land in `queues/**` and surface nowhere. Today's session proved the gap — five `/idea` captures hit `inbox:triage` and `equanimitech:secretariat`, none appeared when `/review` ran two minutes later. The principal's mental model was "one walker over everything"; the implementation forced a parallel filesystem walk through `~/.secretariat/queues/`.

The hierarchy ask is the same gap, one level up. The principal's areas are tree-shaped (Themia → data → dommage_corporel; equanimitech → secretariat; r4tb → enurgy). The current grammar enforces flat `<ns>:<slug>`, so the natural path either flattens (`themia:data-dommage-corporel`, losing structure) or escapes the substrate entirely (back to project-local files). Either compromise erodes the "one substrate" promise.

### The bet

Three slices, medium appetite, all additive:

1. Extend `QueueHandle` grammar to accept multi-level namespaces. Backward-compatible — existing handles stay valid. ~½ day with tests.
2. Add `secretariat://review` MCP resource that unions inbox + outbox + queues and emits a tree projection. Reuse `list_review_queue`; compose `list_inbox_files` at the MCP layer. ~1 day.
3. Rewrite `/review` prompt to fetch the new resource, render area counts, descend interactively, walk leaf envelopes one-per-turn with kind-aware action menu. ~½ day.

The bet pays off if, by end of the cycle, running `/review` shows me Themia's data captures grouped under `themia:data:*`, lets me descend into `dommage_corporel`, walk those captures one at a time, and shape the urgent ones via `/shaping` inline — without ever touching the legacy `secretariat://inbox`/`outbox` resources or the filesystem directly.

### No-gos

- No cross-org peer write ACL. `themia:christophe` from Christophe's machine is a separate auth pitch.
- No time-based bubble-up of captures. Skip + leave-in-place.
- No Tauri app UI changes — covered by 2026-05-05 inbox-walker pitch.
- No `/roundtable` deprecation. Walker subsumes triage; bucketing + parallel shaping dispatch stay in roundtable for now.
- No migration of existing captures to nested namespaces. Flat handles keep working.
- No pagination, no resume cursor, no bulk per-area actions.
- No notifications, no unread badges, no auto-open prompts.
- No append-only event log, no new wire format, no lexicon changes.
